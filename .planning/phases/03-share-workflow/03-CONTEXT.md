---
phase: 03-share-workflow
status: discussed
created: 2026-04-21
---

# Phase 3 Context: Share Workflow

## Goal

Deliver `thufs share` as a flat business command for creating THU Cloud Drive share links from the terminal.

## Decisions

- `thufs share <remote>` creates a share link for one remote file or directory path.
- `--password <value>` and `--expire-days <days>` are explicit optional controls.
- Human output should print the link alone so it is easy to pipe or copy.
- `--json` should include the link plus source metadata for script use.
- Remote path semantics reuse the canonical `repo:<library>/<path>` or default-repo shorthand contract.

## API Assumption

Seafile API v2.1 exposes share-link creation at `POST /api/v2.1/share-links/` with `repo_id`, `path`, optional `password`, and optional `expire_days`. The implementation isolates this assumption in `src/seafile.rs` for later live validation against THU Cloud Drive.

## Risks

- THU Cloud Drive may differ slightly from upstream Seafile share-link response fields.
- Existing Phase 2 `ls` CLI still needs hardening to use real client listing instead of deterministic placeholder entries.

