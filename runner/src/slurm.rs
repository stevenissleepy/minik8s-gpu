use gpu_plugin_api::{GpuJob, GpuJobPhase};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlurmJobStatus {
    pub job_id: String,
    pub state: String,
    pub elapsed: String,
    pub time_limit: String,
    pub reason_or_node: String,
    pub exit_code: String,
    pub max_rss: String,
    pub req_mem: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlurmTerminalState {
    Succeeded,
    Failed,
    Cancelled,
    Unknown,
}

impl SlurmJobStatus {
    pub fn terminal_state(&self) -> SlurmTerminalState {
        match self.state.as_str() {
            "COMPLETED" if self.exit_code.is_empty() || self.exit_code == "0:0" => {
                SlurmTerminalState::Succeeded
            }
            "CANCELLED" | "CANCELLED+" => SlurmTerminalState::Cancelled,
            "FAILED" | "OUT_OF_MEMORY" | "TIMEOUT" | "NODE_FAIL" | "PREEMPTED" => {
                SlurmTerminalState::Failed
            }
            _ => SlurmTerminalState::Unknown,
        }
    }

    pub fn phase(&self) -> GpuJobPhase {
        match self.terminal_state() {
            SlurmTerminalState::Succeeded => GpuJobPhase::Succeeded,
            SlurmTerminalState::Failed => GpuJobPhase::Failed,
            SlurmTerminalState::Cancelled => GpuJobPhase::Cancelled,
            SlurmTerminalState::Unknown => GpuJobPhase::Running,
        }
    }

    pub fn reason(&self) -> String {
        match self.state.as_str() {
            "OUT_OF_MEMORY" => "OutOfMemory",
            "TIMEOUT" => "TimeLimitExceeded",
            "CANCELLED" | "CANCELLED+" => "Cancelled",
            "COMPLETED" => "",
            "FAILED" => "RuntimeFailed",
            "NODE_FAIL" => "NodeFailed",
            "PREEMPTED" => "Preempted",
            state if state.is_empty() => "Unknown",
            state => state,
        }
        .to_string()
    }
}

pub fn render_script(gpujob: &GpuJob) -> String {
    let spec = &gpujob.spec.slurm;
    let mut lines = vec![
        "#!/bin/bash".to_string(),
        format!("#SBATCH --job-name={}", gpujob.metadata.name),
        format!("#SBATCH --partition={}", spec.partition),
        format!("#SBATCH --nodes={}", spec.nodes),
        format!("#SBATCH --ntasks-per-node={}", spec.ntasks_per_node),
        format!("#SBATCH --cpus-per-task={}", spec.cpus_per_task),
        format!("#SBATCH --gres={}", spec.gres),
        format!("#SBATCH --time={}", spec.time),
    ];
    if let Some(qos) = spec.qos.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!("#SBATCH --qos={qos}"));
    }
    if let Some(output) = spec.output.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!("#SBATCH --output={output}"));
    }
    if let Some(error) = spec.error.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!("#SBATCH --error={error}"));
    }
    lines.push(String::new());
    lines.push("set -euo pipefail".to_string());
    lines.push(String::new());
    for module in &spec.modules {
        lines.push(format!("module load {module}"));
    }
    lines.push(String::new());
    lines.push("echo \"[minik8s] build started\"".to_string());
    lines.push("bash build.sh".to_string());
    lines.push(String::new());
    lines.push("echo \"[minik8s] run started\"".to_string());
    lines.push(gpujob.spec.run.command.clone());
    lines.push(String::new());
    lines.push("echo \"[minik8s] completed\"".to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_sbatch_job_id(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|window| {
            if window[0] == "job" && window[1].chars().all(|ch| ch.is_ascii_digit()) {
                Some(window[1].to_string())
            } else {
                None
            }
        })
}

pub fn parse_squeue_line(line: &str) -> Option<SlurmJobStatus> {
    let fields = line.trim().split('|').collect::<Vec<_>>();
    if fields.len() < 5 {
        return None;
    }
    Some(SlurmJobStatus {
        job_id: fields[0].to_string(),
        state: fields[1].to_string(),
        elapsed: fields[2].to_string(),
        time_limit: fields[3].to_string(),
        reason_or_node: fields[4].to_string(),
        ..SlurmJobStatus::default()
    })
}

pub fn parse_sacct_line(output: &str, job_id: &str) -> SlurmJobStatus {
    for line in output.lines() {
        let fields = line.trim().split('|').collect::<Vec<_>>();
        if fields.len() < 6 || fields[0] != job_id {
            continue;
        }
        return SlurmJobStatus {
            job_id: fields[0].to_string(),
            state: fields[1].trim_end_matches('+').to_string(),
            exit_code: fields[2].to_string(),
            elapsed: fields[3].to_string(),
            max_rss: fields[4].to_string(),
            req_mem: fields[5].to_string(),
            ..SlurmJobStatus::default()
        };
    }
    SlurmJobStatus {
        job_id: job_id.to_string(),
        state: "UNKNOWN".to_string(),
        ..SlurmJobStatus::default()
    }
}
