use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{fs, io::AsyncWriteExt, runtime::Runtime};

use crate::{
    config::ConfigManager,
    contract::{RemoteRef, ResolvedRemoteRef},
    transfer::create_progress_bar,
};

const THU_CLOUD_BASE_URL: &str = "https://cloud.tsinghua.edu.cn";

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountInfo {
    #[serde(default)]
    pub login_id: Option<String>,
    #[serde(default)]
    pub is_staff: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    pub email: String,
    #[serde(default)]
    pub contact_email: Option<String>,
    #[serde(default)]
    pub institution: Option<String>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub space_usage: Option<String>,
    #[serde(default)]
    pub usage: Option<u64>,
    #[serde(default)]
    pub total: Option<u64>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadedFile {
    pub name: String,
    pub id: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLinkRequest {
    pub repo_id: String,
    pub path: String,
    pub password: Option<String>,
    pub expire_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareLink {
    pub link: String,
    pub token: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub expire_days: Option<u32>,
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

    pub fn base_url(&self) -> &str {
        THU_CLOUD_BASE_URL
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

    pub fn list_repositories(&self) -> Result<Vec<Repository>> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let response = self
                .http
                .get(format!("{}/api2/repos/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .send()
                .await
                .context("failed to request repository list")?
                .error_for_status()
                .context("repository list request failed")?;

            response
                .json::<Vec<Repository>>()
                .await
                .context("failed to parse repository list")
        })
    }

    pub fn get_account_info(&self) -> Result<AccountInfo> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let response = self
                .http
                .get(format!("{}/api2/account/info/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .send()
                .await
                .context("failed to request account info")?
                .error_for_status()
                .context("account info request failed")?;

            response
                .json::<AccountInfo>()
                .await
                .context("failed to parse account info")
        })
    }

    pub fn list_directory_entries(&self, repo_id: &str, path: &str) -> Result<Vec<DirectoryEntry>> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let response = self
                .http
                .get(format!("{}/api2/repos/{repo_id}/dir/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .query(&[("p", path)])
                .send()
                .await
                .context("failed to request directory entries")?
                .error_for_status()
                .context("directory listing request failed")?;

            let payload = response
                .json::<Value>()
                .await
                .context("failed to decode directory listing")?;
            parse_directory_entries(path, payload)
        })
    }

    pub fn get_upload_link(&self, repo_id: &str, parent_dir: &str) -> Result<String> {
        self.get_text_endpoint(
            &format!("{}/api2/repos/{repo_id}/upload-link/", self.base_url()),
            &[("p", parent_dir)],
        )
    }

    pub fn get_update_link(&self, repo_id: &str, path: &str) -> Result<String> {
        self.get_text_endpoint(
            &format!("{}/api2/repos/{repo_id}/update-link/", self.base_url()),
            &[("p", path)],
        )
    }

    #[allow(dead_code)]
    pub fn get_download_link(&self, repo_id: &str, path: &str) -> Result<String> {
        self.get_text_endpoint(
            &format!("{}/api2/repos/{repo_id}/file/", self.base_url()),
            &[("p", path)],
        )
    }

    pub fn upload_file(
        &self,
        repo_id: &str,
        upload_link: &str,
        local_path: &Path,
        parent_dir: &str,
        target_name: &str,
        replace: bool,
        total_bytes: u64,
    ) -> Result<UploadedFile> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let uploaded_bytes = self
                .uploaded_bytes(repo_id, parent_dir, target_name)
                .await
                .unwrap_or(0);
            let content = fs::read(local_path)
                .await
                .with_context(|| format!("failed to read {}", local_path.display()))?;
            let start = uploaded_bytes.min(content.len() as u64) as usize;

            let progress = create_progress_bar(Some(total_bytes));
            if let Some(progress) = &progress {
                progress.set_position(uploaded_bytes);
                progress.set_message(format!("upload {}", local_path.display()));
            }

            let response = if uploaded_bytes > 0 {
                let part = reqwest::multipart::Part::bytes(content[start..].to_vec())
                    .file_name(target_name.to_string());
                let form = reqwest::multipart::Form::new()
                    .part("file", part)
                    .text("parent_dir", parent_dir.to_string())
                    .text("replace", if replace { "1" } else { "0" }.to_string());

                self.http
                    .post(format!("{upload_link}?ret-json=1"))
                    .header(header::AUTHORIZATION, self.auth_header_value()?)
                    .header(
                        "Content-Range",
                        format!(
                            "bytes {}-{}/{}",
                            start,
                            content.len().saturating_sub(1),
                            content.len()
                        ),
                    )
                    .header(
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{target_name}\""),
                    )
                    .multipart(form)
                    .send()
                    .await
                    .context("failed to resume upload")?
                    .error_for_status()
                    .context("resumable upload request failed")?
            } else {
                let part =
                    reqwest::multipart::Part::bytes(content).file_name(target_name.to_string());
                let form = reqwest::multipart::Form::new()
                    .part("file", part)
                    .text("parent_dir", parent_dir.to_string())
                    .text("replace", if replace { "1" } else { "0" }.to_string());

                self.http
                    .post(format!("{upload_link}?ret-json=1"))
                    .header(header::AUTHORIZATION, self.auth_header_value()?)
                    .multipart(form)
                    .send()
                    .await
                    .context("failed to upload file")?
                    .error_for_status()
                    .context("upload request failed")?
            };

            if let Some(progress) = &progress {
                progress.finish_with_message(format!("upload {} complete", local_path.display()));
            }

            let uploaded = response
                .json::<Vec<UploadedFile>>()
                .await
                .context("failed to parse upload response")?;
            uploaded
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("upload response did not include uploaded file metadata"))
        })
    }

    pub fn update_file(
        &self,
        update_link: &str,
        local_path: &Path,
        target_file: &str,
        total_bytes: u64,
    ) -> Result<UploadedFile> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let content = fs::read(local_path)
                .await
                .with_context(|| format!("failed to read {}", local_path.display()))?;
            let file_name = local_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("upload.bin")
                .to_string();
            let part = reqwest::multipart::Part::bytes(content).file_name(file_name);
            let form = reqwest::multipart::Form::new()
                .part("file", part)
                .text("target_file", target_file.to_string());

            let progress = create_progress_bar(Some(total_bytes));
            if let Some(progress) = &progress {
                progress.set_position(0);
                progress.set_message(format!("upload {}", local_path.display()));
            }

            let response = self
                .http
                .post(update_link)
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .multipart(form)
                .send()
                .await
                .context("failed to update file")?
                .error_for_status()
                .context("update request failed")?;

            if let Some(progress) = &progress {
                progress.finish_with_message(format!("upload {} complete", local_path.display()));
            }

            let uploaded = response
                .json::<Vec<UploadedFile>>()
                .await
                .context("failed to parse update response")?;
            uploaded
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("update response did not include file metadata"))
        })
    }

    #[allow(dead_code)]
    pub fn download_file(&self, download_link: &str, destination: &Path) -> Result<u64> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let existing_bytes = fs::metadata(destination)
                .await
                .map(|meta| meta.len())
                .unwrap_or(0);
            let response = self
                .http
                .get(download_link)
                .header(header::RANGE, format!("bytes={existing_bytes}-"))
                .send()
                .await
                .context("failed to download file body")?
                .error_for_status()
                .context("download request failed")?;

            let total = response.content_length().map(|len| len + existing_bytes);
            let progress = create_progress_bar(total);
            if let Some(progress) = &progress {
                progress.set_position(existing_bytes);
                progress.set_message(format!("download {}", destination.display()));
            }

            let mut stream = response.bytes_stream();
            let mut file = if existing_bytes > 0 {
                fs::OpenOptions::new()
                    .append(true)
                    .open(destination)
                    .await
                    .with_context(|| format!("failed to open {}", destination.display()))?
            } else {
                fs::File::create(destination)
                    .await
                    .with_context(|| format!("failed to create {}", destination.display()))?
            };

            let mut written = existing_bytes;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context("failed to read download body")?;
                file.write_all(&chunk)
                    .await
                    .with_context(|| format!("failed to write {}", destination.display()))?;
                written += chunk.len() as u64;
                if let Some(progress) = &progress {
                    progress.set_position(written);
                }
            }

            file.flush()
                .await
                .with_context(|| format!("failed to flush {}", destination.display()))?;
            if let Some(progress) = &progress {
                progress
                    .finish_with_message(format!("download {} complete", destination.display()));
            }
            Ok(written)
        })
    }

    async fn uploaded_bytes(
        &self,
        repo_id: &str,
        parent_dir: &str,
        file_name: &str,
    ) -> Result<u64> {
        let response = self
            .http
            .get(format!(
                "{}/api/v2.1/repos/{repo_id}/file-uploaded-bytes/",
                self.base_url()
            ))
            .header(header::AUTHORIZATION, self.auth_header_value()?)
            .query(&[("parent_dir", parent_dir), ("file_name", file_name)])
            .send()
            .await
            .context("failed to inspect uploaded bytes")?
            .error_for_status()
            .context("uploaded-bytes request failed")?;

        let payload = response
            .json::<Value>()
            .await
            .context("failed to parse uploaded-bytes response")?;

        Ok(payload
            .get("uploadedBytes")
            .and_then(Value::as_u64)
            .unwrap_or(0))
    }

    pub fn create_share_link(&self, request: ShareLinkRequest) -> Result<ShareLink> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let mut form = vec![("repo_id", request.repo_id), ("path", request.path)];

            if let Some(password) = request.password {
                form.push(("password", password));
            }
            if let Some(expire_days) = request.expire_days {
                form.push(("expire_days", expire_days.to_string()));
            }

            self.http
                .post(format!("{}/api/v2.1/share-links/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .form(&form)
                .send()
                .await
                .context("failed to create share link")?
                .error_for_status()
                .context("share link request failed")?
                .json::<ShareLink>()
                .await
                .context("failed to parse share link response")
        })
    }

    fn get_text_endpoint(&self, url: &str, query: &[(&str, &str)]) -> Result<String> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let response = self
                .http
                .get(url)
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .query(query)
                .send()
                .await
                .with_context(|| format!("failed to request {url}"))?
                .error_for_status()
                .with_context(|| format!("request failed for {url}"))?;

            let text = response
                .text()
                .await
                .context("failed to read text response")?;
            Ok(text.trim().trim_matches('"').to_string())
        })
    }
}

fn parse_directory_entries(dir_path: &str, payload: Value) -> Result<Vec<DirectoryEntry>> {
    let items = payload
        .as_array()
        .ok_or_else(|| anyhow!("unexpected directory listing payload"))?;

    items
        .iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("directory entry missing name"))?
                .to_string();
            let kind = match item.get("type").and_then(Value::as_str).unwrap_or("file") {
                "dir" => EntryKind::Dir,
                _ => EntryKind::File,
            };
            let size = item.get("size").and_then(Value::as_u64);
            let full_path = if dir_path == "/" {
                format!("/{name}")
            } else {
                format!("{}/{}", dir_path.trim_end_matches('/'), name)
            };

            Ok(DirectoryEntry {
                name,
                path: full_path,
                kind,
                size,
            })
        })
        .collect()
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
