mod slurm;
mod ssh;
mod status;
mod workspace;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use client_rs::{Client, TypedApi};
use gpu_plugin_api::{GpuJob, HpcCredential};
use std::time::Duration;

use crate::slurm::{SlurmJobStatus, SlurmTerminalState};
use crate::status::{fail, finish, set_phase};
use crate::workspace::{JobWorkspace, remote_work_dir};

#[derive(Debug, Parser)]
#[command(author, version, about = "Minik8s GPUJob Slurm runner")]
struct Args {
    #[arg(long)]
    api_server: Option<String>,

    #[arg(long, default_value = "default")]
    namespace: String,

    #[arg(long)]
    name: String,

    #[arg(long)]
    dry_run: bool,

    #[arg(long, default_value_t = 10)]
    poll_seconds: u64,
}

fn main() {
    logger::init_tracing();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(error = %error, "failed to build tokio runtime");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(run()) {
        tracing::error!(error = %format!("{error:#}"), "gpujob runner exiting");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let client = match args.api_server.as_deref() {
        Some(endpoint) => Client::new(endpoint)?,
        None => Client::from_env()?,
    };
    let gpujob_api = TypedApi::<GpuJob>::namespaced(client.clone(), args.namespace.clone());
    let gpujob = gpujob_api.get(&args.name).await?;

    match run_gpujob(&client, &args, &gpujob).await {
        Ok(()) => Ok(()),
        Err(error) => {
            let gpujob = gpujob_api.get(&args.name).await.unwrap_or(gpujob);
            let message = format!("{error:#}");
            fail(
                &client,
                &args.namespace,
                &gpujob,
                "RunnerFailed",
                &message,
                None,
            )
            .await
            .ok();
            Err(error)
        }
    }
}

async fn run_gpujob(client: &Client, args: &Args, gpujob: &GpuJob) -> Result<()> {
    set_phase(
        client,
        &args.namespace,
        gpujob,
        gpu_plugin_api::GpuJobPhase::Preparing,
        "preparing local workspace",
    )
    .await?;

    let credential_api =
        TypedApi::<HpcCredential>::namespaced(client.clone(), args.namespace.clone());
    let credential = credential_api
        .get(&gpujob.spec.credential_ref.name)
        .await
        .context("failed to read HPCCredential")?;

    let workspace = JobWorkspace::create(gpujob).await?;
    let slurm_script = slurm::render_script(gpujob);
    workspace.write_sources(gpujob).await?;
    workspace.write_slurm_script(&slurm_script).await?;
    workspace.write_ssh_files(&credential).await?;

    if args.dry_run {
        println!("{}", slurm_script);
        set_phase(
            client,
            &args.namespace,
            gpujob,
            gpu_plugin_api::GpuJobPhase::Succeeded,
            "dry-run completed; Slurm script generated",
        )
        .await?;
        return Ok(());
    }

    let remote_dir = remote_work_dir(gpujob);
    set_phase(
        client,
        &args.namespace,
        gpujob,
        gpu_plugin_api::GpuJobPhase::Uploading,
        "uploading workspace to HPC",
    )
    .await?;

    let ssh = ssh::SshClient::new(&gpujob.spec.hpc.login_host, &credential, &workspace);
    ssh.create_remote_dir(&remote_dir).await?;
    ssh.upload_workspace(&remote_dir, &workspace).await?;

    let output = ssh
        .remote_command(&format!("cd {remote_dir} && sbatch job.slurm"))
        .await
        .context("sbatch failed")?;
    let slurm_job_id = slurm::parse_sbatch_job_id(&output)
        .ok_or_else(|| anyhow!("failed to parse sbatch output: {output}"))?;
    status::submitted(client, &args.namespace, gpujob, &slurm_job_id, &remote_dir).await?;

    let terminal = poll_until_terminal(
        client,
        &args.namespace,
        gpujob,
        &ssh,
        &slurm_job_id,
        Duration::from_secs(args.poll_seconds),
    )
    .await?;

    let collected = ssh
        .collect_outputs(&remote_dir, &slurm_job_id, gpujob, &workspace)
        .await?;
    let result_config_map = status::store_results(
        client,
        &args.namespace,
        gpujob,
        &slurm_job_id,
        &collected.files,
    )
    .await?;
    finish(
        client,
        &args.namespace,
        gpujob,
        &terminal,
        collected.stdout_tail,
        collected.stderr_tail,
        result_config_map,
    )
    .await?;
    Ok(())
}

async fn poll_until_terminal(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    ssh: &ssh::SshClient,
    slurm_job_id: &str,
    interval: Duration,
) -> Result<SlurmJobStatus> {
    loop {
        if let Some(status) = ssh.squeue(slurm_job_id).await? {
            status::running(client, namespace, gpujob, &status).await?;
            tokio::time::sleep(interval).await;
            continue;
        }

        let status = ssh.sacct(slurm_job_id).await?;
        if matches!(status.terminal_state(), SlurmTerminalState::Unknown) {
            tokio::time::sleep(interval).await;
            continue;
        }
        return Ok(status);
    }
}
