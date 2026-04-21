use anyhow::{Context, Result, anyhow, bail};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};

use crate::{
    config::ConfigManager,
    contract::{RemoteRef, ResolvedRemoteRef},
};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedListTarget {
    pub repo_name: String,
    pub repo_id: String,
    pub path: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SeafileClient {
    config: ConfigManager,
    http: Client,
}

impl SeafileClient {
    pub fn new(config: ConfigManager) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn http_client(&self) -> &Client {
        &self.http
    }

    pub fn auth_header_value(&self) -> Result<header::HeaderValue> {
        let config = self.config.load_resolved()?;
        let token = config
            .token
            .ok_or_else(|| anyhow!("no token configured; run `thufs auth set-token <token>`"))?;

        let value = format!("Token {token}");
        let mut header = header::HeaderValue::from_str(&value)
            .context("failed to build Authorization header")?;
        header.set_sensitive(true);
        Ok(header)
    }

    pub fn resolve_remote_ref(
        &self,
        remote: &RemoteRef,
        repos: &[Repository],
    ) -> Result<ResolvedRemoteRef> {
        let repo = self.find_repository(&remote.repo, repos)?;
        Ok(ResolvedRemoteRef::new(
            repo.name.clone(),
            repo.id.clone(),
            remote.path.clone(),
        ))
    }

    pub fn resolve_list_target(
        &self,
        remote: &RemoteRef,
        repos: &[Repository],
    ) -> Result<ResolvedListTarget> {
        let resolved = self.resolve_remote_ref(remote, repos)?;
        Ok(ResolvedListTarget {
            repo_name: resolved.repo_name,
            repo_id: resolved.repo_id,
            path: resolved.path,
        })
    }

    pub fn find_repository<'a>(
        &self,
        repo_name: &str,
        repos: &'a [Repository],
    ) -> Result<&'a Repository> {
        let exact_matches = repos
            .iter()
            .filter(|repo| repo.name == repo_name)
            .collect::<Vec<_>>();

        match exact_matches.as_slice() {
            [repo] => Ok(*repo),
            [] => bail!("repository `{repo_name}` was not found"),
            _ => bail!("repository `{repo_name}` is ambiguous"),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{Repository, SeafileClient};
    use crate::{
        config::{Config, ConfigManager, OutputMode},
        contract::RemoteRef,
    };

    fn configured_client() -> (tempfile::TempDir, SeafileClient) {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("test-token".to_string()),
                default_repo: Some("course-lib".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");
        (temp, SeafileClient::new(manager))
    }

    #[test]
    fn builds_token_authorization_header() {
        let (_temp, client) = configured_client();
        let header = client.auth_header_value().expect("header");
        assert_eq!(header.to_str().expect("str"), "Token test-token");
    }

    #[test]
    fn resolves_repo_name_to_repo_id() {
        let (_temp, client) = configured_client();
        let remote = RemoteRef::parse("repo:course-lib/slides/week1.pdf", None).expect("parse");
        let resolved = client
            .resolve_remote_ref(
                &remote,
                &[Repository {
                    id: "repo-id-1".to_string(),
                    name: "course-lib".to_string(),
                }],
            )
            .expect("resolve");

        assert_eq!(resolved.repo_name, "course-lib");
        assert_eq!(resolved.repo_id, "repo-id-1");
        assert_eq!(resolved.path, "/slides/week1.pdf");
    }

    #[test]
    fn missing_repository_fails_explicitly() {
        let (_temp, client) = configured_client();
        let err = client
            .find_repository("missing", &[])
            .expect_err("should fail");
        assert!(err.to_string().contains("was not found"));
    }

    #[test]
    fn duplicate_repository_names_fail_explicitly() {
        let (_temp, client) = configured_client();
        let err = client
            .find_repository(
                "dup",
                &[
                    Repository {
                        id: "1".to_string(),
                        name: "dup".to_string(),
                    },
                    Repository {
                        id: "2".to_string(),
                        name: "dup".to_string(),
                    },
                ],
            )
            .expect_err("should fail");
        assert!(err.to_string().contains("ambiguous"));
    }
}
