use api::node::TaintEffect;
use api::pod::{ContainerSpec, EnvVar, Pod, PodSpec, Toleration, TolerationOperator};
use apimachinery::{DEFAULT_NAMESPACE, ObjectMeta, Resource, TypeMeta};
use gpu_plugin_api::GpuJob;
use std::collections::BTreeMap;

use crate::ControllerConfig;

pub(crate) fn runner_pod_name(gpujob_name: &str) -> String {
    format!("gpujob-runner-{gpujob_name}")
}

pub(crate) fn runner_pod(gpujob: &GpuJob, config: &ControllerConfig) -> Pod {
    let namespace = if gpujob.metadata.namespace.is_empty() {
        DEFAULT_NAMESPACE.to_string()
    } else {
        gpujob.metadata.namespace.clone()
    };
    let pod_name = runner_pod_name(&gpujob.metadata.name);
    let mut labels = BTreeMap::new();
    labels.insert("component".to_string(), "gpujob-runner".to_string());
    labels.insert(
        "gpujob.minik8s.io/name".to_string(),
        gpujob.metadata.name.clone(),
    );
    if let Some(uid) = gpujob.metadata.uid.as_deref() {
        labels.insert("gpujob.minik8s.io/uid".to_string(), uid.to_string());
    }

    Pod {
        types: TypeMeta::for_resource::<Pod>(),
        metadata: ObjectMeta {
            name: pod_name,
            namespace,
            labels,
            owner_references: vec![gpujob.owner_ref()],
            ..ObjectMeta::default()
        },
        spec: PodSpec {
            host_network: true,
            restart_policy: Some("Never".to_string()),
            node_selector: control_plane_selector(),
            tolerations: control_plane_toleration(),
            containers: vec![ContainerSpec {
                name: "runner".to_string(),
                image: config.runner_image.clone(),
                command: vec!["gpujob-runner".to_string()],
                args: vec![
                    format!("--api-server={}", config.runner_api_server),
                    format!("--namespace={}", gpujob.metadata.namespace),
                    format!("--name={}", gpujob.metadata.name),
                ],
                env: vec![
                    EnvVar {
                        name: "GPUJOB_NAMESPACE".to_string(),
                        value: gpujob.metadata.namespace.clone(),
                        value_from: None,
                    },
                    EnvVar {
                        name: "GPUJOB_NAME".to_string(),
                        value: gpujob.metadata.name.clone(),
                        value_from: None,
                    },
                ],
                ..ContainerSpec::default()
            }],
            ..PodSpec::default()
        },
        status: Default::default(),
    }
}

fn control_plane_selector() -> BTreeMap<String, String> {
    BTreeMap::from([(
        "node-role.kubernetes.io/control-plane".to_string(),
        "".to_string(),
    )])
}

fn control_plane_toleration() -> Vec<Toleration> {
    vec![Toleration {
        key: Some("node-role.kubernetes.io/control-plane".to_string()),
        operator: TolerationOperator::Exists,
        value: None,
        effect: Some(TaintEffect::NoSchedule),
        toleration_seconds: None,
    }]
}
