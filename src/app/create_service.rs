use anyhow::Result;
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{Repository, SeafileClient},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreateRepoResult {
    pub repo: String,
    pub repo_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreateDirResult {
    pub repo: String,
    pub path: String,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct CreateService {
    config: ConfigManager,
    client: SeafileClient,
}

impl CreateService {
    pub fn new(config: ConfigManager, client: SeafileClient) -> Self {
        Self { config, client }
    }

    pub fn create_repo(&self, name: &str) -> Result<CreateRepoResult> {
        let repositories = self.client.list_repositories()?;
        if let Some(existing) = repositories.iter().find(|repo| repo.name == name) {
            return Ok(CreateRepoResult {
                repo: existing.name.clone(),
                repo_id: existing.id.clone(),
                created: false,
            });
        }

        let created = self.client.create_repository(name)?;
        Ok(CreateRepoResult {
            repo: created.name,
            repo_id: created.id,
            created: true,
        })
    }

    pub fn create_dir(&self, remote: &str) -> Result<CreateDirResult> {
        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = ensure_repository(&self.client, &remote.repo, &repositories)?;
        self.client.ensure_directory(&resolved.id, &remote.path)?;

        Ok(CreateDirResult {
            repo: resolved.name,
            path: remote.path,
            created: true,
        })
    }
}

fn ensure_repository(
    client: &SeafileClient,
    repo_name: &str,
    repositories: &[Repository],
) -> Result<Repository> {
    if let Some(existing) = repositories.iter().find(|repo| repo.name == repo_name) {
        return Ok(existing.clone());
    }

    client.create_repository(repo_name)
}
