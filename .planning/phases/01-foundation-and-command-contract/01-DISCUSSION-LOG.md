# Phase 1: Foundation and Command Contract - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 1-Foundation and Command Contract
**Areas discussed:** Authentication entrypoint, Configuration and credential sourcing, Remote path contract, Output and failure behavior, Command surface style

---

## Authentication entrypoint

| Option | Description | Selected |
|--------|-------------|----------|
| `token` driven | User obtains token elsewhere, then sets it in `thufs` | ✓ |
| CLI username/password login | CLI exchanges credentials for later use | |
| Browser-assisted login | Browser completes login, CLI stores resulting credentials | |
| Mixed | Support both login and direct token setup | |

**User's choice:** `token` driven
**Notes:** v1 should not depend on username/password login as the primary onboarding path.

---

## Configuration and credential sourcing

| Option | Description | Selected |
|--------|-------------|----------|
| File-first, env override | Config file is normal path; environment variables override | ✓ |
| Env-first, file supplement | Environment variables are the primary path | |
| File-only | No env override layer | |
| Custom | User-defined approach | |

**User's choice:** File-first, env override
**Notes:** This balances daily terminal use with scripting needs.

---

## Remote path contract

| Option | Description | Selected |
|--------|-------------|----------|
| Fully-qualified remote path only | Every command requires an explicit full remote path | |
| Repo and path split | Repo passed separately from remote path | |
| Default repo plus relative paths | Remote repo omitted once configured | |
| Mixed | Explicit full path always works; default repo is a convenience layer | ✓ |

**User's choice:** Mixed
**Notes:** Explicit form is the safe baseline; default repo is shorthand.

---

## Output and failure behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Human-readable by default, `--json` available | Terminal-first default with scriptable structured output | ✓ |
| Minimal plain text by default | Lean text optimized for parsing | |
| JSON by default | Structured output is the primary UX | |
| Custom | User-defined approach | |

**User's choice:** Human-readable by default, `--json` available
**Notes:** Also paired with stdout for normal output, stderr for errors, and stable exit codes.

---

## Command surface style

| Option | Description | Selected |
|--------|-------------|----------|
| Git style | Hierarchical command groups everywhere | |
| Flat style | Mostly top-level commands, minimal grouping | |
| Hybrid | Business commands flat; management commands grouped | ✓ |

**User's choice:** Hybrid
**Notes:** `push`/`pull`/`ls`/`share` stay flat, while `auth ...` and `config ...` stay grouped.

## the agent's Discretion

- Exact config filename and serialization format
- Exact token subcommand spelling
- Exact verbosity flag surface
- Exact canonical remote-path syntax

## Deferred Ideas

None.
