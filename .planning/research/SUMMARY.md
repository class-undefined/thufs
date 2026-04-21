# Project Research Summary

**Project:** thufs
**Domain:** Terminal-first THU Cloud Drive CLI on top of Seafile-compatible APIs
**Researched:** 2026-04-21
**Confidence:** MEDIUM

## Executive Summary

`thufs` should be built as a narrow, script-first CLI rather than a sync-heavy cloud-drive product. The most defensible architecture is a thin command layer over a small application service layer and a Seafile-focused API client, with explicit separation between backend behavior and user-facing command semantics.

Research and product inputs both point to the same conclusion: launch value comes from reliable `push`, `pull`, `ls`, and `share`, not from generalized Seafile abstraction or background synchronization. The major risks are not flashy technical unknowns; they are UX-contract failures around paths, auth, output stability, and sharing defaults.

## Key Findings

### Recommended Stack

Go is the strongest fit for a terminal-native, distributable binary that needs good HTTP, filesystem, and concurrency support. The Seafile Web API should be treated as the canonical backend integration surface, and the official Seafile Linux CLI should be used as a reference point for expected operator workflows rather than copied blindly.

**Core technologies:**
- Go: single-binary CLI implementation with strong stdlib support
- Seafile Web API: auth, listing, transfer, and share-link backend surface
- Cobra/Viper-style CLI/config stack: practical command and config ergonomics

### Expected Features

The table stakes for this product are straightforward and tightly aligned with the user interview.

**Must have (table stakes):**
- Single-account authentication/configuration
- Remote listing with script-friendly behavior
- Upload and download commands
- Share-link generation with password and expiration controls
- Reliable shell behavior via stable output and exit codes

**Should have (competitive):**
- THU-specific wording and defaults
- Clean Unix-style ergonomics that feel natural in scripts

**Defer (v2+):**
- Multi-account switching
- Generic Seafile support
- Sync-oriented workflows

### Architecture Approach

Use a layered CLI architecture: thin commands, service-level behavior, and an isolated Seafile client. Keep path resolution, transfer orchestration, and rendering as separate concerns so the product can evolve without destabilizing the command surface.

**Major components:**
1. CLI commands — parse flags and dispatch operations
2. Application services — enforce path, overwrite, and sharing semantics
3. Seafile client — encapsulate HTTP/API details
4. Config/auth layer — token persistence and environment integration

### Critical Pitfalls

1. **Ambiguous path semantics** — define and test local/remote path rules before transfer features expand.
2. **Auth that only works manually** — make config/token handling scriptable from the start.
3. **Unsafe transfer reporting** — formalize overwrite and partial-write behavior.
4. **Backend-shaped share UX** — design `share` around user intent, then map to API parameters.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Foundation and Command Contract
**Rationale:** path rules, config, auth, and base CLI ergonomics must stabilize before feature commands become trustworthy.
**Delivers:** project skeleton, config/auth, shared command/runtime conventions.
**Addresses:** auth, command contract, scriptability.
**Avoids:** path ambiguity and ad hoc auth behavior.

### Phase 2: Listing and Transfer Core
**Rationale:** `ls`, `push`, and `pull` are the highest-value operations and depend on the Phase 1 contract.
**Delivers:** remote inspection and file transfer behavior.
**Uses:** Seafile listing and transfer APIs.
**Implements:** transfer and path-resolution services.

### Phase 3: Share Workflow
**Rationale:** share-link features build naturally once remote object resolution is reliable.
**Delivers:** link generation with password and expiration support.
**Uses:** Seafile share-link APIs.
**Implements:** share service and safe CLI defaults.

### Phase 4: Hardening and Release Readiness
**Rationale:** CLI tools fail on edge behavior more often than feature absence.
**Delivers:** testing, docs, packaging polish, and operational confidence.

### Phase Ordering Rationale

- Auth/config and path semantics are prerequisites for every core command.
- Transfer commands must be complete before share UX is finalized, because object resolution behavior is shared.
- Hardening deserves its own phase because script-facing tools need stricter regression protection than demo-ready apps.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** exact Seafile upload/download endpoint behavior and THU-specific compatibility details.
- **Phase 3:** share-link API details and supported controls on the target deployment.

Phases with standard patterns:
- **Phase 1:** CLI structure, config, and command contract design are well-understood.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM | Tooling choice is strong, but exact module versions can be resolved later |
| Features | HIGH | Directly grounded in user-stated priorities |
| Architecture | MEDIUM | Clear pattern, but exact package layout depends on chosen implementation details |
| Pitfalls | MEDIUM | Based on common CLI integration failure modes plus user constraints |

**Overall confidence:** MEDIUM

### Gaps to Address

- Exact THU Cloud Drive auth and API compatibility details need confirmation during implementation.
- Recursive directory semantics and object-type support boundaries should be locked during planning.

## Sources

### Primary (HIGH confidence)
- User initialization answers — product scope and priorities
- https://help.seafile.com/syncing_client/linux-cli/ — official CLI reference point

### Secondary (MEDIUM confidence)
- https://manual.seafile.com/latest/develop/web_api_v2.1/ — official Seafile API reference entry point

### Tertiary (LOW confidence)
- Inferred CLI architecture patterns from comparable Unix tools

---
*Research completed: 2026-04-21*
*Ready for roadmap: yes*
