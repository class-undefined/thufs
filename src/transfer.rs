use std::{
    io::{IsTerminal, Write},
    sync::{Arc, Mutex},
};

use anyhow::{Result, bail};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;

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

impl Default for DownloadMode {
    fn default() -> Self {
        Self::Sequential
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    Auto,
    Jsonl,
    None,
}

#[derive(Clone)]
pub struct ProgressReporter {
    inner: Arc<ProgressReporterInner>,
}

enum ProgressReporterInner {
    None,
    Bar(ProgressBar),
    Jsonl(Mutex<JsonlProgressState>),
}

struct JsonlProgressState {
    writer: Box<dyn Write + Send>,
    operation: &'static str,
    path: String,
    total_bytes: Option<u64>,
    last_transferred: u64,
}

#[derive(Debug, Clone, Copy)]
enum ProgressUpdate {
    Keep,
    Set(u64),
    Add(u64),
}

#[derive(Debug, Serialize)]
struct ProgressEvent<'a> {
    event: &'a str,
    operation: &'a str,
    path: &'a str,
    transferred_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<f64>,
    message: Option<&'a str>,
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

impl ProgressMode {
    pub fn parse_keyword(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "jsonl" => Ok(Self::Jsonl),
            "none" => Ok(Self::None),
            _ => bail!("invalid progress mode `{value}`; expected one of: auto, jsonl, none"),
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

pub fn progress_mode_from_matches(matches: &ArgMatches) -> Result<ProgressMode> {
    matches
        .get_one::<String>("progress")
        .map(|value| ProgressMode::parse_keyword(value))
        .transpose()
        .map(|mode| mode.unwrap_or(ProgressMode::Auto))
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

pub fn create_progress_reporter(
    mode: ProgressMode,
    operation: &'static str,
    path: impl Into<String>,
    total: Option<u64>,
) -> Result<ProgressReporter> {
    match mode {
        ProgressMode::None => Ok(ProgressReporter::none()),
        ProgressMode::Auto => Ok(create_progress_bar(total)
            .map(ProgressReporter::bar)
            .unwrap_or_else(ProgressReporter::none)),
        ProgressMode::Jsonl => {
            let reporter = ProgressReporter::jsonl(std::io::stderr(), operation, path, total);
            reporter.started()?;
            Ok(reporter)
        }
    }
}

fn create_progress_bar(total: Option<u64>) -> Option<ProgressBar> {
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

impl ProgressReporter {
    pub fn none() -> Self {
        Self {
            inner: Arc::new(ProgressReporterInner::None),
        }
    }

    pub fn jsonl(
        writer: impl Write + Send + 'static,
        operation: &'static str,
        path: impl Into<String>,
        total_bytes: Option<u64>,
    ) -> Self {
        Self {
            inner: Arc::new(ProgressReporterInner::Jsonl(Mutex::new(
                JsonlProgressState {
                    writer: Box::new(writer),
                    operation,
                    path: path.into(),
                    total_bytes,
                    last_transferred: 0,
                },
            ))),
        }
    }

    fn bar(progress: ProgressBar) -> Self {
        Self {
            inner: Arc::new(ProgressReporterInner::Bar(progress)),
        }
    }

    pub fn set_message(&self, message: impl Into<String>) {
        if let ProgressReporterInner::Bar(progress) = self.inner.as_ref() {
            progress.set_message(message.into());
        }
    }

    pub fn started(&self) -> Result<()> {
        self.emit_jsonl("progress-started", ProgressUpdate::Keep)
    }

    pub fn set_position(&self, transferred_bytes: u64) -> Result<()> {
        match self.inner.as_ref() {
            ProgressReporterInner::None => Ok(()),
            ProgressReporterInner::Bar(progress) => {
                progress.set_position(transferred_bytes);
                Ok(())
            }
            ProgressReporterInner::Jsonl(_) => {
                self.emit_jsonl("progress", ProgressUpdate::Set(transferred_bytes))
            }
        }
    }

    pub fn inc(&self, delta: u64) -> Result<()> {
        match self.inner.as_ref() {
            ProgressReporterInner::None => Ok(()),
            ProgressReporterInner::Bar(progress) => {
                progress.inc(delta);
                Ok(())
            }
            ProgressReporterInner::Jsonl(_) => {
                self.emit_jsonl("progress", ProgressUpdate::Add(delta))
            }
        }
    }

    pub fn finish(&self) -> Result<()> {
        match self.inner.as_ref() {
            ProgressReporterInner::None => Ok(()),
            ProgressReporterInner::Bar(progress) => {
                progress.finish();
                Ok(())
            }
            ProgressReporterInner::Jsonl(_) => {
                self.emit_jsonl("progress-finished", ProgressUpdate::Keep)
            }
        }
    }

    pub fn finish_with_message(&self, message: impl Into<String>) -> Result<()> {
        match self.inner.as_ref() {
            ProgressReporterInner::None => Ok(()),
            ProgressReporterInner::Bar(progress) => {
                progress.finish_with_message(message.into());
                Ok(())
            }
            ProgressReporterInner::Jsonl(_) => self.finish(),
        }
    }

    pub fn warning(&self, message: impl Into<String>) -> Result<()> {
        let message = message.into();
        match self.inner.as_ref() {
            ProgressReporterInner::None => Ok(()),
            ProgressReporterInner::Bar(_) => {
                eprintln!("warning: {message}");
                Ok(())
            }
            ProgressReporterInner::Jsonl(_) => self.emit_jsonl_warning(&message),
        }
    }

    fn emit_jsonl(&self, event: &'static str, update: ProgressUpdate) -> Result<()> {
        let ProgressReporterInner::Jsonl(state) = self.inner.as_ref() else {
            return Ok(());
        };

        let mut state = state.lock().expect("progress reporter mutex poisoned");
        match update {
            ProgressUpdate::Keep => {}
            ProgressUpdate::Set(transferred_bytes) => {
                state.last_transferred = transferred_bytes;
            }
            ProgressUpdate::Add(delta) => {
                state.last_transferred = state.last_transferred.saturating_add(delta);
            }
        }

        let percent = state.total_bytes.and_then(|total| {
            if total == 0 {
                None
            } else {
                Some((state.last_transferred as f64 / total as f64 * 100.0).min(100.0))
            }
        });
        let event = event.to_string();
        let operation = state.operation;
        let path = state.path.clone();
        let transferred_bytes = state.last_transferred;
        let total_bytes = state.total_bytes;
        let event = ProgressEvent {
            event: &event,
            operation,
            path: &path,
            transferred_bytes,
            total_bytes,
            percent,
            message: None,
        };
        serde_json::to_writer(&mut state.writer, &event)?;
        writeln!(&mut state.writer)?;
        state.writer.flush()?;
        Ok(())
    }

    fn emit_jsonl_warning(&self, message: &str) -> Result<()> {
        let ProgressReporterInner::Jsonl(state) = self.inner.as_ref() else {
            return Ok(());
        };

        let mut state = state.lock().expect("progress reporter mutex poisoned");
        let operation = state.operation;
        let path = state.path.clone();
        let transferred_bytes = state.last_transferred;
        let total_bytes = state.total_bytes;
        let percent = state.total_bytes.and_then(|total| {
            if total == 0 {
                None
            } else {
                Some((state.last_transferred as f64 / total as f64 * 100.0).min(100.0))
            }
        });
        let event = ProgressEvent {
            event: "warning",
            operation,
            path: &path,
            transferred_bytes,
            total_bytes,
            percent,
            message: Some(message),
        };
        serde_json::to_writer(&mut state.writer, &event)?;
        writeln!(&mut state.writer)?;
        state.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{ProgressMode, ProgressReporter};

    #[derive(Debug, Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn parses_progress_modes() {
        assert_eq!(
            ProgressMode::parse_keyword("auto").unwrap(),
            ProgressMode::Auto
        );
        assert_eq!(
            ProgressMode::parse_keyword("jsonl").unwrap(),
            ProgressMode::Jsonl
        );
        assert_eq!(
            ProgressMode::parse_keyword("none").unwrap(),
            ProgressMode::None
        );
        assert!(ProgressMode::parse_keyword("xml").is_err());
    }

    #[test]
    fn jsonl_reporter_emits_percent_progress() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let reporter = ProgressReporter::jsonl(
            SharedWriter(output.clone()),
            "download",
            "file.bin",
            Some(20),
        );

        reporter.started().expect("start");
        reporter.set_position(5).expect("position");
        reporter.inc(5).expect("inc");
        reporter.finish().expect("finish");

        let rendered = String::from_utf8(output.lock().expect("lock").clone()).expect("utf8");
        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("\"event\":\"progress-started\""));
        assert!(lines[1].contains("\"transferred_bytes\":5"));
        assert!(lines[1].contains("\"percent\":25.0"));
        assert!(lines[2].contains("\"transferred_bytes\":10"));
        assert!(lines[2].contains("\"percent\":50.0"));
        assert!(lines[3].contains("\"event\":\"progress-finished\""));
    }

    #[test]
    fn jsonl_reporter_emits_warning_events() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let reporter = ProgressReporter::jsonl(
            SharedWriter(output.clone()),
            "download",
            "file.bin",
            Some(20),
        );

        reporter.warning("falling back").expect("warning");

        let rendered = String::from_utf8(output.lock().expect("lock").clone()).expect("utf8");
        assert!(rendered.contains("\"event\":\"warning\""));
        assert!(rendered.contains("\"message\":\"falling back\""));
    }
}
