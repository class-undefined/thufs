use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use futures_util::future::try_join_all;
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
    runtime::Runtime,
};

use crate::{
    config::ConfigManager,
    contract::{RemoteRef, ResolvedRemoteRef},
    transfer::{DownloadMode, create_progress_bar},
};

const THU_CLOUD_BASE_URL: &str = "https://cloud.tsinghua.edu.cn";
const PARALLEL_DOWNLOAD_THRESHOLD: u64 = 8 * 1024 * 1024;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mtime: Option<i64>,
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
    pub updated_at: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreatedRepository {
    pub repo_id: String,
    pub repo_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileDetail {
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub mtime: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DirectoryDetail {
    #[serde(default)]
    pub mtime: Option<String>,
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
    #[serde(default)]
    pub link: Option<String>,
    pub token: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub repo_id: Option<String>,
    #[serde(default)]
    pub repo_name: Option<String>,
    #[serde(default)]
    pub obj_name: Option<String>,
    #[serde(default)]
    pub is_dir: Option<bool>,
    #[serde(default)]
    pub ctime: Option<String>,
    #[serde(default)]
    pub expire_date: Option<String>,
    #[serde(default)]
    pub view_cnt: Option<u64>,
    #[serde(default)]
    pub expire_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DownloadSupport {
    total_bytes: u64,
    accepts_ranges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedFileDownload {
    pub token: String,
    pub file_name: String,
    pub download_link: String,
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

    pub fn create_repository(&self, name: &str) -> Result<Repository> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let created = self
                .http
                .post(format!("{}/api2/repos/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .form(&[("name", name)])
                .send()
                .await
                .context("failed to create repository")?
                .error_for_status()
                .context("repository creation failed")?
                .json::<CreatedRepository>()
                .await
                .context("failed to parse repository creation response")?;

            Ok(Repository {
                id: created.repo_id,
                name: created.repo_name,
                mtime: None,
            })
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
        self.list_directory_entries_with_time(repo_id, path, true)
    }

    pub fn list_directory_entries_with_time(
        &self,
        repo_id: &str,
        path: &str,
        show_time: bool,
    ) -> Result<Vec<DirectoryEntry>> {
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
            let mut entries = parse_directory_entries(path, payload)?;
            if show_time {
                for entry in &mut entries {
                    entry.updated_at = match entry.kind {
                        EntryKind::File => self.get_file_updated_at(repo_id, &entry.path).await?,
                        EntryKind::Dir => self.get_dir_updated_at(repo_id, &entry.path).await?,
                    };
                }
            }
            Ok(entries)
        })
    }

    pub fn ensure_directory(&self, repo_id: &str, path: &str) -> Result<()> {
        if path == "/" || path.trim().is_empty() {
            return Ok(());
        }

        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            self.http
                .post(format!("{}/api2/repos/{repo_id}/dir/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .query(&[("p", path)])
                .form(&[("operation", "mkdir"), ("create_parents", "true")])
                .send()
                .await
                .context("failed to create remote directory")?
                .error_for_status()
                .context("remote directory creation failed")?;
            Ok(())
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

    pub fn inspect_shared_file(&self, token: &str) -> Result<SharedFileDownload> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let download_link = format!("{}/f/{token}/?dl=1", self.base_url());
            let response = self
                .http
                .get(&download_link)
                .header(header::RANGE, "bytes=0-0")
                .send()
                .await
                .context("failed to inspect shared file")?;

            let status = response.status();
            if status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
                response
                    .error_for_status()
                    .context("shared file request failed")?;
                bail!("shared file request returned unexpected status {status}");
            }

            let file_name = response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_filename_from_content_disposition)
                .ok_or_else(|| {
                    anyhow!(
                        "failed to infer shared filename from server response; the share may require a password or may not point to a file"
                    )
                })?;

            Ok(SharedFileDownload {
                token: token.to_string(),
                file_name,
                download_link,
            })
        })
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
    pub fn download_file(
        &self,
        download_link: &str,
        destination: &Path,
        mode: DownloadMode,
        workers: usize,
    ) -> Result<u64> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let existing_bytes = fs::metadata(destination)
                .await
                .map(|meta| meta.len())
                .unwrap_or(0);
            let workers = workers.max(1);

            match mode {
                DownloadMode::Sequential => {}
                DownloadMode::Auto | DownloadMode::Parallel => {
                    if existing_bytes == 0 {
                        if let Some(support) = self.probe_download_support(download_link).await? {
                            if support.accepts_ranges
                                && support.total_bytes >= PARALLEL_DOWNLOAD_THRESHOLD
                            {
                                return self
                                    .download_file_parallel(
                                        download_link,
                                        destination,
                                        support.total_bytes,
                                        workers,
                                    )
                                    .await;
                            }
                            if mode == DownloadMode::Parallel && !support.accepts_ranges {
                                bail!(
                                    "parallel download was requested but the remote endpoint does not support ranged download"
                                );
                            }
                        }
                    } else if mode == DownloadMode::Parallel {
                        bail!(
                            "parallel download was requested but resumable partial files must continue in sequential mode"
                        );
                    }
                }
            }

            self.download_file_sequential(download_link, destination, existing_bytes)
                .await
        })
    }

    async fn probe_download_support(&self, download_link: &str) -> Result<Option<DownloadSupport>> {
        let response = self
            .http
            .get(download_link)
            .header(header::RANGE, "bytes=0-0")
            .send()
            .await
            .context("failed to probe download range support")?;

        if response.status() == StatusCode::PARTIAL_CONTENT {
            let total_bytes = response
                .headers()
                .get(header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_total_bytes_from_content_range)
                .or_else(|| response.content_length())
                .ok_or_else(|| anyhow!("range probe did not include total content length"))?;

            return Ok(Some(DownloadSupport {
                total_bytes,
                accepts_ranges: true,
            }));
        }

        if response.status().is_success() {
            let total_bytes = response.content_length().unwrap_or(0);
            return Ok(Some(DownloadSupport {
                total_bytes,
                accepts_ranges: false,
            }));
        }

        response
            .error_for_status()
            .context("download capability probe failed")?;
        Ok(None)
    }

    async fn download_file_parallel(
        &self,
        download_link: &str,
        destination: &Path,
        total_bytes: u64,
        requested_parts: usize,
    ) -> Result<u64> {
        let ranges = split_download_ranges(total_bytes, requested_parts.max(1));
        if ranges.len() <= 1 {
            return self
                .download_file_sequential(download_link, destination, 0)
                .await;
        }

        let progress = create_progress_bar(Some(total_bytes));
        if let Some(progress) = &progress {
            progress.set_position(0);
            progress.set_message(format!("download {}", destination.display()));
        }

        let tasks = ranges.iter().enumerate().map(|(index, (start, end))| {
            let client = self.http.clone();
            let url = download_link.to_string();
            let part_path = part_download_path(destination, index);
            let progress = progress.clone();
            let start = *start;
            let end = *end;

            async move {
                let response = client
                    .get(&url)
                    .header(header::RANGE, format!("bytes={start}-{end}"))
                    .send()
                    .await
                    .with_context(|| format!("failed to request byte range {start}-{end}"))?
                    .error_for_status()
                    .with_context(|| format!("range request failed for bytes {start}-{end}"))?;

                if response.status() != StatusCode::PARTIAL_CONTENT {
                    bail!(
                        "server ignored byte range {start}-{end}; expected HTTP 206 for parallel download"
                    );
                }

                let mut file = fs::File::create(&part_path)
                    .await
                    .with_context(|| format!("failed to create {}", part_path.display()))?;
                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.context("failed to read parallel download body")?;
                    file.write_all(&chunk)
                        .await
                        .with_context(|| format!("failed to write {}", part_path.display()))?;
                    if let Some(progress) = &progress {
                        progress.inc(chunk.len() as u64);
                    }
                }
                file.flush()
                    .await
                    .with_context(|| format!("failed to flush {}", part_path.display()))?;
                Ok::<_, anyhow::Error>(part_path)
            }
        });

        let part_paths = match try_join_all(tasks).await {
            Ok(paths) => paths,
            Err(err) => {
                cleanup_download_parts(destination, ranges.len()).await;
                return Err(err);
            }
        };

        let mut merged = fs::File::create(destination)
            .await
            .with_context(|| format!("failed to create {}", destination.display()))?;
        for part_path in &part_paths {
            let mut part = fs::File::open(part_path)
                .await
                .with_context(|| format!("failed to open {}", part_path.display()))?;
            io::copy(&mut part, &mut merged)
                .await
                .with_context(|| format!("failed to merge {}", part_path.display()))?;
        }
        merged
            .flush()
            .await
            .with_context(|| format!("failed to flush {}", destination.display()))?;

        for part_path in &part_paths {
            fs::remove_file(part_path)
                .await
                .with_context(|| format!("failed to remove {}", part_path.display()))?;
        }

        if let Some(progress) = &progress {
            progress.finish_with_message(format!("download {} complete", destination.display()));
        }

        Ok(total_bytes)
    }

    async fn download_file_sequential(
        &self,
        download_link: &str,
        destination: &Path,
        existing_bytes: u64,
    ) -> Result<u64> {
        let response = self
            .http
            .get(download_link)
            .header(header::RANGE, format!("bytes={existing_bytes}-"))
            .send()
            .await
            .context("failed to download file body")?
            .error_for_status()
            .context("download request failed")?;

        let resumed = existing_bytes > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
        let restart_from_zero = existing_bytes > 0 && !resumed;
        let initial_bytes = if resumed { existing_bytes } else { 0 };

        let total = response.content_length().map(|len| len + initial_bytes);
        let progress = create_progress_bar(total);
        if let Some(progress) = &progress {
            progress.set_position(initial_bytes);
            progress.set_message(format!("download {}", destination.display()));
        }

        let mut stream = response.bytes_stream();
        let mut file = if resumed {
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

        if restart_from_zero {
            file.set_len(0)
                .await
                .with_context(|| format!("failed to reset {}", destination.display()))?;
        }

        let mut written = initial_bytes;
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
            progress.finish_with_message(format!("download {} complete", destination.display()));
        }
        Ok(written)
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

    async fn get_file_updated_at(&self, repo_id: &str, path: &str) -> Result<Option<String>> {
        let response = self
            .http
            .get(format!(
                "{}/api2/repos/{repo_id}/file/detail/",
                self.base_url()
            ))
            .header(header::AUTHORIZATION, self.auth_header_value()?)
            .query(&[("p", path)])
            .send()
            .await
            .with_context(|| format!("failed to request file detail for {path}"))?
            .error_for_status()
            .with_context(|| format!("file detail request failed for {path}"))?;

        let detail = response
            .json::<FileDetail>()
            .await
            .with_context(|| format!("failed to decode file detail for {path}"))?;
        Ok(detail.last_modified)
    }

    async fn get_dir_updated_at(&self, repo_id: &str, path: &str) -> Result<Option<String>> {
        if path == "/" {
            return Ok(None);
        }

        let response = self
            .http
            .get(format!(
                "{}/api/v2.1/repos/{repo_id}/dir/detail/",
                self.base_url()
            ))
            .header(header::AUTHORIZATION, self.auth_header_value()?)
            .query(&[("path", path.trim_start_matches('/'))])
            .send()
            .await
            .with_context(|| format!("failed to request directory detail for {path}"))?
            .error_for_status()
            .with_context(|| format!("directory detail request failed for {path}"))?;

        let detail = response
            .json::<DirectoryDetail>()
            .await
            .with_context(|| format!("failed to decode directory detail for {path}"))?;
        Ok(detail.mtime)
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

    pub fn list_all_share_links(&self) -> Result<Vec<ShareLink>> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            self.http
                .get(format!("{}/api/v2.1/share-links/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .send()
                .await
                .context("failed to list share links")?
                .error_for_status()
                .context("list share links request failed")?
                .json::<Vec<ShareLink>>()
                .await
                .context("failed to parse share link list response")
        })
    }

    pub fn list_share_links(&self, repo_id: &str, path: Option<&str>) -> Result<Vec<ShareLink>> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            let mut query = vec![("repo_id", repo_id.to_string())];
            if let Some(path) = path {
                query.push(("path", path.to_string()));
            }

            self.http
                .get(format!("{}/api/v2.1/share-links/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .query(&query)
                .send()
                .await
                .context("failed to list share links")?
                .error_for_status()
                .context("list share links request failed")?
                .json::<Vec<ShareLink>>()
                .await
                .context("failed to parse share link list response")
        })
    }

    pub fn delete_share_link(&self, token: &str) -> Result<()> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        runtime.block_on(async {
            self.http
                .delete(format!("{}/api/v2.1/share-links/{token}/", self.base_url()))
                .header(header::AUTHORIZATION, self.auth_header_value()?)
                .send()
                .await
                .context("failed to delete share link")?
                .error_for_status()
                .context("delete share link request failed")?;
            Ok(())
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

fn parse_total_bytes_from_content_range(header_value: &str) -> Option<u64> {
    header_value
        .rsplit_once('/')
        .and_then(|(_, total)| total.parse::<u64>().ok())
}

fn parse_filename_from_content_disposition(value: &str) -> Option<String> {
    value
        .split(';')
        .map(str::trim)
        .find_map(|part| {
            part.strip_prefix("filename*=UTF-8''")
                .or_else(|| part.strip_prefix("filename=\""))
                .or_else(|| part.strip_prefix("filename="))
                .map(|name| name.trim_end_matches('"'))
        })
        .map(percent_decode)
        .filter(|name| !name.is_empty())
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        decoded.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn split_download_ranges(total_bytes: u64, parts: usize) -> Vec<(u64, u64)> {
    if total_bytes == 0 {
        return Vec::new();
    }

    let parts = parts.max(1) as u64;
    let chunk_size = total_bytes.div_ceil(parts);
    let mut ranges = Vec::new();
    let mut start = 0u64;

    while start < total_bytes {
        let end = (start + chunk_size).min(total_bytes) - 1;
        ranges.push((start, end));
        start = end + 1;
    }

    ranges
}

fn part_download_path(destination: &Path, index: usize) -> std::path::PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.thufs-part");
    parent.join(format!(".{name}.part-{index}.thufs-part"))
}

async fn cleanup_download_parts(destination: &Path, count: usize) {
    for index in 0..count {
        let path = part_download_path(destination, index);
        let _ = fs::remove_file(path).await;
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
                updated_at: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        Repository, SeafileClient, parse_filename_from_content_disposition,
        parse_total_bytes_from_content_range, split_download_ranges,
    };
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
                    mtime: None,
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
                        mtime: None,
                    },
                    Repository {
                        id: "2".to_string(),
                        name: "dup".to_string(),
                        mtime: None,
                    },
                ],
            )
            .expect_err("should fail");
        assert!(err.to_string().contains("ambiguous"));
    }

    #[test]
    fn parses_total_bytes_from_content_range() {
        assert_eq!(
            parse_total_bytes_from_content_range("bytes 0-0/12345"),
            Some(12345)
        );
        assert_eq!(parse_total_bytes_from_content_range("invalid"), None);
    }

    #[test]
    fn parses_content_disposition_filename() {
        let parsed = parse_filename_from_content_disposition(
            "attachment; filename*=UTF-8''week1%20notes.pdf",
        );
        assert_eq!(parsed.as_deref(), Some("week1 notes.pdf"));
    }

    #[test]
    fn splits_download_ranges_evenly() {
        assert_eq!(
            split_download_ranges(10, 4),
            vec![(0, 2), (3, 5), (6, 8), (9, 9)]
        );
    }
}
