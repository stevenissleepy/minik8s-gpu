use anyhow::Result;
use api::pod::Pod;
use apimachinery::{DEFAULT_NAMESPACE, ObjectRef};
use client_rs::{Client, Store, TypedApi};
use gpu_plugin_api::{GpuJob, GpuJobPhase};

use crate::ControllerConfig;
use crate::runner_pod::{runner_pod, runner_pod_name};
use crate::status::mark_runner_pod;

pub(crate) async fn reconcile_gpujob(
    client: &Client,
    config: &ControllerConfig,
    key: &ObjectRef,
    gpujobs: &Store<GpuJob>,
) -> Result<()> {
    let namespace = object_namespace(&key.namespace);
    let pod_api = TypedApi::<Pod>::namespaced(client.clone(), namespace.clone());

    let Some(gpujob) = gpujobs.get(key) else {
        let name = runner_pod_name(&key.name);
        match pod_api.delete(&name).await {
            Ok(_) => tracing::info!(namespace, name, "deleted runner pod for removed gpujob"),
            Err(error) if error.is_not_found() => {}
            Err(error) => return Err(error.into()),
        }
        return Ok(());
    };

    if is_terminal(gpujob.status.phase) {
        return Ok(());
    }

    let pod_name = runner_pod_name(&gpujob.metadata.name);
    match pod_api.get(&pod_name).await {
        Ok(_) => {}
        Err(error) if error.is_not_found() => {
            let pod = runner_pod(&gpujob, config);
            pod_api.create(&pod).await?;
            tracing::info!(
                namespace,
                gpujob = %gpujob.metadata.name,
                pod = %pod_name,
                "created gpujob runner pod"
            );
        }
        Err(error) => return Err(error.into()),
    }

    if gpujob.status.runner_pod != pod_name
        || (matches!(gpujob.status.phase, GpuJobPhase::Pending)
            && gpujob.status.slurm_job_id.is_empty())
    {
        mark_runner_pod(client, &namespace, &gpujob, &pod_name).await?;
    }

    Ok(())
}

fn object_namespace(namespace: &str) -> String {
    if namespace.is_empty() {
        DEFAULT_NAMESPACE.to_string()
    } else {
        namespace.to_string()
    }
}

fn is_terminal(phase: GpuJobPhase) -> bool {
    matches!(
        phase,
        GpuJobPhase::Succeeded | GpuJobPhase::Failed | GpuJobPhase::Cancelled
    )
}
