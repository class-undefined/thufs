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
    transfer::{ConflictPolicy, ProgressMode},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushResult {
    pub repo: String,
    pub requested_remote_path: String,
    pub final_remote_path: String,
    pub remote_path: String,
    pub local_name: String,
    pub local_path: String,
    pub remote_name: String,
    pub size: u64,
    pub overwritten: bool,
    pub uniquified: bool,
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

    pub fn push(
        &self,
        local: &Path,
        remote: &str,
        policy: ConflictPolicy,
        progress_mode: ProgressMode,
    ) -> Result<PushResult> {
        self.validate_local_source(local)?;

        let resolved_config = self.config.load_resolved()?;
        let remote = parse_upload_target(remote, local, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let repository = repositories
            .iter()
            .find(|repo| repo.name == remote.repo)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| self.client.create_repository(&remote.repo))?;
        let resolved = crate::contract::ResolvedRemoteRef::new(
            repository.name.clone(),
            repository.id.clone(),
            remote.path.clone(),
        );

        let (parent_dir, target_name) = split_parent_and_name(&resolved.path)?;
        self.client
            .ensure_directory(&resolved.repo_id, parent_dir)?;
        let entries = self
            .client
            .list_directory_entries(&resolved.repo_id, parent_dir)?;
        let requested_remote_path = resolved.path.clone();
        let mut final_remote_path = resolved.path.clone();
        let mut final_target_name = target_name.to_string();
        let existing = entries
            .iter()
            .find(|entry| entry.name == target_name)
            .cloned();
        let mut overwritten = false;
        let mut uniquified = false;

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
                            progress_mode,
                        )?
                    }
                    ConflictPolicy::Uniquify => {
                        final_target_name = next_available_name(target_name, &entries);
                        final_remote_path = join_remote_path(parent_dir, &final_target_name);
                        uniquified = true;
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
                            progress_mode,
                        )?
                    }
                    ConflictPolicy::Fail | ConflictPolicy::Prompt => {
                        bail!(
                            "remote target `{}` already exists; rerun with --conflict overwrite, --conflict uniquify, or --conflict fail",
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
                    progress_mode,
                )?
            }
        };

        Ok(PushResult {
            repo: resolved.repo_name,
            requested_remote_path,
            final_remote_path: final_remote_path.clone(),
            remote_path: final_remote_path,
            local_name: file_name(local)?,
            local_path: local.display().to_string(),
            remote_name: final_target_name,
            size: uploaded.size.unwrap_or(metadata.len()),
            overwritten,
            uniquified,
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

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("path `{}` has invalid filename", path.display()))
}

fn parse_upload_target(
    remote: &str,
    local: &Path,
    default_repo: Option<&str>,
) -> Result<RemoteRef> {
    let file_name = local
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!("local source `{}` has invalid filename", local.display())
        })?;

    let trimmed = remote.trim();
    if trimmed.is_empty() {
        bail!("remote path cannot be empty");
    }

    if let Some(explicit_repo) = trimmed.strip_prefix("repo:") {
        let repo = explicit_repo.trim_end_matches('/');
        if repo.is_empty() || repo.contains('/') {
            return RemoteRef::parse(remote, default_repo);
        }
        return Ok(RemoteRef {
            repo: repo.to_string(),
            path: format!("/{file_name}"),
        });
    }

    if trimmed.ends_with('/') {
        if let Some(default_repo) = default_repo.filter(|repo| !repo.trim().is_empty()) {
            let base = trimmed.trim_end_matches('/');
            let path = if base.is_empty() {
                format!("/{file_name}")
            } else {
                format!("/{}/{}", base.trim_start_matches('/'), file_name)
            };
            return Ok(RemoteRef {
                repo: default_repo.to_string(),
                path,
            });
        }
    }

    if let Ok(parsed) = RemoteRef::parse(remote, default_repo) {
        return Ok(parsed);
    }

    if let Some(default_repo) = default_repo.filter(|repo| !repo.trim().is_empty()) {
        let base = trimmed.trim_end_matches('/');
        let path = if base.is_empty() {
            format!("/{file_name}")
        } else {
            format!("/{}/{}", base.trim_start_matches('/'), file_name)
        };
        return Ok(RemoteRef {
            repo: default_repo.to_string(),
            path,
        });
    }

    bail!("remote path must use repo:<library>/<path> when no default repo is configured")
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
            format!("{stem}-({index})")
        } else {
            format!("{stem}-({index}).{ext}")
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
    if stderr.is_terminal() {
        let mut stderr = stderr.lock();
        writeln!(
            stderr,
            "Remote file `{path}` already exists. Defaulting to uniquify; use --conflict to override."
        )?;
        stderr.flush()?;
    }

    Ok(ConflictPolicy::Uniquify)
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
        let (temp, service) = make_service();
        let missing = temp.path().join("missing.txt");
        let err = service
            .push(
                &missing,
                "repo:course-lib/a.txt",
                ConflictPolicy::Fail,
                crate::transfer::ProgressMode::None,
            )
            .expect_err("should fail");
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn split_filename_preserves_extension() {
        assert_eq!(super::split_filename("report.pdf"), ("report", "pdf"));
        assert_eq!(super::split_filename("archive"), ("archive", ""));
    }

    #[test]
    fn next_available_name_uses_dash_number_pattern() {
        let entries = vec![crate::seafile::DirectoryEntry {
            name: "report.pdf".to_string(),
            path: "/report.pdf".to_string(),
            kind: crate::seafile::EntryKind::File,
            size: Some(1),
            updated_at: None,
        }];

        let renamed = super::next_available_name("report.pdf", &entries);
        assert_eq!(renamed, "report-(1).pdf");
    }

    #[test]
    fn prompt_policy_defaults_to_uniquify() {
        let resolved =
            super::resolve_conflict_policy(ConflictPolicy::Prompt, "/report.pdf").expect("resolve");
        assert_eq!(resolved, ConflictPolicy::Uniquify);
    }

    #[test]
    fn upload_target_can_use_repo_root_and_local_filename() {
        let remote = super::parse_upload_target(
            "repo:course-lib",
            PathBuf::from("report.pdf").as_path(),
            None,
        )
        .expect("parse");

        assert_eq!(remote.repo, "course-lib");
        assert_eq!(remote.path, "/report.pdf");
    }

    #[test]
    fn upload_target_can_use_default_repo_directory_and_local_filename() {
        let remote = super::parse_upload_target(
            "submissions/",
            PathBuf::from("report.pdf").as_path(),
            Some("course-lib"),
        )
        .expect("parse");

        assert_eq!(remote.repo, "course-lib");
        assert_eq!(remote.path, "/submissions/report.pdf");
    }
}
