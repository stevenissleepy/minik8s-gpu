use anyhow::{Context, Result, anyhow};
use gpu_plugin_api::{GpuJob, HpcCredential};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub struct JobWorkspace {
    root: PathBuf,
    ssh_dir: PathBuf,
}

impl JobWorkspace {
    pub async fn create(gpujob: &GpuJob) -> Result<Self> {
        let uid = gpujob
            .metadata
            .uid
            .as_deref()
            .unwrap_or(&gpujob.metadata.name);
        let root =
            std::env::temp_dir().join(format!("minik8s-gpujob-{}-{}", gpujob.metadata.name, uid));
        if root.exists() {
            tokio::fs::remove_dir_all(&root)
                .await
                .with_context(|| format!("failed to remove {}", root.display()))?;
        }
        tokio::fs::create_dir_all(&root)
            .await
            .with_context(|| format!("failed to create {}", root.display()))?;
        let ssh_dir = root.join(".ssh");
        tokio::fs::create_dir_all(&ssh_dir).await?;
        Ok(Self { root, ssh_dir })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn private_key_path(&self) -> PathBuf {
        self.ssh_dir.join("id_ed25519")
    }

    pub fn known_hosts_path(&self) -> PathBuf {
        self.ssh_dir.join("known_hosts")
    }

    pub async fn write_sources(&self, gpujob: &GpuJob) -> Result<()> {
        for file in &gpujob.spec.source.files {
            let path = safe_child_path(&self.root, &file.path)?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut handle = tokio::fs::File::create(&path).await?;
            handle.write_all(file.content.as_bytes()).await?;
            if let Some(mode) = file.mode.as_deref() {
                set_mode(&path, mode).await?;
            }
        }
        Ok(())
    }

    pub async fn write_slurm_script(&self, content: &str) -> Result<()> {
        let path = self.root.join("job.slurm");
        tokio::fs::write(&path, content).await?;
        set_mode(&path, "0755").await
    }

    pub async fn write_ssh_files(&self, credential: &HpcCredential) -> Result<()> {
        let private_key = self.private_key_path();
        tokio::fs::write(&private_key, &credential.spec.private_key).await?;
        set_mode(&private_key, "0600").await?;
        if !credential.spec.known_hosts.trim().is_empty() {
            tokio::fs::write(self.known_hosts_path(), &credential.spec.known_hosts).await?;
        }
        Ok(())
    }
}

pub fn remote_work_dir(gpujob: &GpuJob) -> String {
    let namespace = if gpujob.metadata.namespace.is_empty() {
        "default"
    } else {
        &gpujob.metadata.namespace
    };
    let uid = gpujob
        .metadata
        .uid
        .as_deref()
        .unwrap_or(&gpujob.metadata.name);
    format!(
        "~/minik8s-gpujobs/{}-{}-{}",
        sanitize(namespace),
        sanitize(&gpujob.metadata.name),
        sanitize(uid)
    )
}

pub fn safe_child_path(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(anyhow!("path {relative_path} must be relative"));
    }
    let mut result = root.to_path_buf();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            _ => return Err(anyhow!("path {relative_path} must not escape workspace")),
        }
    }
    Ok(result)
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

async fn set_mode(path: &Path, mode: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let parsed = u32::from_str_radix(mode.trim_start_matches('0'), 8)
            .with_context(|| format!("invalid file mode {mode}"))?;
        let permissions = std::fs::Permissions::from_mode(parsed);
        tokio::fs::set_permissions(path, permissions).await?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}
