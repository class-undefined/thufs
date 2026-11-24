use std::fmt;

use anyhow::{Result, anyhow, bail};
use reqwest::{Client, ClientBuilder, NoProxy, Proxy};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::Config;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ProxyUrl(String);

impl ProxyUrl {
    pub fn parse(input: &str) -> Result<Self> {
        if input.chars().any(char::is_control) {
            bail!("Enter a valid proxy URL, such as http://127.0.0.1:7890.");
        }
        let input = input.trim();
        if input.is_empty()
            || input.chars().any(char::is_whitespace)
            || input.chars().any(char::is_control)
        {
            bail!("Enter a valid proxy URL, such as http://127.0.0.1:7890.");
        }
        let parsed = Url::parse(input)
            .map_err(|_| anyhow!("Enter a valid proxy URL, such as http://127.0.0.1:7890."))?;
        if !matches!(parsed.scheme(), "http" | "https" | "socks5" | "socks5h") {
            bail!("Proxy scheme must be http, https, socks5, or socks5h.");
        }
        if parsed.host_str().is_none()
            || parsed.port() == Some(0)
            || !matches!(parsed.path(), "" | "/")
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            bail!("Enter a proxy host and port without a path, query, or fragment.");
        }
        Ok(Self(parsed.to_string()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn redacted(&self) -> String {
        let Ok(mut url) = Url::parse(&self.0) else {
            return "[invalid proxy]".into();
        };
        if !url.username().is_empty() || url.password().is_some() {
            let _ = url.set_username("redacted");
            let _ = url.set_password(None);
        }
        url.to_string()
    }
}

impl fmt::Debug for ProxyUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.redacted())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ProxyConfig {
    #[default]
    Auto,
    Direct,
    Manual {
        url: ProxyUrl,
    },
}

impl ProxyConfig {
    pub fn mode(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Direct => "Direct",
            Self::Manual { .. } => "Manual",
        }
    }

    pub fn from_input(mode: &str, address: &str) -> Result<Self> {
        match mode.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "direct" => Ok(Self::Direct),
            "manual" => Ok(Self::Manual {
                url: ProxyUrl::parse(address)?,
            }),
            _ => bail!("Proxy mode must be auto, manual, or direct."),
        }
    }

    pub fn normalized(&self) -> Result<Self> {
        match self {
            Self::Manual { url } => Self::from_input("manual", url.expose()),
            _ => Ok(self.clone()),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Auto => "Auto (environment / system)".into(),
            Self::Direct => "Direct (no proxy)".into(),
            Self::Manual { url } => url.redacted(),
        }
    }
}

pub fn client_builder(config: &Config) -> Result<ClientBuilder> {
    let builder = Client::builder()
        .user_agent(config.effective_user_agent())
        .connect_timeout(std::time::Duration::from_secs(10));
    match &config.proxy {
        // reqwest reads HTTP(S)_PROXY/ALL_PROXY and NO_PROXY, then static system
        // settings on macOS and Windows. Keep its built-in precedence rules.
        ProxyConfig::Auto => Ok(builder),
        ProxyConfig::Direct => Ok(builder.no_proxy()),
        ProxyConfig::Manual { url } => {
            let proxy = Proxy::all(url.expose())
                .map_err(|_| {
                    anyhow!("Cannot configure the proxy. Check its address and hostname.")
                })?
                .no_proxy(NoProxy::from_env());
            Ok(builder.no_proxy().proxy(proxy))
        }
    }
}

#[cfg(test)]
mod tests;
