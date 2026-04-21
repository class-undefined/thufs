# Phase 2: Listing and Transfer Core - Discussion Log

> **Audit trail only.** Decisions are captured in `02-CONTEXT.md`.

**Date:** 2026-04-22
**Phase:** 2-Listing and Transfer Core
**Mode:** auto-derived from existing product decisions and codebase state

---

## Areas resolved from prior decisions

- **Command shape:** `ls`, `push`, and `pull` remain flat top-level business verbs.
- **Auth model:** token-driven configuration from Phase 1 is reused; no login flow added.
- **Remote path model:** explicit `repo:<library>/<path>` stays canonical, default repo remains shorthand only.
- **Output contract:** stdout/stderr split and `--json` remain mandatory.

## Phase 2-specific decisions

- `ls` should list remote directories with deterministic, script-friendly output.
- Phase 2 only covers single-file upload/download, not recursive directory transfer.
- Overwrite and invalid-path behavior should be explicit and fail safely.
- Repo lookup and remote path resolution should be shared infrastructure, not duplicated per command.

## Deferred Ideas

- Recursive directory transfer
- Resume support
- Batch multi-file operations

