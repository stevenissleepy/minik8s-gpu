use anyhow::{Result, anyhow};
use apimachinery::{HasStatus, ObjectMeta, Resource, TypeMeta, Validatable};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{GROUP, VERSION};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuJob {
    #[serde(flatten)]
    pub types: TypeMeta,
    pub metadata: ObjectMeta,
    pub spec: GpuJobSpec,
    #[serde(default)]
    pub status: GpuJobStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuJobSpec {
    #[serde(rename = "credentialRef")]
    pub credential_ref: CredentialRef,
    pub hpc: HpcSpec,
    pub slurm: SlurmSpec,
    pub source: GpuJobSourceSpec,
    pub run: GpuJobRunSpec,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRef {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HpcSpec {
    #[serde(rename = "loginHost")]
    pub login_host: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlurmSpec {
    pub partition: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos: Option<String>,
    #[serde(default = "default_one")]
    pub nodes: u32,
    #[serde(rename = "ntasksPerNode", default = "default_one")]
    pub ntasks_per_node: u32,
    #[serde(rename = "cpusPerTask", default = "default_one")]
    pub cpus_per_task: u32,
    pub gres: String,
    pub time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuJobSourceSpec {
    #[serde(default)]
    pub files: Vec<GpuJobSourceFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuJobSourceFile {
    pub path: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuJobRunSpec {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collect: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuJobPhase {
    #[default]
    Pending,
    Preparing,
    Uploading,
    Submitted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuJobStatus {
    #[serde(default)]
    pub phase: GpuJobPhase,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(
        rename = "runnerPod",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub runner_pod: String,
    #[serde(
        rename = "slurmJobId",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub slurm_job_id: String,
    #[serde(
        rename = "slurmState",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub slurm_state: String,
    #[serde(
        rename = "remoteWorkDir",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub remote_work_dir: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub elapsed: String,
    #[serde(
        rename = "timeLimit",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub time_limit: String,
    #[serde(rename = "exitCode", default, skip_serializing_if = "String::is_empty")]
    pub exit_code: String,
    #[serde(rename = "maxRss", default, skip_serializing_if = "String::is_empty")]
    pub max_rss: String,
    #[serde(rename = "reqMem", default, skip_serializing_if = "String::is_empty")]
    pub req_mem: String,
    #[serde(
        rename = "stdoutTail",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub stdout_tail: String,
    #[serde(
        rename = "stderrTail",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub stderr_tail: String,
    #[serde(
        rename = "resultConfigMap",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub result_config_map: String,
    #[serde(
        rename = "lastProbeTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_probe_time: Option<DateTime<Utc>>,
    #[serde(rename = "startTime", default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(
        rename = "completionTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub completion_time: Option<DateTime<Utc>>,
}

impl Resource for GpuJob {
    type DynamicType = ();

    fn kind(_: &()) -> Cow<'_, str> {
        "GPUJob".into()
    }

    fn plural(_: &()) -> Cow<'_, str> {
        "gpujobs".into()
    }

    fn group(_: &()) -> Cow<'_, str> {
        GROUP.into()
    }

    fn version(_: &()) -> Cow<'_, str> {
        VERSION.into()
    }

    fn metadata(&self) -> &ObjectMeta {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut ObjectMeta {
        &mut self.metadata
    }
}

impl HasStatus for GpuJob {
    type Status = GpuJobStatus;

    fn status(&self) -> &Self::Status {
        &self.status
    }

    fn status_mut(&mut self) -> &mut Self::Status {
        &mut self.status
    }
}

impl Validatable for GpuJob {
    fn validate_spec(&self) -> Result<()> {
        if self.spec.credential_ref.name.trim().is_empty() {
            return Err(anyhow!("gpujob spec.credentialRef.name is required"));
        }
        if self.spec.hpc.login_host.trim().is_empty() {
            return Err(anyhow!("gpujob spec.hpc.loginHost is required"));
        }
        if self.spec.slurm.partition.trim().is_empty() {
            return Err(anyhow!("gpujob spec.slurm.partition is required"));
        }
        if self.spec.slurm.gres.trim().is_empty() {
            return Err(anyhow!("gpujob spec.slurm.gres is required"));
        }
        if self.spec.slurm.time.trim().is_empty() {
            return Err(anyhow!("gpujob spec.slurm.time is required"));
        }
        if self.spec.slurm.nodes == 0
            || self.spec.slurm.ntasks_per_node == 0
            || self.spec.slurm.cpus_per_task == 0
        {
            return Err(anyhow!(
                "gpujob spec.slurm nodes, ntasksPerNode and cpusPerTask must be greater than 0"
            ));
        }
        if self.spec.source.files.is_empty() {
            return Err(anyhow!("gpujob spec.source.files must not be empty"));
        }
        for file in &self.spec.source.files {
            validate_relative_path(&file.path, "gpujob spec.source.files.path")?;
        }
        if self.spec.run.command.trim().is_empty() {
            return Err(anyhow!("gpujob spec.run.command is required"));
        }
        for path in &self.spec.run.collect {
            validate_relative_path(path, "gpujob spec.run.collect")?;
        }
        Ok(())
    }
}

fn validate_relative_path(path: &str, field: &str) -> Result<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    if trimmed.starts_with('/') || trimmed.contains("..") {
        return Err(anyhow!(
            "{field} must be a relative path inside the job workspace"
        ));
    }
    Ok(())
}

fn default_one() -> u32 {
    1
}
