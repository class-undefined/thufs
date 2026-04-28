use std::{
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{DownloadAuth, SeafileClient},
    transfer::{ConflictPolicy, DownloadMode, ProgressMode},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PullResult {
    pub source: String,
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
        from_share: bool,
        policy: ConflictPolicy,
        download_mode: DownloadMode,
        workers: usize,
        progress_mode: ProgressMode,
    ) -> Result<PullResult> {
        let local_requires_remote_name = local.is_none() || local.is_some_and(|path| path.is_dir());
        let target = if local_requires_remote_name {
            Some(self.resolve_download_target(remote, from_share)?)
        } else {
            None
        };
        let requested_destination = match (local, target.as_ref()) {
            (Some(local), _) => local.to_path_buf(),
            (None, Some(target)) => PathBuf::from(&target.remote_name),
            (None, None) => {
                unreachable!("remote name is required when local destination is omitted")
            }
        };
        let mut destination = match target.as_ref() {
            Some(target) => resolve_local_destination(&requested_destination, &target.remote_name)?,
            None => requested_destination.clone(),
        };
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

        let target = match target {
            Some(target) => target,
            None => self.resolve_download_target(remote, from_share)?,
        };

        let temp_path = temporary_download_path(&destination)?;
        let bytes_written = self.client.download_file(
            &target.download_link,
            &temp_path,
            download_mode,
            workers,
            progress_mode,
            target.auth,
        )?;
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
            source: target.source,
            repo: target.repo,
            remote_path: target.remote_path,
            remote_name: target.remote_name,
            requested_local_path,
            final_local_path: destination.display().to_string(),
            local_path: destination.display().to_string(),
            local_name: local_filename(&destination)?,
            bytes_written,
            overwritten,
            uniquified,
        })
    }

    fn resolve_download_target(
        &self,
        remote: &str,
        from_share: bool,
    ) -> Result<ResolvedDownloadTarget> {
        if let Some(shared) = parse_share_ref(remote, from_share)? {
            let shared_file = self.client.inspect_shared_file(&shared.token)?;
            return Ok(ResolvedDownloadTarget {
                source: format!("share:{}", shared.token),
                repo: "share".to_string(),
                remote_path: format!("/{}", shared_file.file_name),
                remote_name: shared_file.file_name,
                download_link: shared_file.download_link,
                auth: DownloadAuth::Optional,
            });
        }

        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_remote_ref(&remote, &repositories)?;
        let download_link = self
            .client
            .get_download_link(&resolved.repo_id, &resolved.path)?;

        Ok(ResolvedDownloadTarget {
            source: format!("repo:{}{}", resolved.repo_name, resolved.path),
            repo: resolved.repo_name,
            remote_path: resolved.path,
            remote_name: remote_filename(&remote.path)?.to_string(),
            download_link,
            auth: DownloadAuth::Required,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedDownloadTarget {
    source: String,
    repo: String,
    remote_path: String,
    remote_name: String,
    download_link: String,
    auth: DownloadAuth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShareRef {
    token: String,
}

fn parse_share_ref(input: &str, from_share: bool) -> Result<Option<ShareRef>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if let Some(token) = parse_share_url(trimmed)? {
        return Ok(Some(ShareRef { token }));
    }

    if from_share {
        return Ok(Some(ShareRef {
            token: parse_share_token(trimmed)?,
        }));
    }

    Ok(None)
}

fn parse_share_url(input: &str) -> Result<Option<String>> {
    let remainder = input
        .strip_prefix("https://cloud.tsinghua.edu.cn/")
        .or_else(|| input.strip_prefix("http://cloud.tsinghua.edu.cn/"));
    let Some(remainder) = remainder else {
        return Ok(None);
    };

    let remainder = remainder
        .split(['#', '?'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/');
    let mut parts = remainder.split('/');
    let kind = parts.next().unwrap_or_default();
    if kind != "f" && kind != "d" {
        return Ok(None);
    }

    let token = parts.next().unwrap_or_default();
    if token.is_empty() {
        bail!("share link is missing hashcode");
    }

    Ok(Some(parse_share_token(token)?))
}

fn parse_share_token(input: &str) -> Result<String> {
    let token = input
        .split(['?', '#', '/'])
        .next()
        .unwrap_or_default()
        .trim();
    if token.is_empty() {
        bail!("share hashcode cannot be empty");
    }
    if !token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("share hashcode contains unsupported characters");
    }
    Ok(token.to_string())
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
    use std::path::Path;

    use tempfile::tempdir;

    use super::{
        next_available_local_path, parse_share_ref, parse_share_url, resolve_local_destination,
    };

    #[test]
    fn existing_directory_destination_uses_remote_filename() {
        let temp = tempdir().expect("tempdir");
        let destination =
            resolve_local_destination(temp.path(), "/slides/week1.pdf").expect("resolve");
        assert!(destination.ends_with("week1.pdf"));
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

    #[test]
    fn parses_share_file_url() {
        let token =
            parse_share_url("https://cloud.tsinghua.edu.cn/f/abc123XYZ_/").expect("parse url");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn parses_share_url_with_dl_query() {
        let token =
            parse_share_url("https://cloud.tsinghua.edu.cn/f/abc123XYZ_/?dl=1").expect("parse url");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn parses_share_url_without_trailing_slash_but_with_dl_query() {
        let token =
            parse_share_url("https://cloud.tsinghua.edu.cn/f/abc123XYZ_?dl=1").expect("parse");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn parses_http_share_url() {
        let token = parse_share_url("http://cloud.tsinghua.edu.cn/f/abc123XYZ_/").expect("parse");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn parses_directory_share_url() {
        let token =
            parse_share_url("https://cloud.tsinghua.edu.cn/d/abc123XYZ_/?dl=1").expect("parse");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn parses_share_url_with_fragment() {
        let token =
            parse_share_url("https://cloud.tsinghua.edu.cn/f/abc123XYZ_/#/view").expect("parse");
        assert_eq!(token.as_deref(), Some("abc123XYZ_"));
    }

    #[test]
    fn requires_share_flag_for_bare_hashcode() {
        let shared = parse_share_ref("abc123XYZ_", false).expect("parse");
        assert_eq!(shared, None);
    }

    #[test]
    fn accepts_bare_hashcode_with_share_flag() {
        let shared = parse_share_ref("abc123XYZ_", true).expect("parse");
        assert_eq!(shared.expect("shared").token, "abc123XYZ_".to_string());
    }

    #[test]
    fn repo_download_targets_require_authentication() {
        let target = super::ResolvedDownloadTarget {
            source: "repo:course-lib/slides/week1.pdf".to_string(),
            repo: "course-lib".to_string(),
            remote_path: "/slides/week1.pdf".to_string(),
            remote_name: "week1.pdf".to_string(),
            download_link: "https://cloud.tsinghua.edu.cn/seafhttp/files/example".to_string(),
            auth: crate::seafile::DownloadAuth::Required,
        };

        assert_eq!(target.auth, crate::seafile::DownloadAuth::Required);
    }

    #[test]
    fn share_download_targets_use_optional_authentication() {
        let target = super::ResolvedDownloadTarget {
            source: "share:abc123XYZ_".to_string(),
            repo: "share".to_string(),
            remote_path: "/week1.pdf".to_string(),
            remote_name: "week1.pdf".to_string(),
            download_link: "https://cloud.tsinghua.edu.cn/f/abc123XYZ_/?dl=1".to_string(),
            auth: crate::seafile::DownloadAuth::Optional,
        };

        assert_eq!(target.auth, crate::seafile::DownloadAuth::Optional);
    }
}
