# thufs

## What This Is

`thufs` is a CLI tool that integrates Tsinghua Cloud Drive into the local terminal for users who prefer scriptable, Unix-style workflows. It focuses on clear, reliable file upload, download, listing, and share-link operations so command-line users can treat THU Cloud Drive as a practical part of their daily automation and backup flow.

The product is intentionally narrow: it is not trying to be a full desktop sync client or a generic Seafile platform. It is a focused terminal interface for THU Cloud Drive users who need predictable commands that work well in shells, scripts, and remote machines.

## Core Value

Terminal users can move files into and out of THU Cloud Drive with simple, reliable commands that are easy to script and hard to misuse.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Provide a THU Cloud Drive specific CLI for upload, download, listing, and share-link workflows.
- [ ] Optimize for scriptable terminal use rather than interactive desktop-style sync.
- [ ] Keep the user model simple in v1: one configured account, one predictable command surface.
- [ ] Build on Seafile-compatible APIs while exposing THU-oriented UX and terminology.

### Out of Scope

- Full bidirectional sync engine — not core to the stated value, and significantly raises complexity.
- Generic multi-instance Seafile support — v1 is intentionally scoped to THU Cloud Drive only.
- Multi-account or profile switching — deferred to keep authentication and config simple in v1.
- Rich GUI or TUI workflow — the target experience is shell-first and script-first.

## Context

The target user is a Tsinghua Cloud Drive terminal user, often operating through scripts on local machines, servers, or clusters. The highest-value workflows identified during initialization are `thufs push`, `thufs pull`, `thufs ls`, and `thufs share`.

The product philosophy is explicitly Unix-oriented: do one thing well, keep commands intuitive, and make shell composition natural. That implies stable exit codes, machine-friendly output modes, transparent path handling, and clear failure messages.

The backend integration is constrained by THU Cloud Drive being based on Seafile. This means the implementation should align with Seafile Web API patterns for authentication, repository and directory traversal, upload/download flows, and share-link creation, while still presenting a THU-first CLI experience.

In v1, sharing should go beyond a bare link and cover common operational controls such as expiration and password protection. Since the main usage is scripted automation, command semantics and output stability matter more than feature breadth.

## Constraints

- **Product Scope**: THU Cloud Drive only in v1 — avoid diluting the CLI with generic multi-instance abstractions too early.
- **API Dependency**: Must work against Seafile-compatible server behavior — core file and sharing operations depend on upstream API capabilities.
- **Interface**: CLI-first and automation-friendly — commands, flags, output, and exit behavior must support shell scripting.
- **Account Model**: Single configured account in v1 — reduces config and support burden while validating the core workflows.
- **Non-goal**: Sync is secondary — upload/download and direct resource operations take precedence over maintaining local/remote parity.
- **Reliability**: File operations must be explicit and predictable — destructive ambiguity in paths, overwrites, or sharing defaults is unacceptable.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Scope v1 to `push`, `pull`, `ls`, and `share` | These are the workflows the user explicitly identified as highest value | — Pending |
| Prioritize upload/download over sync | The user stated sync demand is low compared with direct transfer operations | — Pending |
| Support share password and expiration in v1 | Bare share links are insufficient for real operational use | — Pending |
| Restrict v1 to THU Cloud Drive only | Product positioning is THU-focused rather than generic Seafile | — Pending |
| Use a single-account model in v1 | Keeps onboarding and automation simpler for early versions | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `$gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `$gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-21 after initialization*
