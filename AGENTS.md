<!-- GSD:project-start source:PROJECT.md -->
## Project

`thufs` is a THU Cloud Drive CLI focused on terminal-native file upload, download, listing, and share-link workflows. The current v1 scope is intentionally narrow: THU-only, single-account, script-friendly, and built around `push`, `pull`, `ls`, and `share` instead of sync-heavy behavior.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:STACK.md -->
## Technology Stack

The planned implementation stack is Go with a Seafile-compatible HTTP client layer and a shell-friendly CLI surface. Favor a thin command layer, typed services, stable stdout/stderr behavior, and explicit path semantics over generic abstractions.
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

The intended architecture is a layered CLI: commands parse input, application services enforce product semantics, and a dedicated Seafile client isolates THU/Seafile API behavior. Keep config/auth, transfer, sharing, and output rendering as separate concerns.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `$gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `$gsd-debug` for investigation and bug fixing
- `$gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `$gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` - do not edit manually.
<!-- GSD:profile-end -->
