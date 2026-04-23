use anyhow::{Result, bail};
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{SeafileClient, ShareLink, ShareLinkRequest},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareResult {
    pub repo: String,
    pub remote_path: String,
    pub link: String,
    pub url: String,
    pub token: Option<String>,
    pub expires_in_days: Option<u32>,
    pub password_protected: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareItem {
    pub repo: Option<String>,
    pub repo_id: Option<String>,
    pub path: Option<String>,
    pub token: Option<String>,
    pub link: String,
    pub url: String,
    pub is_dir: Option<bool>,
    pub created_at: Option<String>,
    pub expire_date: Option<String>,
    pub view_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareListResult {
    pub repo: Option<String>,
    pub path: Option<String>,
    pub page: usize,
    pub per_page: usize,
    pub total: usize,
    pub has_more: bool,
    pub shares: Vec<ShareItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UnshareResult {
    pub token: String,
    pub removed: bool,
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
        let url = share_url(&created, created.path.as_deref().unwrap_or(&resolved.path));

        Ok(ShareResult {
            repo: resolved.repo_name,
            remote_path: resolved.path,
            link: url.clone(),
            url,
            token: created.token,
            expires_in_days: created.expire_days.or(options.expire_days),
            password_protected: options
                .password
                .as_ref()
                .is_some_and(|password| !password.is_empty()),
        })
    }

    pub fn unshare(&self, token: &str) -> Result<UnshareResult> {
        let token = token.trim();
        if token.is_empty() {
            bail!("share token cannot be empty");
        }

        self.client.delete_share_link(token)?;
        Ok(UnshareResult {
            token: token.to_string(),
            removed: true,
        })
    }

    pub fn list_shares(
        &self,
        remote: Option<&str>,
        page: usize,
        per_page: usize,
        all: bool,
    ) -> Result<ShareListResult> {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let resolved_config = self.config.load_resolved()?;
        let repositories = self.client.list_repositories()?;
        let (repo_name, path, links) = match remote {
            Some(remote) => {
                let remote =
                    RemoteRef::parse_list_target(remote, resolved_config.default_repo.as_deref())?;
                let resolved = self.client.resolve_list_target(&remote, &repositories)?;
                let links = self
                    .client
                    .list_share_links(&resolved.repo_id, Some(&resolved.path))?;
                (Some(resolved.repo_name), Some(resolved.path), links)
            }
            None => (None, None, self.client.list_all_share_links()?),
        };

        let total = links.len();
        let start = if all { 0 } else { (page - 1) * per_page };
        let end = if all {
            total
        } else {
            (start + per_page).min(total)
        };
        let has_more = !all && end < total;
        let page_links = if start >= total {
            Vec::new()
        } else {
            links[start..end].to_vec()
        };

        let shares = page_links
            .into_iter()
            .map(|share| ShareItem {
                repo: share.repo_name.clone().or_else(|| repo_name.clone()),
                repo_id: share.repo_id.clone(),
                path: share.path.clone(),
                token: share.token.clone(),
                link: share_url(&share, share.path.as_deref().unwrap_or("/")),
                url: share_url(&share, share.path.as_deref().unwrap_or("/")),
                is_dir: share.is_dir,
                created_at: share.ctime,
                expire_date: share.expire_date,
                view_count: share.view_cnt,
            })
            .collect();

        Ok(ShareListResult {
            repo: repo_name,
            path,
            page,
            per_page: if all { total.max(1) } else { per_page },
            total,
            has_more,
            shares,
        })
    }
}

fn share_url(share: &ShareLink, path: &str) -> String {
    if let Some(link) = &share.link {
        if !link.is_empty() {
            return link.clone();
        }
    }

    let token = share.token.as_deref().unwrap_or_default();
    let prefix = if share.is_dir.unwrap_or_else(|| path.ends_with('/')) {
        "d"
    } else {
        "f"
    };
    format!("https://cloud.tsinghua.edu.cn/{prefix}/{token}/")
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
