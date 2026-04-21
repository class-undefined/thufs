use anyhow::Result;
use serde::Serialize;

use crate::{
    config::{ConfigManager, OutputMode},
    output::redact_token,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TokenSetResult {
    pub config_path: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConfigInspection {
    pub config_path: String,
    pub token: String,
    pub default_repo: Option<String>,
    pub output: String,
    pub environment_overrides: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthService {
    config: ConfigManager,
}

impl AuthService {
    pub fn new(config: ConfigManager) -> Self {
        Self { config }
    }

    pub fn set_token(&self, token: &str) -> Result<TokenSetResult> {
        self.config.set_token(token)?;

        Ok(TokenSetResult {
            config_path: self.config.path().display().to_string(),
            token: redact_token(token),
        })
    }

    pub fn inspect(&self) -> Result<ConfigInspection> {
        let (resolved, overrides) = self.config.resolve()?;

        Ok(ConfigInspection {
            config_path: self.config.path().display().to_string(),
            token: resolved
                .token
                .as_deref()
                .map(redact_token)
                .unwrap_or_else(|| "not set".to_string()),
            default_repo: resolved.default_repo,
            output: match resolved.output {
                OutputMode::Human => "human".to_string(),
                OutputMode::Json => "json".to_string(),
            },
            environment_overrides: overrides.active_keys(),
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::AuthService;
    use crate::config::{Config, ConfigManager, EnvironmentOverrides, OutputMode};

    #[test]
    fn inspection_redacts_token_and_reports_overrides() {
        let temp = tempdir().expect("tempdir");
        let manager = ConfigManager::from_path(temp.path().join("config.json"));
        manager
            .write_config(&Config {
                token: Some("file-token-value".to_string()),
                default_repo: Some("repo-a".to_string()),
                output: OutputMode::Human,
            })
            .expect("write config");

        let resolved = manager
            .resolve_with_overrides(EnvironmentOverrides {
                token: Some("env-token-value".to_string()),
                default_repo: None,
                output: Some(OutputMode::Json),
            })
            .expect("resolve with env overrides");

        assert_eq!(resolved.0.output, OutputMode::Json);
        assert_eq!(
            resolved.1.active_keys(),
            vec!["THUFS_TOKEN", "THUFS_OUTPUT"]
        );

        manager
            .set_token("file-token-value")
            .expect("set file token back");

        let service = AuthService::new(manager.clone());
        let inspection = service.inspect().expect("inspect");

        assert_eq!(inspection.token, "fi...ue");
        assert_eq!(inspection.default_repo, Some("repo-a".to_string()));
        assert_eq!(inspection.output, "human");
    }
}
