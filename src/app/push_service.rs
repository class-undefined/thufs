use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{EntryKind, SeafileClient},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushResult {
    pub repo: String,
    pub remote_path: String,
    pub local_path: String,
    pub size: u64,
    pub overwritten: bool,
}

#[derive(Debug, Clone)]
pub struct PushService {
    config: ConfigManager,
    client: SeafileClient,
}

impl PushService {
    pub fn new(config: ConfigManager, client: SeafileClient) -> Self {
        Self { config, client }
    }

    pub fn push(&self, local: &Path, remote: &str, overwrite: bool) -> Result<PushResult> {
        self.validate_local_source(local)?;

        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_remote_ref(&remote, &repositories)?;

        let (parent_dir, target_name) = split_parent_and_name(&resolved.path)?;
        let entries = self
            .client
            .list_directory_entries(&resolved.repo_id, parent_dir)?;
        let existing = entries
            .iter()
            .find(|entry| entry.name == target_name)
            .cloned();
        let overwritten = existing.is_some();

        let metadata = std::fs::metadata(local)
            .with_context(|| format!("failed to inspect {}", local.display()))?;
        let uploaded = match existing {
            Some(entry) => {
                if entry.kind != EntryKind::File {
                    bail!("remote target `{}` is not a file path", resolved.path);
                }
                if !overwrite {
                    bail!(
                        "remote target `{}` already exists; rerun with --overwrite to replace it",
                        resolved.path
                    );
                }
                let update_link = self
                    .client
                    .get_update_link(&resolved.repo_id, &resolved.path)?;
                self.client
                    .update_file(&update_link, local, &resolved.path)?
            }
            None => {
                let upload_link = self.client.get_upload_link(&resolved.repo_id, parent_dir)?;
                self.client
                    .upload_file(&upload_link, local, parent_dir, target_name, false)?
            }
        };

        Ok(PushResult {
            repo: resolved.repo_name,
            remote_path: resolved.path,
            local_path: local.display().to_string(),
            size: uploaded.size.unwrap_or(metadata.len()),
            overwritten,
        })
    }

    fn validate_local_source(&self, local: &Path) -> Result<()> {
        if !local.exists() {
            bail!("local source `{}` does not exist", local.display());
        }
        if !local.is_file() {
            bail!("local source `{}` is not a regular file", local.display());
        }
        Ok(())
    }
}

fn split_parent_and_name(path: &str) -> Result<(&str, &str)> {
    let normalized = path.trim();
    let idx = normalized
        .rfind('/')
        .ok_or_else(|| anyhow::anyhow!("remote path `{normalized}` is invalid"))?;
    let name = &normalized[idx + 1..];
    if name.is_empty() {
        bail!("remote path `{normalized}` must point to a file");
    }
    let parent = if idx == 0 { "/" } else { &normalized[..idx] };
    Ok((parent, name))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::PushService;
    use crate::{
        config::{Config, ConfigManager, OutputMode},
        seafile::SeafileClient,
    };

    fn make_service() -> (tempfile::TempDir, PushService) {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("test-token".to_string()),
                default_repo: Some("course-lib".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");

        let service = PushService::new(manager.clone(), SeafileClient::new(manager));
        (temp, service)
    }

    #[test]
    fn rejects_missing_local_source() {
        let (_temp, service) = make_service();
        let err = service
            .push(
                PathBuf::from("/definitely/missing").as_path(),
                "repo:course-lib/a.txt",
                false,
            )
            .expect_err("should fail");
        assert!(err.to_string().contains("does not exist"));
    }
}
