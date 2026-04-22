use anyhow::Result;
use serde::Serialize;

use crate::{
    config::ConfigManager,
    contract::RemoteRef,
    seafile::{DirectoryEntry, EntryKind, SeafileClient},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListItem {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListResult {
    pub repo: String,
    pub path: String,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone)]
pub struct ListService {
    client: SeafileClient,
}

impl ListService {
    pub fn new(client: SeafileClient) -> Self {
        Self { client }
    }

    pub fn list(&self, remote: &str, config: &ConfigManager) -> Result<ListResult> {
        let resolved_config = config.load_resolved()?;
        let remote = RemoteRef::parse_list_target(remote, resolved_config.default_repo.as_deref())?;
        let repositories = self.client.list_repositories()?;
        let resolved = self.client.resolve_list_target(&remote, &repositories)?;
        let entries = self
            .client
            .list_directory_entries(&resolved.repo_id, &resolved.path)?;

        Ok(ListResult {
            repo: resolved.repo_name,
            path: resolved.path,
            items: entries.into_iter().map(ListItem::from).collect(),
        })
    }

    #[cfg(test)]
    pub fn list_with_repositories(
        &self,
        remote: &str,
        config: &ConfigManager,
        repositories: &[crate::seafile::Repository],
        entries: &[DirectoryEntry],
    ) -> Result<ListResult> {
        let resolved_config = config.load_resolved()?;
        let remote = RemoteRef::parse_list_target(remote, resolved_config.default_repo.as_deref())?;
        let resolved = self.client.resolve_list_target(&remote, repositories)?;

        Ok(ListResult {
            repo: resolved.repo_name,
            path: resolved.path,
            items: entries.iter().cloned().map(ListItem::from).collect(),
        })
    }

    pub fn format_human(result: &ListResult) -> String {
        let mut lines = vec![format!("{}{}", result.repo, result.path)];
        for item in &result.items {
            let kind = match item.kind.as_str() {
                "dir" => "d",
                _ => "f",
            };

            let rendered_path = if item.path.starts_with('/') {
                item.path.clone()
            } else {
                format!("/{}", item.path)
            };
            let mut line = format!("{kind} {rendered_path}");
            if let Some(size) = item.size {
                line.push_str(&format!(" {size}"));
            }
            lines.push(line);
        }
        lines.join("\n")
    }
}

impl From<DirectoryEntry> for ListItem {
    fn from(entry: DirectoryEntry) -> Self {
        Self {
            name: entry.name,
            path: entry.path,
            kind: match entry.kind {
                EntryKind::File => "file".to_string(),
                EntryKind::Dir => "dir".to_string(),
            },
            size: entry.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::ListService;
    use crate::{
        config::{Config, ConfigManager, OutputMode},
        seafile::{DirectoryEntry, EntryKind, Repository, SeafileClient},
    };

    fn manager_with_default_repo() -> (tempfile::TempDir, ConfigManager) {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("test-token".to_string()),
                default_repo: Some("course-lib".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");
        (temp, manager)
    }

    #[test]
    fn list_result_resolves_default_repo_shorthand() {
        let (_temp, manager) = manager_with_default_repo();
        let client = SeafileClient::new(manager.clone());
        let service = ListService::new(client);

        let result = service
            .list_with_repositories(
                "slides",
                &manager,
                &[Repository {
                    id: "repo-1".to_string(),
                    name: "course-lib".to_string(),
                }],
                &[DirectoryEntry {
                    name: "week1.pdf".to_string(),
                    path: "/slides/week1.pdf".to_string(),
                    kind: EntryKind::File,
                    size: Some(42),
                }],
            )
            .expect("list");

        assert_eq!(result.repo, "course-lib");
        assert_eq!(result.path, "/slides");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].kind, "file");
    }

    #[test]
    fn list_result_accepts_repo_root_without_default_repo() {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        let client = SeafileClient::new(manager.clone());
        let service = ListService::new(client);

        let result = service
            .list_with_repositories(
                "course-lib",
                &manager,
                &[Repository {
                    id: "repo-1".to_string(),
                    name: "course-lib".to_string(),
                }],
                &[DirectoryEntry {
                    name: "slides".to_string(),
                    path: "/slides".to_string(),
                    kind: EntryKind::Dir,
                    size: None,
                }],
            )
            .expect("list");

        assert_eq!(result.repo, "course-lib");
        assert_eq!(result.path, "/");
        assert_eq!(result.items[0].kind, "dir");
    }

    #[test]
    fn human_render_distinguishes_files_and_directories() {
        let rendered = ListService::format_human(&super::ListResult {
            repo: "course-lib".to_string(),
            path: "/slides".to_string(),
            items: vec![
                super::ListItem {
                    name: "week1".to_string(),
                    path: "/slides/week1".to_string(),
                    kind: "dir".to_string(),
                    size: None,
                },
                super::ListItem {
                    name: "week1.pdf".to_string(),
                    path: "/slides/week1.pdf".to_string(),
                    kind: "file".to_string(),
                    size: Some(42),
                },
            ],
        });

        assert!(rendered.contains("d /slides/week1"));
        assert!(rendered.contains("f /slides/week1.pdf 42"));
    }
}
