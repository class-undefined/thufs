use std::{
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::SeafileClient,
    transfer::{ConflictPolicy, DownloadMode},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PullResult {
    pub repo: String,
    pub requested_local_path: String,
    pub final_local_path: String,
    pub remote_path: String,
    pub remote_name: String,
    pub local_path: String,
    pub local_name: String,
    pub bytes_written: u64,
    pub overwritten: bool,
    pub uniquified: bool,
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

    pub fn pull(
        &self,
        remote: &str,
        local: Option<&Path>,
        policy: ConflictPolicy,
        download_mode: DownloadMode,
        workers: usize,
    ) -> Result<PullResult> {
        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let requested_destination = resolve_requested_destination(local, &remote.path)?;
        let mut destination = resolve_local_destination(&requested_destination, &remote.path)?;
        let requested_local_path = destination.display().to_string();
        let mut overwritten = false;
        let mut uniquified = false;
        if destination.exists() {
            match resolve_conflict_policy(policy, &destination)? {
                ConflictPolicy::Overwrite => overwritten = true,
                ConflictPolicy::Uniquify => {
                    destination = next_available_local_path(&destination);
                    uniquified = true;
                }
                ConflictPolicy::Fail | ConflictPolicy::Prompt => {
                    bail!(
                        "local destination `{}` already exists; rerun with --conflict overwrite, --conflict uniquify, or --conflict fail",
                        destination.display()
                    );
                }
            }
        }

        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
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
        let bytes_written =
            self.client
                .download_file(&download_link, &temp_path, download_mode, workers)?;
        if overwritten && destination.exists() {
            std::fs::remove_file(&destination).with_context(|| {
                format!(
                    "failed to remove existing destination before overwrite: {}",
                    destination.display()
                )
            })?;
        }
        std::fs::rename(&temp_path, &destination).with_context(|| {
            format!(
                "failed to move downloaded file into place: {}",
                destination.display()
            )
        })?;

        Ok(PullResult {
            repo: resolved.repo_name,
            remote_path: resolved.path,
            remote_name: remote_filename(&remote.path)?.to_string(),
            requested_local_path,
            final_local_path: destination.display().to_string(),
            local_path: destination.display().to_string(),
            local_name: local_filename(&destination)?,
            bytes_written,
            overwritten,
            uniquified,
        })
    }
}

fn resolve_requested_destination(local: Option<&Path>, remote_path: &str) -> Result<PathBuf> {
    match local {
        Some(local) => Ok(local.to_path_buf()),
        None => {
            let name = remote_filename(remote_path)?;
            Ok(PathBuf::from(name))
        }
    }
}

fn resolve_local_destination(local: &Path, remote_path: &str) -> Result<PathBuf> {
    if local.exists() && local.is_dir() {
        let name = remote_filename(remote_path)?;
        return Ok(local.join(name));
    }
    Ok(local.to_path_buf())
}

fn remote_filename(remote_path: &str) -> Result<&str> {
    remote_path
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| anyhow::anyhow!("remote path `{remote_path}` must point to a file"))
}

fn local_filename(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .ok_or_else(|| anyhow::anyhow!("local path `{}` has invalid filename", path.display()))
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
    Ok(parent.join(format!(".{name}.thufs-part")))
}

fn next_available_local_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let (stem, ext) = split_filename(name);

    let mut index = 1usize;
    loop {
        let candidate = if ext.is_empty() {
            format!("{stem}-({index})")
        } else {
            format!("{stem}-({index}).{ext}")
        };
        let candidate_path = parent.join(candidate);
        if !candidate_path.exists() {
            return candidate_path;
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

fn resolve_conflict_policy(policy: ConflictPolicy, path: &Path) -> Result<ConflictPolicy> {
    if policy != ConflictPolicy::Prompt {
        return Ok(policy);
    }

    let stderr = std::io::stderr();
    if stderr.is_terminal() {
        let mut stderr = stderr.lock();
        writeln!(
            stderr,
            "Local file `{}` already exists. Defaulting to uniquify; use --conflict to override.",
            path.display()
        )?;
        stderr.flush()?;
    }

    Ok(ConflictPolicy::Uniquify)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        next_available_local_path, resolve_local_destination, resolve_requested_destination,
    };

    #[test]
    fn existing_directory_destination_uses_remote_filename() {
        let temp = tempdir().expect("tempdir");
        let destination =
            resolve_local_destination(temp.path(), "/slides/week1.pdf").expect("resolve");
        assert!(destination.ends_with("week1.pdf"));
    }

    #[test]
    fn missing_local_destination_defaults_to_remote_filename() {
        let destination =
            resolve_requested_destination(None, "/slides/week1.pdf").expect("resolve");
        assert_eq!(destination, PathBuf::from("week1.pdf"));
    }

    #[test]
    fn rename_strategy_picks_unique_local_name() {
        let temp = tempdir().expect("tempdir");
        let existing = temp.path().join("week1.pdf");
        std::fs::write(&existing, "data").expect("write");
        let renamed = next_available_local_path(&existing);
        assert!(renamed.ends_with("week1-(1).pdf"));
    }

    #[test]
    fn prompt_policy_defaults_to_uniquify() {
        let resolved = super::resolve_conflict_policy(
            crate::transfer::ConflictPolicy::Prompt,
            Path::new("week1.pdf"),
        )
        .expect("resolve");
        assert_eq!(resolved, crate::transfer::ConflictPolicy::Uniquify);
    }
}
