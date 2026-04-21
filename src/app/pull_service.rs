use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{config::ConfigManager, contract::RemoteRef, seafile::SeafileClient};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PullResult {
    pub repo: String,
    pub remote_path: String,
    pub local_path: String,
    pub bytes_written: u64,
    pub overwritten: bool,
}

#[derive(Debug, Clone)]
pub struct PullService {
    config: ConfigManager,
    client: SeafileClient,
}

impl PullService {
    pub fn new(config: ConfigManager, client: SeafileClient) -> Self {
        Self { config, client }
    }

    pub fn pull(&self, remote: &str, local: &Path, overwrite: bool) -> Result<PullResult> {
        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let destination = resolve_local_destination(local, &remote.path)?;
        let overwritten = destination.exists();
        if overwritten && !overwrite {
            bail!(
                "local destination `{}` already exists; rerun with --overwrite to replace it",
                destination.display()
            );
        }

        let parent = destination.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "local destination `{}` has no parent directory",
                destination.display()
            )
        })?;
        if !parent.exists() {
            bail!(
                "parent directory `{}` does not exist for destination `{}`",
                parent.display(),
                destination.display()
            );
        }

        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_remote_ref(&remote, &repositories)?;

        let download_link = self
            .client
            .get_download_link(&resolved.repo_id, &resolved.path)?;
        let temp_path = temporary_download_path(&destination)?;
        let bytes_written = self.client.download_file(&download_link, &temp_path)?;
        std::fs::rename(&temp_path, &destination).with_context(|| {
            format!(
                "failed to move downloaded file into place: {}",
                destination.display()
            )
        })?;

        Ok(PullResult {
            repo: resolved.repo_name,
            remote_path: resolved.path,
            local_path: destination.display().to_string(),
            bytes_written,
            overwritten,
        })
    }
}

fn resolve_local_destination(local: &Path, remote_path: &str) -> Result<PathBuf> {
    if local.exists() && local.is_dir() {
        let name = remote_path
            .rsplit('/')
            .next()
            .filter(|segment| !segment.is_empty())
            .ok_or_else(|| anyhow::anyhow!("remote path `{remote_path}` must point to a file"))?;
        return Ok(local.join(name));
    }
    Ok(local.to_path_buf())
}

fn temporary_download_path(destination: &Path) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("destination `{}` has no parent", destination.display()))?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "destination `{}` has invalid filename",
                destination.display()
            )
        })?;
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_nanos();
    Ok(parent.join(format!(".{name}.thufs-part-{suffix}")))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::resolve_local_destination;

    #[test]
    fn existing_directory_destination_uses_remote_filename() {
        let temp = tempdir().expect("tempdir");
        let destination =
            resolve_local_destination(temp.path(), "/slides/week1.pdf").expect("resolve");
        assert!(destination.ends_with("week1.pdf"));
    }
}
