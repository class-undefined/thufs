use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const CONFIG_DIR_ENV: &str = "THUFS_CONFIG_DIR";
const TOKEN_ENV: &str = "THUFS_TOKEN";
const DEFAULT_REPO_ENV: &str = "THUFS_DEFAULT_REPO";
const OUTPUT_ENV: &str = "THUFS_OUTPUT";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    #[default]
    Human,
    Json,
}

#[allow(dead_code)]
impl OutputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
        }
    }
}

impl FromStr for OutputMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            other => bail!("unsupported output mode: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_repo: Option<String>,
    #[serde(default)]
    pub output: OutputMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: None,
            default_repo: None,
            output: OutputMode::Human,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentOverrides {
    pub token: Option<String>,
    pub default_repo: Option<String>,
    pub output: Option<OutputMode>,
}

impl EnvironmentOverrides {
    pub fn from_process() -> Result<Self> {
        Ok(Self {
            token: env::var(TOKEN_ENV).ok(),
            default_repo: env::var(DEFAULT_REPO_ENV).ok(),
            output: env::var(OUTPUT_ENV)
                .ok()
                .map(|value| OutputMode::from_str(value.trim()))
                .transpose()?,
        })
    }

    pub fn active_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();

        if self.token.is_some() {
            keys.push(TOKEN_ENV.to_string());
        }
        if self.default_repo.is_some() {
            keys.push(DEFAULT_REPO_ENV.to_string());
        }
        if self.output.is_some() {
            keys.push(OUTPUT_ENV.to_string());
        }

        keys
    }
}

#[derive(Debug, Clone)]
pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            path: default_config_path(),
        }
    }

    #[cfg(test)]
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[allow(dead_code)]
    pub fn load_resolved(&self) -> Result<Config> {
        self.resolve().map(|(config, _)| config)
    }

    pub fn resolve(&self) -> Result<(Config, EnvironmentOverrides)> {
        self.resolve_with_overrides(EnvironmentOverrides::from_process()?)
    }

    pub fn resolve_with_overrides(
        &self,
        overrides: EnvironmentOverrides,
    ) -> Result<(Config, EnvironmentOverrides)> {
        let mut config = self.load_file()?;

        if let Some(token) = overrides.token.clone() {
            config.token = Some(token);
        }
        if let Some(default_repo) = overrides.default_repo.clone() {
            config.default_repo = Some(default_repo);
        }
        if let Some(output) = overrides.output {
            config.output = output;
        }

        Ok((config, overrides))
    }

    pub fn load_file(&self) -> Result<Config> {
        if !self.path.exists() {
            return Ok(Config::default());
        }

        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read config file {}", self.path.display()))?;

        let config = serde_json::from_str::<Config>(&raw)
            .with_context(|| format!("failed to parse config file {}", self.path.display()))?;

        Ok(config)
    }

    pub fn set_token(&self, token: &str) -> Result<()> {
        let mut config = self.load_file()?;
        config.token = Some(token.to_string());
        self.write_config(&config)
    }

    pub fn write_config(&self, config: &Config) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }

        let raw = serde_json::to_string_pretty(config)?;

        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);

        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(&self.path)
            .with_context(|| format!("failed to open config file {}", self.path.display()))?;
        file.write_all(raw.as_bytes())
            .with_context(|| format!("failed to write config file {}", self.path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to finalize config file {}", self.path.display()))?;

        #[cfg(unix)]
        {
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("failed to secure config file {}", self.path.display()))?;
        }

        Ok(())
    }
}

fn default_config_path() -> PathBuf {
    if let Some(path) = env::var_os(CONFIG_DIR_ENV) {
        return PathBuf::from(path).join("config.json");
    }

    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("thufs").join("config.json");
    }

    if let Some(path) = env::var_os("HOME") {
        return PathBuf::from(path)
            .join(".config")
            .join("thufs")
            .join("config.json");
    }

    if let Some(path) = env::var_os("APPDATA") {
        return PathBuf::from(path).join("thufs").join("config.json");
    }

    PathBuf::from(".thufs-config.json")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{Config, ConfigManager, EnvironmentOverrides, OutputMode};

    #[test]
    fn resolve_prefers_environment_over_file_values() {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("file-token".to_string()),
                default_repo: Some("file-repo".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");

        let (resolved, overrides) = manager
            .resolve_with_overrides(EnvironmentOverrides {
                token: Some("env-token".to_string()),
                default_repo: Some("env-repo".to_string()),
                output: Some(OutputMode::Json),
            })
            .expect("resolve");

        assert_eq!(resolved.token.as_deref(), Some("env-token"));
        assert_eq!(resolved.default_repo.as_deref(), Some("env-repo"));
        assert_eq!(resolved.output, OutputMode::Json);
        assert_eq!(
            overrides.active_keys(),
            vec!["THUFS_TOKEN", "THUFS_DEFAULT_REPO", "THUFS_OUTPUT"]
        );
    }

    #[test]
    fn resolve_falls_back_to_file_values_when_env_is_missing() {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("file-token".to_string()),
                default_repo: Some("file-repo".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");

        let (resolved, overrides) = manager
            .resolve_with_overrides(EnvironmentOverrides::default())
            .expect("resolve");

        assert_eq!(resolved.token.as_deref(), Some("file-token"));
        assert_eq!(resolved.default_repo.as_deref(), Some("file-repo"));
        assert_eq!(resolved.output, OutputMode::Human);
        assert!(overrides.active_keys().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn writes_config_file_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("config.json");
        let manager = ConfigManager::from_path(path.clone());
        manager
            .write_config(&Config::default())
            .expect("write config");

        let metadata = fs::metadata(path).expect("metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn rejects_unknown_output_modes() {
        let err = "yaml".parse::<OutputMode>().expect_err("should fail");
        assert!(
            err.to_string().contains("unsupported output mode"),
            "unexpected error: {err}"
        );
    }
}
