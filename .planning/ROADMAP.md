# Roadmap: thufs

## Overview

The roadmap starts by locking down the command contract, configuration, and Seafile-facing client boundary, then builds the high-value user workflows in the order they depend on each other: inspection and transfer first, sharing next, and hardening last. This keeps the project aligned with its Unix-style value proposition while avoiding premature work on sync or generic platform abstractions.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation and Command Contract** - Establish CLI structure, auth/config model, and path semantics.
- [x] **Phase 2: Listing and Transfer Core** - Deliver `ls`, `push`, and `pull` on top of the Seafile-backed client.
- [x] **Phase 3: Share Workflow** - Deliver `thufs share` with password and expiration support.
- [ ] **Phase 4: Hardening and Release Readiness** - Lock in reliability, tests, docs, and packaging behavior for real shell use.

## Phase Details

### Phase 1: Foundation and Command Contract
**Goal**: Create the initial CLI skeleton and the stable behavioral contract for auth, config, help, output, and path handling.
**Depends on**: Nothing (first phase)
**Requirements**: [CONF-01, CONF-02, CONF-03, CLI-01, CLI-02, CLI-03]
**Success Criteria** (what must be TRUE):
  1. User can configure one THU Cloud Drive account for future CLI use.
  2. User can run the CLI help and configuration commands without ambiguous output behavior.
  3. Local and remote path conventions are documented and enforced consistently by shared command logic.
**Plans**: 3 plans

Plans:
- [x] 01-01: Initialize Rust CLI project structure and shared command/runtime plumbing
- [x] 01-02: Implement config and authentication handling for a single THU account
- [x] 01-03: Define and test command contract, output discipline, and path semantics

### Phase 2: Listing and Transfer Core
**Goal**: Implement the core remote inspection and file transfer workflows that define v1 value.
**Depends on**: Phase 1
**Requirements**: [NAV-01, NAV-02, NAV-03, XFER-01, XFER-02, XFER-03, XFER-04]
**Success Criteria** (what must be TRUE):
  1. User can list remote THU Cloud Drive paths with clear file/directory output.
  2. User can upload and download files with `thufs push` and `thufs pull`.
  3. Invalid paths, overwrite conflicts, and transfer failures produce clear stderr output and correct exit codes.
**Plans**: 4 plans

Plans:
- [x] 02-01: Implement Seafile repository and path resolution client operations
- [x] 02-02: Build `thufs ls` with stable output and failure semantics
- [x] 02-03: Build `thufs push` upload workflow and overwrite/path validation
- [x] 02-04: Build `thufs pull` download workflow and local write safety behavior

### Phase 3: Share Workflow
**Goal**: Add a share-link command that fits terminal workflows and exposes the intended v1 controls.
**Depends on**: Phase 2
**Requirements**: [SHARE-01, SHARE-02, SHARE-03, SHARE-04]
**Success Criteria** (what must be TRUE):
  1. User can create a share link for a supported remote target with a single CLI command.
  2. User can set password and expiration controls through explicit flags.
  3. Command output is usable directly in scripts or copy-paste terminal workflows.
**Plans**: 2 plans

Plans:
- [x] 03-01: Implement share-link API client behavior and remote target validation
- [x] 03-02: Build `thufs share` command UX, output behavior, and regression coverage

### Phase 4: Hardening and Release Readiness
**Goal**: Make the CLI reliable enough for real shell automation and prepare the project for initial release.
**Depends on**: Phase 3
**Requirements**: [CONF-01, NAV-03, XFER-03, XFER-04, SHARE-04, CLI-02, CLI-03]
**Success Criteria** (what must be TRUE):
  1. Core commands have regression coverage for success paths and critical failure paths.
  2. Documentation explains setup, command usage, and behavioral constraints clearly enough for a new terminal user.
  3. The project is packaged and structured so the initial release can be built and used consistently.
**Plans**: 3 plans

Plans:
- [ ] 04-01: Add integration-style test coverage and fixtures for auth, transfer, and share behavior
- [ ] 04-02: Write operator-facing documentation and examples for common workflows
- [ ] 04-03: Finalize release packaging, installation flow, and quality checks

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation and Command Contract | 3/3 | Complete | 2026-04-21 |
| 2. Listing and Transfer Core | 4/4 | Complete | 2026-04-21 |
| 3. Share Workflow | 2/2 | Complete | 2026-04-21 |
| 4. Hardening and Release Readiness | 0/3 | Not started | - |
