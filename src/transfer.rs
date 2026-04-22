use std::io::IsTerminal;

use anyhow::{Result, bail};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    Prompt,
    Overwrite,
    Uniquify,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadMode {
    Auto,
    Parallel,
    Sequential,
}

impl DownloadMode {
    pub fn parse_keyword(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "parallel" => Ok(Self::Parallel),
            "sequential" => Ok(Self::Sequential),
            _ => bail!(
                "invalid download mode `{value}`; expected one of: auto, parallel, sequential"
            ),
        }
    }
}

impl ConflictPolicy {
    pub fn parse_keyword(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "prompt" => Ok(Self::Prompt),
            "overwrite" => Ok(Self::Overwrite),
            "uniquify" => Ok(Self::Uniquify),
            "fail" => Ok(Self::Fail),
            _ => bail!(
                "invalid conflict policy `{value}`; expected one of: prompt, overwrite, uniquify, fail"
            ),
        }
    }
}

pub fn conflict_policy_from_matches(matches: &ArgMatches) -> Result<ConflictPolicy> {
    if let Some(value) = matches.get_one::<String>("conflict") {
        return ConflictPolicy::parse_keyword(value);
    }

    if matches.get_flag("overwrite") {
        Ok(ConflictPolicy::Overwrite)
    } else if matches.get_flag("rename") {
        Ok(ConflictPolicy::Uniquify)
    } else if matches.get_flag("fail") {
        Ok(ConflictPolicy::Fail)
    } else {
        Ok(ConflictPolicy::Uniquify)
    }
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

    let mut value = bytes as f64;
    let mut unit_index = 0usize;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} {}", UNITS[unit_index])
    } else if value >= 10.0 || (value.fract() - 0.0).abs() < f64::EPSILON {
        format!("{value:.0} {}", UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

pub fn create_progress_bar(total: Option<u64>) -> Option<ProgressBar> {
    let stderr = std::io::stderr();
    if !stderr.is_terminal() {
        return None;
    }

    let progress = match total {
        Some(total) => ProgressBar::new(total),
        None => ProgressBar::new_spinner(),
    };

    let style = if total.is_some() {
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
    } else {
        ProgressStyle::with_template("{spinner:.green} {msg} {bytes}")
    }
    .expect("valid progress template");

    progress.set_style(style);
    Some(progress)
}
