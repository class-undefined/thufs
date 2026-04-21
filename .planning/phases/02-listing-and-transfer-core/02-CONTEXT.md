# Phase 2: Listing and Transfer Core - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers the first business commands that make `thufs` useful in daily terminal workflows: `ls`, `push`, and `pull`. It covers Seafile-backed repository discovery, remote path resolution, remote listing, single-file upload, and single-file download. It does not include sharing, recursive directory transfer, resumable transfer, or sync behavior.

</domain>

<decisions>
## Implementation Decisions

### Seafile API integration
- **D-02-01:** Phase 2 should use THU Cloud Drive's Seafile-compatible HTTP APIs directly rather than shelling out to another CLI.
- **D-02-02:** The token configured in Phase 1 should be treated as an account token used in `Authorization: Token ...` style requests.
- **D-02-03:** Repository discovery and path resolution should be implemented as reusable client operations before wiring `ls`, `push`, and `pull`.

### Remote target semantics
- **D-02-04:** Explicit `repo:<library>/<path>` remains the canonical remote path form.
- **D-02-05:** Commands may use the configured default repo as shorthand, but must never require it.
- **D-02-06:** Phase 2 only needs to support single-file upload and single-file download; recursive directory transfer is out of scope.

### Listing behavior
- **D-02-07:** `thufs ls` should return deterministic directory entries suitable for both human terminal use and shell scripting.
- **D-02-08:** Listing output must let users distinguish files from directories without ambiguous heuristics.
- **D-02-09:** `ls` should operate against remote directories, and file-target behavior should be explicit rather than silently coerced.

### Upload and download behavior
- **D-02-10:** `push` and `pull` should prioritize explicit overwrite and path validation over optimistic convenience behavior.
- **D-02-11:** Phase 2 should prefer correctness and clear failure messages over batching or aggressive parallel transfer behavior.
- **D-02-12:** Download behavior should avoid reporting success before the final destination file is safely written.

### Output and errors
- **D-02-13:** Human-readable output remains the default; `--json` must work for newly introduced commands too.
- **D-02-14:** Normal results stay on stdout, error messages stay on stderr, and transfer/listing failures must map to stable non-zero exits.
- **D-02-15:** The CLI should keep a shell-first UX: simple verbs, predictable output, and no hidden interactive prompts.

### the agent's Discretion
- Exact repository selection strategy when multiple libraries share similar names
- Exact JSON field layout for `ls`, `push`, and `pull`
- Exact local temporary-file strategy for downloads
- Exact overwrite flag spelling and conflict messaging

</decisions>

<canonical_refs>
## Canonical References

### Product and requirement anchors
- `.planning/ROADMAP.md` — Phase 2 goal, requirements, and success criteria
- `.planning/REQUIREMENTS.md` — `NAV-*` and `XFER-*` requirements
- `.planning/PROJECT.md` — THU-only, shell-first, single-account product boundary
- `.planning/STATE.md` — current milestone state and Phase 1 completion context

### Prior phase contracts
- `.planning/phases/01-foundation-and-command-contract/01-CONTEXT.md`
- `.planning/phases/01-foundation-and-command-contract/01-01-SUMMARY.md`
- `.planning/phases/01-foundation-and-command-contract/01-02-SUMMARY.md`
- `.planning/phases/01-foundation-and-command-contract/01-03-SUMMARY.md`

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/config.rs` already resolves token, default repo, and output mode with file-first/env-override behavior.
- `src/contract.rs` already provides canonical remote path parsing and default-repo shorthand handling.
- `src/output.rs` already centralizes text/JSON rendering and token redaction helpers.
- `src/cli/root.rs` already exposes the global `--json` flag and grouped management command structure.

### Established Patterns
- Thin CLI handlers delegate to application services.
- Command output goes through a renderer rather than ad hoc printing scattered across service code.
- Behavior contracts are locked with unit tests and CLI integration tests.

### Integration Points
- New work should add a Seafile/THU client boundary under `src/seafile.rs` or a similar dedicated module.
- `ls`, `push`, and `pull` should plug into the existing Rust CLI command tree as flat business commands.
- Listing and transfer commands should consume `RemoteRef` rather than inventing new path parsing logic.

</code_context>

<specifics>
## Specific Ideas

- Keep `ls` output compact and script-friendly; avoid decorative tables in the default mode.
- Treat overwrite safety as a first-class decision for both upload and download flows.
- Build repo/path resolution once and reuse it across all three commands.
- Favor streaming file I/O for transfer commands instead of buffering whole files in memory.

</specifics>

<deferred>
## Deferred Ideas

- Recursive upload/download
- Resume support for interrupted transfers
- Multi-file batch commands
- Generic Seafile instance support beyond THU Cloud Drive

</deferred>

---
*Phase: 02-listing-and-transfer-core*
*Context gathered: 2026-04-22*
