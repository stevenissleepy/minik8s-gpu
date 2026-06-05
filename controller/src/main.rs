mod reconcile;
mod runner_pod;
mod status;

use anyhow::{Context, Result};
use apimachinery::Resource;
use clap::Parser;
use client_rs::{Client, InformerEvent, ListParams, TypedApi, WorkQueue, spawn_informer};
use gpu_plugin_api::GpuJob;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::reconcile::reconcile_gpujob;

#[derive(Debug, Parser)]
#[command(author, version, about = "Minik8s GPUJob plugin controller")]
struct Args {
    #[arg(long)]
    api_server: Option<String>,

    #[arg(long, default_value = "minik8s/gpujob-runner:latest")]
    runner_image: String,

    #[arg(long, default_value = "http://127.0.0.1:8080")]
    runner_api_server: String,
}

#[derive(Clone)]
pub(crate) struct ControllerConfig {
    pub runner_image: String,
    pub runner_api_server: String,
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
        tracing::error!(error = %format!("{error:#}"), "gpu plugin controller exiting");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let client = match args.api_server.as_deref() {
        Some(endpoint) => Client::new(endpoint)?,
        None => Client::from_env()?,
    };
    let config = ControllerConfig {
        runner_image: args.runner_image,
        runner_api_server: args.runner_api_server,
    };

    let (tx, mut rx) = mpsc::unbounded_channel();
    let informer = spawn_informer::<GpuJob, _>(
        TypedApi::<GpuJob>::all(client.clone()),
        ListParams::default(),
        move |event, _store| match event {
            InformerEvent::Added(key)
            | InformerEvent::Modified(key)
            | InformerEvent::Deleted(key) => {
                let _ = tx.send(key);
            }
            InformerEvent::Error(error) => {
                tracing::warn!(error = %error, "gpujob informer error");
            }
            InformerEvent::Synced => {}
        },
    );

    while !informer.has_synced() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut queue = WorkQueue::default();
    queue.extend(
        informer
            .store
            .items()
            .into_iter()
            .map(|gpujob| gpujob.object_ref()),
    );

    loop {
        while let Ok(key) = rx.try_recv() {
            queue.add(key);
        }

        while let Some(key) = queue.pop() {
            match reconcile_gpujob(&client, &config, &key, &informer.store)
                .await
                .with_context(|| format!("reconcile {}", key.object_key()))
            {
                Ok(()) => queue.forget(&key),
                Err(error) => {
                    tracing::warn!(key = %key.object_key(), error = %format!("{error:#}"), "reconcile failed");
                    queue.add_failed(key);
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
