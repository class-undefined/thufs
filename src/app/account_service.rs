use anyhow::Result;
use serde::Serialize;

use crate::seafile::SeafileClient;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AccountInfoResult {
    pub login_id: Option<String>,
    pub name: Option<String>,
    pub email: String,
    pub contact_email: Option<String>,
    pub is_staff: Option<bool>,
    pub usage: Option<u64>,
    pub total: Option<u64>,
    pub space_usage: Option<String>,
    pub institution: Option<String>,
    pub department: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepoItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepoListResult {
    pub repos: Vec<RepoItem>,
}

#[derive(Debug, Clone)]
pub struct AccountService {
    client: SeafileClient,
}

impl AccountService {
    pub fn new(client: SeafileClient) -> Self {
        Self { client }
    }

    pub fn account_info(&self) -> Result<AccountInfoResult> {
        let info = self.client.get_account_info()?;
        Ok(AccountInfoResult {
            login_id: info.login_id,
            name: info.name,
            email: info.email,
            contact_email: info.contact_email,
            is_staff: info.is_staff,
            usage: info.usage,
            total: info.total,
            space_usage: info.space_usage,
            institution: info.institution,
            department: info.department,
        })
    }

    pub fn repositories(&self) -> Result<RepoListResult> {
        let mut repos = self
            .client
            .list_repositories()?
            .into_iter()
            .map(|repo| RepoItem {
                id: repo.id,
                name: repo.name,
            })
            .collect::<Vec<_>>();
        repos.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(RepoListResult { repos })
    }

    pub fn format_account_info(info: &AccountInfoResult) -> String {
        [
            format!(
                "Login ID: {}",
                info.login_id.as_deref().unwrap_or("not set")
            ),
            format!("Name: {}", info.name.as_deref().unwrap_or("not set")),
            format!("Email: {}", info.email),
            format!(
                "Contact email: {}",
                info.contact_email.as_deref().unwrap_or("not set")
            ),
            format!(
                "Institution: {}",
                info.institution.as_deref().unwrap_or("not set")
            ),
            format!(
                "Department: {}",
                info.department.as_deref().unwrap_or("not set")
            ),
            format!(
                "Staff: {}",
                info.is_staff
                    .map(|value| if value { "yes" } else { "no" })
                    .unwrap_or("unknown")
            ),
            format!(
                "Usage: {}",
                match (info.usage, info.total, info.space_usage.as_deref()) {
                    (Some(usage), Some(total), Some(space_usage)) => {
                        format!("{usage}/{total} ({space_usage})")
                    }
                    (Some(usage), Some(total), None) => format!("{usage}/{total}"),
                    (Some(usage), None, _) => usage.to_string(),
                    _ => "unknown".to_string(),
                }
            ),
        ]
        .join("\n")
    }

    pub fn format_repositories(result: &RepoListResult) -> String {
        if result.repos.is_empty() {
            return "No repositories found".to_string();
        }

        result
            .repos
            .iter()
            .map(|repo| format!("{}\t{}", repo.name, repo.id))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountInfoResult, AccountService, RepoItem, RepoListResult};
    #[test]
    fn formats_account_info_for_humans() {
        let rendered = AccountService::format_account_info(&AccountInfoResult {
            login_id: Some("user-1".to_string()),
            name: Some("Alice".to_string()),
            email: "alice@example.com".to_string(),
            contact_email: None,
            is_staff: Some(false),
            usage: Some(10),
            total: Some(20),
            space_usage: Some("50%".to_string()),
            institution: None,
            department: Some("CS".to_string()),
        });

        assert!(rendered.contains("Login ID: user-1"));
        assert!(rendered.contains("Email: alice@example.com"));
        assert!(rendered.contains("Usage: 10/20 (50%)"));
    }

    #[test]
    fn formats_repository_list_for_humans() {
        let rendered = AccountService::format_repositories(&RepoListResult {
            repos: vec![RepoItem {
                id: "repo-1".to_string(),
                name: "course-lib".to_string(),
            }],
        });

        assert_eq!(rendered, "course-lib\trepo-1");
    }

    #[test]
    fn empty_repository_list_has_stable_human_output() {
        let rendered = AccountService::format_repositories(&RepoListResult { repos: vec![] });
        assert_eq!(rendered, "No repositories found");
    }
}
