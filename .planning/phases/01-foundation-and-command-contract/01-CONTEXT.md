# Phase 1: Foundation and Command Contract - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase establishes the shared behavioral contract for the `thufs` CLI: command structure, token-based authentication flow, configuration loading rules, local/remote path semantics, and stdout/stderr plus exit-code behavior. It does not implement the core business commands themselves beyond the scaffolding needed to support them consistently.

</domain>

<decisions>
## Implementation Decisions

### Authentication entrypoint
- **D-01:** v1 authentication is token-driven rather than username/password login or browser-assisted login.
- **D-02:** The CLI should assume the user has already obtained a usable THU Cloud Drive token from outside `thufs`, then provide a CLI path to store and use it.

### Configuration and credential sourcing
- **D-03:** Configuration is file-first for normal usage, with environment variables acting as an override layer.
- **D-04:** The configuration model should be designed for both daily terminal usage and script execution, without making environment variables the primary UX.

### Remote path contract
- **D-05:** Remote paths must always support an explicit fully-qualified form.
- **D-06:** A default repo may be configured as a convenience shortcut, but command semantics must not depend on that shortcut being present.
- **D-07:** Planner and implementer should treat the explicit form as canonical and the default-repo form as syntactic sugar.

### Output and failure behavior
- **D-08:** Default command output is human-readable.
- **D-09:** Commands should provide `--json` for machine-readable output.
- **D-10:** Normal results go to stdout, errors go to stderr, and exit codes must remain stable for automation.
- **D-11:** The command contract should leave room for `--quiet` and `--verbose` style controls without undermining the default human-readable behavior.

### Command surface style
- **D-12:** High-frequency business commands stay short and flat: `push`, `pull`, `ls`, `share`.
- **D-13:** Management-oriented commands should be grouped, such as `auth ...` and `config ...`.
- **D-14:** The command tree should feel shell-first and compact for daily use, without letting administrative verbs sprawl into unrelated top-level commands.

### the agent's Discretion
- Exact config file format and filename
- Exact token-setting command spelling under `auth`
- Exact flag names for verbosity and JSON output
- Exact syntax of the fully-qualified remote path, as long as it preserves the explicit-vs-default-repo contract above

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` — Phase 1 goal, success criteria, and plan breakdown anchor
- `.planning/REQUIREMENTS.md` — `CONF-*` and `CLI-*` requirements that this phase must satisfy
- `.planning/PROJECT.md` — project-wide product boundaries and non-goals that constrain Phase 1 decisions
- `.planning/STATE.md` — current project position and active blocker around THU/Seafile compatibility validation

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No reusable code assets yet — repository is still at project-initialization stage.

### Established Patterns
- No code-level patterns exist yet; Phase 1 is responsible for defining the initial CLI and package structure.

### Integration Points
- New implementation will form the initial project structure and should align with the planned Go CLI layering described in `.planning/research/ARCHITECTURE.md`.

</code_context>

<specifics>
## Specific Ideas

- Keep the CLI Unix-style and script-friendly rather than sync-heavy or GUI-oriented.
- Remote paths should preserve an explicit form and may support a convenience form through a configured default repo.
- The desired command style is a hybrid: flat user workflows, grouped management workflows.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---
*Phase: 01-foundation-and-command-contract*
*Context gathered: 2026-04-21*
