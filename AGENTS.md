## Project

`thufs` is a THU Cloud Drive CLI with an interactive terminal UI and script-friendly file upload, download, listing, and share-link workflows.

## Technology Stack

The implementation uses Rust, clap for CLI parsing, ratatui/crossterm for the TUI, and reqwest for Seafile-compatible HTTP APIs. Favor a thin command layer, typed services, stable stdout/stderr behavior, and explicit path semantics over generic abstractions.

## Architecture

Commands parse input, application services enforce product semantics, and the Seafile client isolates THU/Seafile API behavior. Keep config/auth, transfer, sharing, and output rendering as separate concerns.
