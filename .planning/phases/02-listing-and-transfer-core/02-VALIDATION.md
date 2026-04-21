---
phase: 2
slug: listing-and-transfer-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
---

# Phase 2 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` |
| **Config file** | `Cargo.toml` / `Cargo.lock` |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~20 seconds |

## Sampling Rate

- After every task commit: run `cargo test`
- After every wave: run `cargo test`
- Before phase close-out: all CLI and unit tests green

## Per-Plan Verification Map

| Plan | Focus | Verification |
|------|-------|-------------|
| 02-01 | repo resolution + Seafile client core | client and path-resolution unit tests |
| 02-02 | `ls` | listing unit tests + CLI integration tests |
| 02-03 | `push` | upload service tests + CLI overwrite behavior tests |
| 02-04 | `pull` | download safety tests + CLI path/error tests |

## Manual Checks

- `cargo run -- ls --help`
- `cargo run -- push --help`
- `cargo run -- pull --help`
- Spot-check human-readable and `--json` output modes

## Validation Sign-Off

- [ ] Every new command has automated coverage
- [ ] Overwrite and invalid-path flows are covered
- [ ] Output behavior is stable for stdout/stderr and `--json`
- [ ] `nyquist_compliant: true` set before phase completion

