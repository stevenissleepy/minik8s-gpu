use anyhow::{Context, Result, anyhow};
use gpu_plugin_api::{GpuJob, HpcCredential};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::process::Command;

use crate::slurm::{SlurmJobStatus, parse_sacct_line, parse_squeue_line};
use crate::workspace::{JobWorkspace, safe_child_path};

pub struct CollectedOutputs {
    pub files: BTreeMap<String, String>,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

pub struct SshClient {
    login_host: String,
    username: String,
    key_path: std::path::PathBuf,
    known_hosts_path: std::path::PathBuf,
    strict_host_key_checking: bool,
}

impl SshClient {
    pub fn new(login_host: &str, credential: &HpcCredential, workspace: &JobWorkspace) -> Self {
        let strict_host_key_checking = !credential.spec.known_hosts.trim().is_empty();
        Self {
            login_host: login_host.to_string(),
            username: credential.spec.username.clone(),
            key_path: workspace.private_key_path(),
            known_hosts_path: workspace.known_hosts_path(),
            strict_host_key_checking,
        }
    }

    pub async fn create_remote_dir(&self, remote_dir: &str) -> Result<()> {
        self.remote_command(&format!("mkdir -p {remote_dir}"))
            .await
            .map(|_| ())
    }

    pub async fn upload_workspace(&self, remote_dir: &str, workspace: &JobWorkspace) -> Result<()> {
        let mut entries = tokio::fs::read_dir(workspace.root()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name == ".ssh" {
                continue;
            }
            self.scp_to_remote(&path, &format!("{remote_dir}/{name}"))
                .await?;
        }
        Ok(())
    }

    pub async fn remote_command(&self, command: &str) -> Result<String> {
        let output = self
            .base_ssh_command()
            .arg(self.remote())
            .arg(command)
            .output()
            .await
            .with_context(|| format!("failed to run ssh command: {command}"))?;
        decode_output(output, command)
    }

    pub async fn squeue(&self, job_id: &str) -> Result<Option<SlurmJobStatus>> {
        let output = self
            .remote_command(&format!("squeue -j {job_id} -h -o \"%i|%T|%M|%l|%R\""))
            .await?;
        Ok(output.lines().find_map(parse_squeue_line))
    }

    pub async fn sacct(&self, job_id: &str) -> Result<SlurmJobStatus> {
        let output = self
            .remote_command(&format!(
                "sacct -n -X -j {job_id} -P --format=JobID,State,ExitCode,Elapsed,MaxRSS,ReqMem"
            ))
            .await?;
        Ok(parse_sacct_line(&output, job_id))
    }

    pub async fn collect_outputs(
        &self,
        remote_dir: &str,
        slurm_job_id: &str,
        gpujob: &GpuJob,
        workspace: &JobWorkspace,
    ) -> Result<CollectedOutputs> {
        let mut files = BTreeMap::new();
        let output_path = gpujob
            .spec
            .slurm
            .output
            .clone()
            .unwrap_or_else(|| "slurm-%j.out".to_string())
            .replace("%j", slurm_job_id);
        let error_path = gpujob
            .spec
            .slurm
            .error
            .clone()
            .unwrap_or_else(|| "slurm-%j.err".to_string())
            .replace("%j", slurm_job_id);

        let mut collect = gpujob.spec.run.collect.clone();
        collect.push(output_path.clone());
        collect.push(error_path.clone());
        collect.sort();
        collect.dedup();

        for remote_file in collect {
            let resolved_remote_file = remote_file.replace("%j", slurm_job_id);
            let local_path =
                safe_child_path(workspace.root(), &format!("results/{resolved_remote_file}"))?;
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            match self
                .scp_from_remote(&format!("{remote_dir}/{resolved_remote_file}"), &local_path)
                .await
            {
                Ok(()) => {
                    let content = tokio::fs::read_to_string(&local_path)
                        .await
                        .unwrap_or_default();
                    files.insert(config_map_key(&resolved_remote_file), content);
                }
                Err(error) => {
                    tracing::warn!(file = %resolved_remote_file, error = %format!("{error:#}"), "failed to collect output file");
                }
            }
        }

        let stdout_tail = files
            .get(&config_map_key(&output_path))
            .map(|content| tail(content, 8192))
            .unwrap_or_default();
        let stderr_tail = files
            .get(&config_map_key(&error_path))
            .map(|content| tail(content, 8192))
            .unwrap_or_default();
        Ok(CollectedOutputs {
            files,
            stdout_tail,
            stderr_tail,
        })
    }

    async fn scp_to_remote(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        let output = self
            .base_scp_command()
            .arg("-r")
            .arg(local_path)
            .arg(format!("{}:{remote_path}", self.remote()))
            .output()
            .await
            .with_context(|| format!("failed to scp {}", local_path.display()))?;
        decode_output(output, "scp upload").map(|_| ())
    }

    async fn scp_from_remote(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        let output = self
            .base_scp_command()
            .arg(format!("{}:{remote_path}", self.remote()))
            .arg(local_path)
            .output()
            .await
            .with_context(|| format!("failed to scp {remote_path}"))?;
        decode_output(output, "scp download").map(|_| ())
    }

    fn base_ssh_command(&self) -> Command {
        let mut command = Command::new("ssh");
        self.add_ssh_options(&mut command);
        command
    }

    fn base_scp_command(&self) -> Command {
        let mut command = Command::new("scp");
        self.add_ssh_options(&mut command);
        command
    }

    fn add_ssh_options(&self, command: &mut Command) {
        command
            .arg("-i")
            .arg(&self.key_path)
            .arg("-o")
            .arg(format!(
                "StrictHostKeyChecking={}",
                if self.strict_host_key_checking {
                    "yes"
                } else {
                    "accept-new"
                }
            ))
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                self.known_hosts_path.display()
            ));
    }

    fn remote(&self) -> String {
        format!("{}@{}", self.username, self.login_host)
    }
}

fn decode_output(output: std::process::Output, context: &str) -> Result<String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        Err(anyhow!(
            "{context} exited with {}; stdout: {}; stderr: {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ))
    }
}

fn config_map_key(path: &str) -> String {
    path.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn tail(content: &str, max_chars: usize) -> String {
    let chars = content.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return content.to_string();
    }
    chars[chars.len() - max_chars..].iter().collect()
}
