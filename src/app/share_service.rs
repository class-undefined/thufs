use anyhow::{Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{SeafileClient, ShareLinkRequest},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareResult {
    pub repo: String,
    pub remote_path: String,
    pub link: String,
    pub token: Option<String>,
    pub expires_in_days: Option<u32>,
    pub password_protected: bool,
}

#[derive(Debug, Clone)]
pub struct ShareOptions {
    pub password: Option<String>,
    pub expire_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ShareService {
    config: ConfigManager,
    client: SeafileClient,
}

impl ShareService {
    pub fn new(config: ConfigManager, client: SeafileClient) -> Self {
        Self { config, client }
    }

    pub fn share(&self, remote: &str, options: ShareOptions) -> Result<ShareResult> {
        validate_options(&options)?;

        let resolved_config = self.config.load_resolved()?;
        let remote = RemoteRef::parse(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_remote_ref(&remote, &repositories)?;

        let created = self.client.create_share_link(ShareLinkRequest {
            repo_id: resolved.repo_id,
            path: resolved.path.clone(),
            password: options.password.clone(),
            expire_days: options.expire_days,
        })?;

        Ok(ShareResult {
            repo: resolved.repo_name,
            remote_path: resolved.path,
            link: created.link,
            token: created.token,
            expires_in_days: created.expire_days.or(options.expire_days),
            password_protected: options
                .password
                .as_ref()
                .is_some_and(|password| !password.is_empty()),
        })
    }
}

fn validate_options(options: &ShareOptions) -> Result<()> {
    if options
        .password
        .as_ref()
        .is_some_and(|password| password.is_empty())
    {
        bail!("share password cannot be empty");
    }
    if options.expire_days == Some(0) {
        bail!("share expiration must be at least 1 day");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ShareOptions, validate_options};

    #[test]
    fn rejects_zero_day_expiration() {
        let err = validate_options(&ShareOptions {
            password: None,
            expire_days: Some(0),
        })
        .expect_err("should fail");
        assert!(err.to_string().contains("at least 1 day"));
    }

    #[test]
    fn rejects_empty_password() {
        let err = validate_options(&ShareOptions {
            password: Some(String::new()),
            expire_days: None,
        })
        .expect_err("should fail");
        assert!(err.to_string().contains("cannot be empty"));
    }
}
