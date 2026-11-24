use std::{
    fs,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use reqwest::{StatusCode, header};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::config::ConfigManager;

mod source;
pub use source::UpdateSource;

pub const RELEASES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");
const CRATE_URL: &str = "https://crates.io/crates/thufs";
const CACHE_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    Cached,
    Refresh,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub source: UpdateSource,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: String,
    pub checked_at: u64,
    pub cached: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedRelease {
    #[serde(default)]
    source: UpdateSource,
    checked_at: u64,
    tag: Option<String>,
}

#[derive(Deserialize)]
struct LatestRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

#[derive(Deserialize)]
struct PublishedCrate {
    versions: Vec<PublishedVersion>,
}

#[derive(Deserialize)]
struct PublishedVersion {
    num: String,
    yanked: bool,
}

pub struct UpdateService {
    config: ConfigManager,
    source: UpdateSource,
    endpoint: String,
    current_version: String,
}

impl UpdateService {
    pub fn new(config: ConfigManager) -> Self {
        let source = UpdateSource::detect();
        Self {
            config,
            source,
            endpoint: source.endpoint().into(),
            current_version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn check(&self, mode: CheckMode) -> Result<UpdateCheck> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Cannot read the clock for update checks")?
            .as_secs();
        self.check_at(mode, now)
    }

    fn check_at(&self, mode: CheckMode, now: u64) -> Result<UpdateCheck> {
        if mode == CheckMode::Cached {
            if let Some(result) = self.read_cache(now) {
                return Ok(result);
            }
        }
        let config = self.config.load_file()?;
        // A separate client deliberately carries no cloud-drive authorization headers.
        let client = crate::network::client_builder(&config)?
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(6))
            .build()
            .context("Cannot create update-check client")?;
        let runtime = tokio::runtime::Runtime::new().context("Cannot start update check")?;
        let tag = runtime.block_on(async {
            let response = client
                .get(&self.endpoint)
                .header(
                    header::ACCEPT,
                    match self.source {
                        UpdateSource::GitHub => "application/vnd.github+json",
                        UpdateSource::CratesIo => "application/json",
                    },
                )
                .send()
                .await
                .with_context(|| {
                    format!(
                        "Cannot contact {} to check for updates",
                        self.source.label()
                    )
                })?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if matches!(
                response.status(),
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            ) {
                bail!("Update check is rate limited. Try again later.");
            }
            let response = response.error_for_status().context("Update check failed")?;
            match self.source {
                UpdateSource::GitHub => {
                    let release: LatestRelease = response
                        .json()
                        .await
                        .context("Cannot read the latest release")?;
                    let version = release_version(&release.tag_name)?;
                    if release.draft || release.prerelease || !version.pre.is_empty() {
                        bail!("The latest release is not a stable version.");
                    }
                    Ok(Some(release.tag_name))
                }
                UpdateSource::CratesIo => {
                    let published: PublishedCrate = response
                        .json()
                        .await
                        .context("Cannot read the published Cargo versions")?;
                    let mut latest: Option<Version> = None;
                    for release in published
                        .versions
                        .into_iter()
                        .filter(|version| !version.yanked)
                    {
                        let version = Version::parse(&release.num)
                            .context("The published Cargo version is not a valid version")?;
                        if version.pre.is_empty()
                            && latest
                                .as_ref()
                                .is_none_or(|latest| version.cmp_precedence(latest).is_gt())
                        {
                            latest = Some(version);
                        }
                    }
                    Ok(latest.map(|version| version.to_string()))
                }
            }
        })?;
        let release = CachedRelease {
            source: self.source,
            checked_at: now,
            tag,
        };
        let result = self.result(&release, false)?;
        // Failure to write an optional cache must not turn a successful check into an error.
        let _ = self.write_cache(&release);
        Ok(result)
    }

    fn read_cache(&self, now: u64) -> Option<UpdateCheck> {
        let bytes = fs::read(self.config.path().with_file_name("update-check.json")).ok()?;
        let release: CachedRelease = serde_json::from_slice(&bytes).ok()?;
        if release.source != self.source
            || now.checked_sub(release.checked_at)? >= CACHE_LIFETIME.as_secs()
        {
            return None;
        }
        self.result(&release, true).ok()
    }

    fn result(&self, release: &CachedRelease, cached: bool) -> Result<UpdateCheck> {
        let current = Version::parse(&self.current_version).context("Invalid installed version")?;
        let latest = release.tag.as_deref().map(release_version).transpose()?;
        if latest
            .as_ref()
            .is_some_and(|version| !version.pre.is_empty())
        {
            bail!("The cached release is not a stable version.");
        }
        Ok(UpdateCheck {
            source: self.source,
            current_version: self.current_version.clone(),
            latest_version: latest.as_ref().map(ToString::to_string),
            update_available: latest
                .as_ref()
                .is_some_and(|version| version.cmp_precedence(&current).is_gt()),
            release_url: match self.source {
                UpdateSource::GitHub => release.tag.as_ref().map_or_else(
                    || RELEASES_URL.to_string(),
                    |tag| format!("{RELEASES_URL}/tag/{tag}"),
                ),
                UpdateSource::CratesIo => latest.as_ref().map_or_else(
                    || CRATE_URL.to_string(),
                    |version| format!("{CRATE_URL}/{version}"),
                ),
            },
            checked_at: release.checked_at,
            cached,
        })
    }

    fn write_cache(&self, release: &CachedRelease) -> Result<()> {
        let path = self.config.path().with_file_name("update-check.json");
        let parent = path.parent().context("Update cache directory is missing")?;
        fs::create_dir_all(parent)?;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temporary = parent.join(format!(
            ".update-check-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            serde_json::to_writer(&mut file, release)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn release_version(tag: &str) -> Result<Version> {
    Version::parse(
        tag.strip_prefix('v')
            .or_else(|| tag.strip_prefix('V'))
            .unwrap_or(tag),
    )
    .context("The release tag is not a valid version")
}

#[cfg(test)]
mod tests;
