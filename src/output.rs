use std::io::Write;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Default)]
pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        Self
    }

    pub fn write_line(&self, writer: &mut dyn Write, message: &str) -> Result<()> {
        writeln!(writer, "{message}")?;
        Ok(())
    }

    pub fn write_json<T: Serialize>(&self, writer: &mut dyn Write, value: &T) -> Result<()> {
        writeln!(writer, "{}", serde_json::to_string_pretty(value)?)?;
        Ok(())
    }
}

pub fn redact_token(token: &str) -> String {
    if token.len() <= 4 {
        return "*".repeat(token.len());
    }

    let prefix = &token[..2];
    let suffix = &token[token.len() - 2..];
    format!("{prefix}...{suffix}")
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::{Renderer, redact_token};

    #[derive(Serialize)]
    struct Sample<'a> {
        status: &'a str,
    }

    #[test]
    fn writes_json_output() {
        let renderer = Renderer::new();
        let mut stdout = Vec::new();

        renderer
            .write_json(&mut stdout, &Sample { status: "ok" })
            .expect("json render");

        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("\"status\": \"ok\""));
    }

    #[test]
    fn writes_text_to_selected_stream() {
        let renderer = Renderer::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        renderer
            .write_line(&mut stdout, "normal result")
            .expect("stdout write");
        renderer
            .write_line(&mut stderr, "error result")
            .expect("stderr write");

        assert_eq!(String::from_utf8(stdout).expect("utf8"), "normal result\n");
        assert_eq!(String::from_utf8(stderr).expect("utf8"), "error result\n");
    }

    #[test]
    fn redacts_tokens_for_display() {
        assert_eq!(redact_token("file-token-value"), "fi...ue");
        assert_eq!(redact_token("abcd"), "****");
    }
}
