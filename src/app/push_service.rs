use std::{
    io::{IsTerminal, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{EntryKind, SeafileClient},
    transfer::ConflictPolicy,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushResult {
    pub repo: String,
    pub remote_path: String,
    pub local_path: String,
    pub size: u64,
    pub overwritten: bool,
    pub renamed: bool,
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

    pub fn push(&self, local: &Path, remote: &str, policy: ConflictPolicy) -> Result<PushResult> {
        self.validate_local_source(local)?;

        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_remote_ref(&remote, &repositories)?;

        let (parent_dir, target_name) = split_parent_and_name(&resolved.path)?;
        let entries = self
            .client
            .list_directory_entries(&resolved.repo_id, parent_dir)?;
        let mut final_remote_path = resolved.path.clone();
        let mut final_target_name = target_name.to_string();
        let existing = entries
            .iter()
            .find(|entry| entry.name == target_name)
            .cloned();
        let mut overwritten = false;
        let mut renamed = false;

        let metadata = std::fs::metadata(local)
            .with_context(|| format!("failed to inspect {}", local.display()))?;
        let uploaded = match existing {
            Some(entry) => {
                if entry.kind != EntryKind::File {
                    bail!("remote target `{}` is not a file path", final_remote_path);
                }
                match resolve_conflict_policy(policy, &resolved.path)? {
                    ConflictPolicy::Overwrite => {
                        overwritten = true;
                        let update_link = self
                            .client
                            .get_update_link(&resolved.repo_id, &final_remote_path)?;
                        self.client.update_file(
                            &update_link,
                            local,
                            &final_remote_path,
                            metadata.len(),
                        )?
                    }
                    ConflictPolicy::Rename => {
                        final_target_name = next_available_name(target_name, &entries);
                        final_remote_path = join_remote_path(parent_dir, &final_target_name);
                        renamed = true;
                        let upload_link =
                            self.client.get_upload_link(&resolved.repo_id, parent_dir)?;
                        self.client.upload_file(
                            &resolved.repo_id,
                            &upload_link,
                            local,
                            parent_dir,
                            &final_target_name,
                            false,
                            metadata.len(),
                        )?
                    }
                    ConflictPolicy::Fail | ConflictPolicy::Prompt => {
                        bail!(
                            "remote target `{}` already exists; rerun with --overwrite or --rename",
                            resolved.path
                        );
                    }
                }
            }
            None => {
                let upload_link = self.client.get_upload_link(&resolved.repo_id, parent_dir)?;
                self.client.upload_file(
                    &resolved.repo_id,
                    &upload_link,
                    local,
                    parent_dir,
                    &final_target_name,
                    false,
                    metadata.len(),
                )?
            }
        };

        Ok(PushResult {
            repo: resolved.repo_name,
            remote_path: final_remote_path,
            local_path: local.display().to_string(),
            size: uploaded.size.unwrap_or(metadata.len()),
            overwritten,
            renamed,
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

fn join_remote_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

fn next_available_name(target_name: &str, entries: &[crate::seafile::DirectoryEntry]) -> String {
    let (stem, ext) = split_filename(target_name);
    let mut index = 1usize;
    loop {
        let candidate = if ext.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{ext}")
        };
        if !entries.iter().any(|entry| entry.name == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn split_filename(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext),
        _ => (name, ""),
    }
}

fn resolve_conflict_policy(policy: ConflictPolicy, path: &str) -> Result<ConflictPolicy> {
    if policy != ConflictPolicy::Prompt {
        return Ok(policy);
    }

    let stderr = std::io::stderr();
    if !stderr.is_terminal() {
        bail!(
            "upload to `{path}` requires --overwrite, --rename, or --fail in non-interactive mode"
        );
    }

    let mut stderr = stderr.lock();
    writeln!(
        stderr,
        "Remote file `{path}` already exists. [o]verwrite, [r]ename, [f]ail?"
    )?;
    stderr.flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    match input.trim().to_ascii_lowercase().as_str() {
        "o" | "overwrite" => Ok(ConflictPolicy::Overwrite),
        "r" | "rename" => Ok(ConflictPolicy::Rename),
        "f" | "fail" | "" => Ok(ConflictPolicy::Fail),
        _ => bail!("unrecognized conflict choice"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::PushService;
    use crate::{
        config::{Config, ConfigManager, OutputMode},
        seafile::SeafileClient,
        transfer::ConflictPolicy,
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
                ConflictPolicy::Fail,
            )
            .expect_err("should fail");
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn split_filename_preserves_extension() {
        assert_eq!(super::split_filename("report.pdf"), ("report", "pdf"));
        assert_eq!(super::split_filename("archive"), ("archive", ""));
    }
}
