# Phase 2: Listing and Transfer Core - Research

**Researched:** 2026-04-22
**Domain:** Rust CLI business commands over Seafile-compatible HTTP APIs
**Confidence:** MEDIUM

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-02-01:** Phase 2 should use THU Cloud Drive's Seafile-compatible HTTP APIs directly rather than shelling out to another CLI.
- **D-02-02:** The token configured in Phase 1 should be treated as an account token used in `Authorization: Token ...` style requests.
- **D-02-03:** Repository discovery and path resolution should be implemented as reusable client operations before wiring `ls`, `push`, and `pull`.
- **D-02-04:** Explicit `repo:<library>/<path>` remains the canonical remote path form.
- **D-02-05:** Commands may use the configured default repo as shorthand, but must never require it.
- **D-02-06:** Phase 2 only needs to support single-file upload and single-file download; recursive directory transfer is out of scope.
- **D-02-07:** `thufs ls` should return deterministic directory entries suitable for both human terminal use and shell scripting.
- **D-02-08:** Listing output must let users distinguish files from directories without ambiguous heuristics.
- **D-02-09:** `ls` should operate against remote directories, and file-target behavior should be explicit rather than silently coerced.
- **D-02-10:** `push` and `pull` should prioritize explicit overwrite and path validation over optimistic convenience behavior.
- **D-02-11:** Phase 2 should prefer correctness and clear failure messages over batching or aggressive parallel transfer behavior.
- **D-02-12:** Download behavior should avoid reporting success before the final destination file is safely written.
- **D-02-13:** Human-readable output remains the default; `--json` must work for newly introduced commands too.
- **D-02-14:** Normal results stay on stdout, error messages stay on stderr, and transfer/listing failures must map to stable non-zero exits.
- **D-02-15:** The CLI should keep a shell-first UX: simple verbs, predictable output, and no hidden interactive prompts.

### the agent's Discretion
- Exact repository selection strategy when multiple libraries share similar names
- Exact JSON field layout for `ls`, `push`, and `pull`
- Exact local temporary-file strategy for downloads
- Exact overwrite flag spelling and conflict messaging

</user_constraints>

<research_summary>
## Summary

Phase 2 should build one reusable Seafile client boundary that covers four core operations: repository lookup, directory listing, upload link / update flow, and download link resolution. The CLI commands should stay thin and drive these operations through Rust services that consume the Phase 1 config and remote-path contracts.

The highest-risk area is not the HTTP wiring itself, but contract drift: if `ls`, `push`, and `pull` each invent their own repo lookup, overwrite policy, or output format, Phase 2 will create the same inconsistency debt that Phase 1 was explicitly designed to prevent. The implementation should therefore establish a single remote resolution path and a single transfer result model before the user-facing commands are finalized.

**Primary recommendation:** Split Phase 2 into infrastructure-first work: Seafile client + remote resolution, then `ls`, then `push`, then `pull`, each backed by tests that lock error handling and output behavior.
</research_summary>

<official_api_notes>
## Official Seafile API Notes

Using official Seafile documentation and reference pages as the primary source, the relevant API patterns for this phase are:

- Account-token based requests with `Authorization: Token <token>`
- Repository/library listing endpoints to discover repo IDs and names
- Directory entry listing endpoints under a repo and path
- Upload/update link retrieval before sending multipart upload requests
- Download-link retrieval before streaming file content to disk

Implementation inference from the docs:
- Library name to repo ID resolution is a necessary shared step because user-facing remote paths are name-oriented while low-level APIs typically use repo IDs.
- Upload is not a single generic POST; it requires an upload-link or update-link flow first.
- Download should likely resolve a temporary download URL before streaming the file body, rather than constructing the final file endpoint ad hoc.

</official_api_notes>

<recommended_stack>
## Recommended Rust Stack

| Crate | Purpose | Why |
|------|---------|-----|
| `reqwest` | HTTP client and streaming download/upload support | Standard Rust choice for authenticated HTTP + multipart |
| `tokio` | Async runtime | Pairs naturally with `reqwest` for file and network I/O |
| `serde` / `serde_json` | Response parsing | Already used in the project |
| `thiserror` or `anyhow` | Error typing and propagation | `anyhow` already exists; typed user-facing errors can be layered later |
| `tempfile` | Safe temporary download path handling | Useful for write-then-rename download flow |

</recommended_stack>

<architecture_patterns>
## Architecture Patterns

### Pattern 1: Remote reference resolves before business logic
`ls`, `push`, and `pull` should all take a parsed `RemoteRef`, then run the same repo-resolution step to obtain the concrete repo ID and normalized remote path.

### Pattern 2: Client boundary encapsulates Seafile flow details
The Seafile client should own:
- auth header creation
- repo lookup
- directory listing requests
- upload-link / update-link lookup
- download-link lookup

The command and service layers should not know Seafile endpoint choreography.

### Pattern 3: Transfer safety is explicit
For uploads and downloads:
- validate source and destination shape before network work
- refuse overwrite by default unless an explicit flag is present
- only print success after the final state is durable

</architecture_patterns>

<common_pitfalls>
## Common Pitfalls

### Pitfall 1: Repo-name resolution becomes ambiguous
If multiple repos can match user-facing input loosely, commands may act on the wrong library.
Avoid by making repo matching deterministic and failing loudly on ambiguity.

### Pitfall 2: Upload/download flows leak backend quirks into CLI behavior
If command handlers know too much about upload-link or download-link details, output and error handling will drift.
Avoid by putting Seafile-specific choreography behind a shared client.

### Pitfall 3: Downloads report success before file safety is guaranteed
Streaming directly to the final path risks partial files after interruption.
Avoid by writing to a temp file and renaming only after the stream completes successfully.

### Pitfall 4: `ls` output becomes pretty but unusable
A human-only table format will weaken shell usage.
Avoid by keeping the default format compact and adding `--json` for structured consumers.

</common_pitfalls>

<implementation_guidance>
## Implementation Guidance

- Add a `src/seafile.rs` or `src/seafile/mod.rs` boundary with request/response structs for repo, directory, upload, and download operations.
- Add application services for list/upload/download so CLI modules only handle argument parsing and rendering.
- Extend `RemoteRef` if needed, but keep a single canonical parse path.
- Add CLI integration tests for the new command surfaces and unit tests for repo/path resolution and overwrite logic.

</implementation_guidance>

<sources>
## Sources

- Official Seafile documentation and API reference/manual pages for repository, directory, upload-link, update-link, and download-link workflows
- Existing project Phase 1 contracts and summaries

</sources>

---
*Phase: 02-listing-and-transfer-core*
*Researched: 2026-04-22*
