use anyhow::Result;
use api::configmap::ConfigMap;
use apimachinery::{ObjectMeta, Resource, TypeMeta};
use chrono::Utc;
use client_rs::{Client, TypedApi};
use gpu_plugin_api::{GpuJob, GpuJobPhase, GpuJobStatus};
use std::collections::BTreeMap;

use crate::slurm::SlurmJobStatus;

pub async fn set_phase(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    phase: GpuJobPhase,
    message: &str,
) -> Result<()> {
    let mut status = current_status(client, namespace, gpujob).await?;
    status.phase = phase;
    status.message = message.to_string();
    status.last_probe_time = Some(Utc::now());
    if status.start_time.is_none() {
        status.start_time = Some(Utc::now());
    }
    replace(client, namespace, gpujob, status).await
}

pub async fn submitted(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    slurm_job_id: &str,
    remote_work_dir: &str,
) -> Result<()> {
    let mut status = current_status(client, namespace, gpujob).await?;
    status.phase = GpuJobPhase::Submitted;
    status.message = "submitted Slurm job".to_string();
    status.slurm_job_id = slurm_job_id.to_string();
    status.remote_work_dir = remote_work_dir.to_string();
    status.submitted_time = Some(Utc::now());
    status.query_command = format!("squeue -j {slurm_job_id}");
    status.last_probe_time = Some(Utc::now());
    if status.start_time.is_none() {
        status.start_time = Some(Utc::now());
    }
    replace(client, namespace, gpujob, status).await
}

pub async fn running(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    slurm: &SlurmJobStatus,
) -> Result<()> {
    let mut status = current_status(client, namespace, gpujob).await?;
    merge_slurm(&mut status, slurm);
    status.phase = if slurm.state == "PENDING" {
        GpuJobPhase::Pending
    } else {
        GpuJobPhase::Running
    };
    status.message = if slurm.reason_or_node.is_empty() {
        if slurm.state == "PENDING" {
            "Slurm job pending".to_string()
        } else {
            "Slurm job running".to_string()
        }
    } else if slurm.state == "PENDING" {
        format!("Slurm job pending: {}", slurm.reason_or_node)
    } else {
        format!("Slurm job running: {}", slurm.reason_or_node)
    };
    status.last_probe_time = Some(Utc::now());
    replace(client, namespace, gpujob, status).await
}

pub async fn fail(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    reason: &str,
    message: &str,
    slurm: Option<&SlurmJobStatus>,
) -> Result<()> {
    let mut status = current_status(client, namespace, gpujob).await?;
    status.phase = GpuJobPhase::Failed;
    status.reason = reason.to_string();
    status.message = message.to_string();
    status.last_probe_time = Some(Utc::now());
    status.completion_time = Some(Utc::now());
    if let Some(slurm) = slurm {
        merge_slurm(&mut status, slurm);
    }
    replace(client, namespace, gpujob, status).await
}

pub async fn finish(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    slurm: &SlurmJobStatus,
    stdout_tail: String,
    stderr_tail: String,
    result_config_map: String,
) -> Result<()> {
    let mut status = current_status(client, namespace, gpujob).await?;
    merge_slurm(&mut status, slurm);
    status.phase = slurm.phase();
    status.reason = slurm.reason();
    status.message = if status.reason.is_empty() {
        "Slurm job completed".to_string()
    } else {
        format!("Slurm job ended with state {}", slurm.state)
    };
    status.stdout_tail = stdout_tail;
    status.stderr_tail = stderr_tail;
    status.result_config_map = result_config_map;
    status.last_probe_time = Some(Utc::now());
    status.completion_time = Some(Utc::now());
    replace(client, namespace, gpujob, status).await
}

pub async fn store_results(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    slurm_job_id: &str,
    files: &BTreeMap<String, String>,
) -> Result<String> {
    if files.is_empty() {
        return Ok(String::new());
    }
    let name = format!("gpujob-{}-result", gpujob.metadata.name);
    let mut data = files.clone();
    data.insert("slurmJobId".to_string(), slurm_job_id.to_string());
    let configmap = ConfigMap {
        types: TypeMeta::for_resource::<ConfigMap>(),
        metadata: ObjectMeta {
            name: name.clone(),
            namespace: namespace.to_string(),
            labels: BTreeMap::from([(
                "gpujob.minik8s.io/name".to_string(),
                gpujob.metadata.name.clone(),
            )]),
            owner_references: vec![gpujob.owner_ref()],
            ..ObjectMeta::default()
        },
        data,
        binary_data: BTreeMap::new(),
        immutable: None,
    };
    let api = TypedApi::<ConfigMap>::namespaced(client.clone(), namespace.to_string());
    api.apply(&configmap).await?;
    Ok(name)
}

async fn replace(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    status: GpuJobStatus,
) -> Result<()> {
    let api = TypedApi::<GpuJob>::namespaced(client.clone(), namespace.to_string());
    api.replace_status(&gpujob.metadata.name, &status).await?;
    Ok(())
}

async fn current_status(client: &Client, namespace: &str, gpujob: &GpuJob) -> Result<GpuJobStatus> {
    let api = TypedApi::<GpuJob>::namespaced(client.clone(), namespace.to_string());
    Ok(api.get(&gpujob.metadata.name).await?.status)
}

fn merge_slurm(status: &mut GpuJobStatus, slurm: &SlurmJobStatus) {
    status.slurm_job_id = slurm.job_id.clone();
    status.slurm_state = slurm.state.clone();
    if !slurm.job_id.is_empty() && status.query_command.is_empty() {
        status.query_command = format!("squeue -j {}", slurm.job_id);
    }
    status.elapsed = slurm.elapsed.clone();
    status.time_limit = slurm.time_limit.clone();
    status.exit_code = slurm.exit_code.clone();
    status.max_rss = slurm.max_rss.clone();
    status.req_mem = slurm.req_mem.clone();
}
