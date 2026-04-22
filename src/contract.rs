#![allow(dead_code)]

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRef {
    pub repo: String,
    pub path: String,
}

impl RemoteRef {
    pub fn parse(input: &str, default_repo: Option<&str>) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            bail!("remote path cannot be empty");
        }

        if let Some(explicit) = trimmed.strip_prefix("repo:") {
            let (repo, path) = split_repo_and_path(explicit)?;
            return Ok(Self {
                repo: repo.to_string(),
                path: normalize_remote_path(path),
            });
        }

        if let Some(repo) = default_repo.filter(|repo| !repo.trim().is_empty()) {
            return Ok(Self {
                repo: repo.to_string(),
                path: normalize_remote_path(trimmed),
            });
        }

        bail!("remote path must use repo:<library>/<path> when no default repo is configured")
    }

    pub fn parse_list_target(input: &str, default_repo: Option<&str>) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            bail!("remote path cannot be empty");
        }

        if let Some(explicit) = trimmed.strip_prefix("repo:") {
            if !explicit.contains('/') {
                let repo = explicit.trim();
                if repo.is_empty() {
                    bail!("remote path is missing repo name");
                }
                return Ok(Self {
                    repo: repo.to_string(),
                    path: "/".to_string(),
                });
            }

            return Self::parse(trimmed, default_repo);
        }

        if default_repo.is_none() && !trimmed.contains('/') {
            return Ok(Self {
                repo: trimmed.to_string(),
                path: "/".to_string(),
            });
        }

        Self::parse(trimmed, default_repo)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRemoteRef {
    pub repo_name: String,
    pub repo_id: String,
    pub path: String,
}

impl ResolvedRemoteRef {
    pub fn new(
        repo_name: impl Into<String>,
        repo_id: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            repo_name: repo_name.into(),
            repo_id: repo_id.into(),
            path: path.into(),
        }
    }
}

fn split_repo_and_path(input: &str) -> Result<(&str, &str)> {
    let mut parts = input.splitn(2, '/');
    let repo = parts.next().unwrap_or_default().trim();
    let path = parts.next().unwrap_or_default().trim();

    if repo.is_empty() {
        bail!("remote path is missing repo name");
    }
    if path.is_empty() {
        bail!("remote path is missing file or directory path");
    }

    Ok((repo, path))
}

fn normalize_remote_path(path: &str) -> String {
    let collapsed = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/");

    format!("/{}", collapsed)
}

#[cfg(test)]
mod tests {
    use super::RemoteRef;

    #[test]
    fn explicit_remote_paths_work_without_default_repo() {
        let remote = RemoteRef::parse("repo:course-lib/slides/week1.pdf", None).expect("parse");

        assert_eq!(remote.repo, "course-lib");
        assert_eq!(remote.path, "/slides/week1.pdf");
    }

    #[test]
    fn shorthand_paths_require_default_repo() {
        let remote = RemoteRef::parse("notes/todo.md", Some("default-lib")).expect("parse");

        assert_eq!(remote.repo, "default-lib");
        assert_eq!(remote.path, "/notes/todo.md");
    }

    #[test]
    fn shorthand_paths_fail_without_default_repo() {
        let err = RemoteRef::parse("notes/todo.md", None).expect_err("should fail");
        assert!(
            err.to_string().contains("remote path must use repo:"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn normalizes_redundant_path_segments() {
        let remote = RemoteRef::parse("repo:course-lib//slides/./week1.pdf", None).expect("parse");
        assert_eq!(remote.path, "/slides/week1.pdf");
    }

    #[test]
    fn list_target_accepts_repo_root_without_default_repo() {
        let remote = RemoteRef::parse_list_target("course-lib", None).expect("parse");
        assert_eq!(remote.repo, "course-lib");
        assert_eq!(remote.path, "/");
    }

    #[test]
    fn list_target_accepts_explicit_repo_root() {
        let remote = RemoteRef::parse_list_target("repo:course-lib", None).expect("parse");
        assert_eq!(remote.repo, "course-lib");
        assert_eq!(remote.path, "/");
    }
}
