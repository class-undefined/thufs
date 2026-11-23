use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::i18n::Language;
use crate::{app::account_service::AccountInfoResult, config::ConfigManager};

const ENVIRONMENT: &str = "@environment";
const CLI_CONFIG: &str = "@config";
const SIGNED_OUT: &str = "@signed-out";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SecretToken(String);

impl SecretToken {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedAccount {
    pub key: String,
    pub label: String,
    pub email: String,
    token: SecretToken,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: Language,
    pub active_account: Option<String>,
    pub accounts: Vec<SavedAccount>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Language::detect(),
            active_account: None,
            accounts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccountChoice {
    pub key: String,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct SettingsView {
    pub language: Language,
    pub active_account: Option<String>,
    pub accounts: Vec<AccountChoice>,
}

pub enum Credential {
    Saved(String),
    Token { token: SecretToken, label: String },
}

pub struct AuthorizedAccount {
    pub token: SecretToken,
    pub account: AccountInfoResult,
    pub settings: SettingsView,
}

pub struct SettingsStore {
    path: Option<PathBuf>,
    preferences: Preferences,
    environment: Option<SecretToken>,
    cli_token: Option<SecretToken>,
}

impl SettingsStore {
    pub fn open(config: &ConfigManager) -> Result<Self> {
        let file = config.load_file()?;
        let (_, overrides) = config.resolve()?;
        let path = config.path().with_file_name("tui.json");
        let preferences = if path.exists() {
            serde_json::from_slice(&fs::read(&path).context("Cannot read TUI settings")?)
                .context("Cannot parse TUI settings")?
        } else {
            Preferences::default()
        };
        Ok(Self {
            path: Some(path),
            preferences,
            environment: nonempty(overrides.token),
            cli_token: nonempty(file.token),
        })
    }

    pub fn ephemeral() -> Self {
        Self {
            path: None,
            preferences: Preferences::default(),
            environment: None,
            cli_token: None,
        }
    }

    pub fn view(&self) -> SettingsView {
        let mut accounts: Vec<_> = self
            .preferences
            .accounts
            .iter()
            .map(|a| AccountChoice {
                key: a.key.clone(),
                label: a.label.clone(),
                detail: a.email.clone(),
            })
            .collect();
        if self.environment.is_some() {
            accounts.push(AccountChoice {
                key: ENVIRONMENT.into(),
                label: "Environment account".into(),
                detail: "THUFS_TOKEN".into(),
            });
        }
        if self.cli_token.is_some() && self.cli_token != self.environment {
            accounts.push(AccountChoice {
                key: CLI_CONFIG.into(),
                label: "CLI account".into(),
                detail: "config.json".into(),
            });
        }
        SettingsView {
            language: self.preferences.language,
            active_account: self.active_key(),
            accounts,
        }
    }

    fn active_key(&self) -> Option<String> {
        match self.preferences.active_account.as_deref() {
            Some(SIGNED_OUT) => None,
            Some(key) if self.token_for(key).is_ok() => Some(key.into()),
            _ if self.environment.is_some() => Some(ENVIRONMENT.into()),
            _ if self.cli_token.is_some() => Some(CLI_CONFIG.into()),
            _ => None,
        }
    }

    pub fn initial_token(&self) -> Option<SecretToken> {
        self.active_key().and_then(|key| self.token_for(&key).ok())
    }

    pub fn token_for(&self, key: &str) -> Result<SecretToken> {
        let token = match key {
            ENVIRONMENT => self.environment.as_ref(),
            CLI_CONFIG => self.cli_token.as_ref(),
            key => self
                .preferences
                .accounts
                .iter()
                .find(|a| a.key == key)
                .map(|a| &a.token),
        };
        token
            .cloned()
            .context("The selected account is no longer available. Sign in again.")
    }

    pub fn activate_verified(
        &mut self,
        credential: &Credential,
        token: SecretToken,
        account: &AccountInfoResult,
    ) -> Result<SettingsView> {
        let mut next = self.preferences.clone();
        let key = match credential {
            Credential::Saved(key) => key.clone(),
            Credential::Token { label, .. } => {
                let key = format!("account:{}", account.email);
                let label = if label.trim().is_empty() {
                    account
                        .name
                        .as_deref()
                        .filter(|n| !n.is_empty())
                        .unwrap_or(&account.email)
                        .to_string()
                } else {
                    label.trim().to_string()
                };
                let saved = SavedAccount {
                    key: key.clone(),
                    label,
                    email: account.email.clone(),
                    token,
                };
                if let Some(existing) = next.accounts.iter_mut().find(|a| a.key == key) {
                    *existing = saved;
                } else {
                    next.accounts.push(saved);
                }
                key
            }
        };
        next.active_account = Some(key);
        self.persist(&next)?;
        self.preferences = next;
        Ok(self.view())
    }

    pub fn set_language(&mut self, language: Language) -> Result<()> {
        let mut next = self.preferences.clone();
        next.language = language;
        self.persist(&next)?;
        self.preferences = next;
        Ok(())
    }

    pub fn sign_out(&mut self) -> Result<SettingsView> {
        let mut next = self.preferences.clone();
        next.active_account = Some(SIGNED_OUT.into());
        self.persist(&next)?;
        self.preferences = next;
        Ok(self.view())
    }

    fn persist(&self, next: &Preferences) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path.parent().context("Settings directory is missing")?;
        fs::create_dir_all(parent).context("Cannot create settings directory")?;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temp = parent.join(format!(
            ".tui-{}-{}.tmp",
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
            let mut file = options
                .open(&temp)
                .context("Cannot create private settings file")?;
            serde_json::to_writer_pretty(&mut file, next).context("Cannot encode settings")?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, path).context("Cannot save settings")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

pub fn validate_token(token: &str) -> Result<()> {
    if token.is_empty() {
        bail!("Token cannot be empty.");
    }
    if token.chars().any(char::is_whitespace) || token.chars().any(char::is_control) {
        bail!("Paste the token only, without spaces or line breaks.");
    }
    Ok(())
}

fn nonempty(value: Option<String>) -> Option<SecretToken> {
    value.filter(|s| !s.trim().is_empty()).map(SecretToken::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, SettingsStore) {
        let temp = tempfile::tempdir().unwrap();
        let store = SettingsStore {
            path: Some(temp.path().join("tui.json")),
            preferences: Preferences {
                language: Language::English,
                ..Preferences::default()
            },
            environment: Some(SecretToken::new("environment-secret".into())),
            cli_token: Some(SecretToken::new("cli-secret".into())),
        };
        (temp, store)
    }

    fn account(email: &str) -> AccountInfoResult {
        let super::super::backend::Message::Account(Ok(mut account)) =
            super::super::demo::respond(super::super::backend::Request::Account)
        else {
            unreachable!()
        };
        account.email = email.into();
        account.name = Some("Test user".into());
        account
    }

    fn save(store: &mut SettingsStore, email: &str, token: &str) -> SettingsView {
        let secret = SecretToken::new(token.into());
        store
            .activate_verified(
                &Credential::Token {
                    token: secret.clone(),
                    label: "Saved account".into(),
                },
                secret,
                &account(email),
            )
            .unwrap()
    }

    #[test]
    fn saved_accounts_override_environment_without_modifying_cli_config() {
        let (temp, mut store) = fixture();
        let config = ConfigManager::from_path(temp.path().join("config.json"));
        config.set_token("original-cli-token").unwrap();
        save(&mut store, "one@example.com", "first-account-secret");
        assert_eq!(
            store.initial_token().unwrap().expose(),
            "first-account-secret"
        );
        assert_eq!(
            config.load_file().unwrap().token.as_deref(),
            Some("original-cli-token")
        );
        let restored = SettingsStore::open(&config).unwrap();
        assert_eq!(
            restored.initial_token().unwrap().expose(),
            "first-account-secret"
        );
        assert_eq!(
            restored.view().active_account.as_deref(),
            Some("account:one@example.com")
        );
    }

    #[test]
    fn accounts_can_be_selected_and_a_rotated_token_updates_the_same_identity() {
        let (_temp, mut store) = fixture();
        save(&mut store, "one@example.com", "first-account-secret");
        save(&mut store, "two@example.com", "second-account-secret");
        save(&mut store, "one@example.com", "rotated-account-secret");
        assert_eq!(store.preferences.accounts.len(), 2);
        let token = store.token_for("account:two@example.com").unwrap();
        store
            .activate_verified(
                &Credential::Saved("account:two@example.com".into()),
                token,
                &account("two@example.com"),
            )
            .unwrap();
        assert_eq!(
            store.initial_token().unwrap().expose(),
            "second-account-secret"
        );
        assert_eq!(
            store.token_for("account:one@example.com").unwrap().expose(),
            "rotated-account-secret"
        );
    }

    #[test]
    fn language_and_logout_persist_and_logout_overrides_available_environment_credentials() {
        let (temp, mut store) = fixture();
        save(&mut store, "one@example.com", "first-account-secret");
        store.set_language(Language::Chinese).unwrap();
        store.sign_out().unwrap();
        assert!(store.initial_token().is_none());
        assert!(store.view().active_account.is_none());
        assert!(!store.view().accounts.is_empty());
        let config = ConfigManager::from_path(temp.path().join("config.json"));
        let mut restored = SettingsStore::open(&config).unwrap();
        restored.environment = Some(SecretToken::new("another-environment-secret".into()));
        assert!(restored.initial_token().is_none());
        assert_eq!(restored.view().language, Language::Chinese);
    }

    #[test]
    fn account_metadata_and_debug_output_never_include_credentials() {
        let (_temp, mut store) = fixture();
        let view = save(&mut store, "one@example.com", "first-account-secret");
        let debug = format!("{:?} {:?}", store.preferences, view);
        for token in ["first-account-secret", "environment-secret", "cli-secret"] {
            assert!(!debug.contains(token));
        }
    }

    #[test]
    fn failed_settings_write_does_not_change_the_active_account_or_language() {
        let (temp, mut store) = fixture();
        let before = store.view();
        let not_directory = temp.path().join("a-file");
        fs::write(&not_directory, "test").unwrap();
        store.path = Some(not_directory.join("tui.json"));
        assert!(store.set_language(Language::Chinese).is_err());
        assert_eq!(store.view().language, before.language);
        assert_eq!(store.view().active_account, before.active_account);
        let secret = SecretToken::new("new-secret".into());
        assert!(
            store
                .activate_verified(
                    &Credential::Token {
                        token: secret.clone(),
                        label: String::new()
                    },
                    secret,
                    &account("new@example.com")
                )
                .is_err()
        );
        assert!(store.preferences.accounts.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn persisted_tokens_have_private_permissions_and_no_temporary_file_is_left() {
        use std::os::unix::fs::PermissionsExt;
        let (temp, mut store) = fixture();
        save(&mut store, "one@example.com", "first-account-secret");
        assert_eq!(
            fs::metadata(temp.path().join("tui.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        store.set_language(Language::Chinese).unwrap();
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn token_validation_rejects_empty_tokens_and_embedded_whitespace() {
        for token in ["", "Token value", "abc\ndef", "abc\tdef", "abc\0def"] {
            assert!(validate_token(token).is_err());
        }
        assert!(validate_token("0123456789abcdef").is_ok());
    }
}
