use anyhow::Result;
use chrono::Utc;
use client_rs::{Client, TypedApi};
use gpu_plugin_api::{GpuJob, GpuJobPhase};

pub(crate) async fn mark_runner_pod(
    client: &Client,
    namespace: &str,
    gpujob: &GpuJob,
    pod_name: &str,
) -> Result<()> {
    let mut status = gpujob.status.clone();
    status.phase = GpuJobPhase::Preparing;
    status.runner_pod = pod_name.to_string();
    status.message = "runner pod created".to_string();
    status.last_probe_time = Some(Utc::now());

    let api = TypedApi::<GpuJob>::namespaced(client.clone(), namespace.to_string());
    api.replace_status(&gpujob.metadata.name, &status).await?;
    Ok(())
}
