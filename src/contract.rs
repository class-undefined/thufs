#![allow(dead_code)]

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
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
}
