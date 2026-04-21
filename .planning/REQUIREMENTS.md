# Requirements: thufs

**Defined:** 2026-04-21
**Core Value:** Terminal users can move files into and out of THU Cloud Drive with simple, reliable commands that are easy to script and hard to misuse.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Configuration

- [x] **CONF-01**: User can configure one THU Cloud Drive account for subsequent CLI usage
- [x] **CONF-02**: User can store authentication material in a local config format suitable for scriptable reuse
- [x] **CONF-03**: User can inspect or validate current configuration without performing a file transfer

### Navigation

- [x] **NAV-01**: User can list remote files and directories from THU Cloud Drive by path
- [x] **NAV-02**: User can distinguish files from directories in listing output
- [x] **NAV-03**: User can rely on `thufs ls` output in shell workflows without ambiguous failure states

### Transfer

- [x] **XFER-01**: User can upload a local file to a remote THU Cloud Drive path with `thufs push`
- [x] **XFER-02**: User can download a remote file from THU Cloud Drive to a local path with `thufs pull`
- [x] **XFER-03**: User receives clear failure behavior when source paths, target paths, or overwrites are invalid
- [x] **XFER-04**: Transfer commands return exit codes suitable for automation

### Sharing

- [ ] **SHARE-01**: User can create a share link for a remote THU Cloud Drive file with `thufs share`
- [ ] **SHARE-02**: User can set an expiration time when creating a share link
- [ ] **SHARE-03**: User can set a password when creating a share link
- [ ] **SHARE-04**: User receives the created share link in a format suitable for terminal and script usage

### CLI Experience

- [x] **CLI-01**: User can discover available commands and flags through built-in help output
- [x] **CLI-02**: User receives error messages on stderr and normal command results on stdout
- [x] **CLI-03**: User can use a consistent local-path/remote-path contract across `ls`, `push`, `pull`, and `share`

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Accounts

- **ACCT-01**: User can manage multiple THU Cloud Drive accounts or profiles

### Platform Scope

- **PLAT-01**: User can target non-THU Seafile instances

### Synchronization

- **SYNC-01**: User can synchronize local and remote directories bidirectionally

### Transfer Enhancements

- **XFER-05**: User can resume interrupted large transfers
- **XFER-06**: User can transfer directories recursively with explicit policy controls

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full sync daemon | Not aligned with the highest-value v1 use case |
| Generic Seafile client positioning | v1 is intentionally THU-specific |
| Multi-account switching | Deferred to keep auth and config simple |
| GUI/TUI interface | Product is intentionally shell-first |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CONF-01 | Phase 1 | Complete |
| CONF-02 | Phase 1 | Complete |
| CONF-03 | Phase 1 | Complete |
| CLI-01 | Phase 1 | Complete |
| CLI-02 | Phase 1 | Complete |
| CLI-03 | Phase 1 | Complete |
| NAV-01 | Phase 2 | Complete |
| NAV-02 | Phase 2 | Complete |
| NAV-03 | Phase 2 | Complete |
| XFER-01 | Phase 2 | Complete |
| XFER-02 | Phase 2 | Complete |
| XFER-03 | Phase 2 | Complete |
| XFER-04 | Phase 2 | Complete |
| SHARE-01 | Phase 3 | Pending |
| SHARE-02 | Phase 3 | Pending |
| SHARE-03 | Phase 3 | Pending |
| SHARE-04 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-21*
*Last updated: 2026-04-21 after Phase 2 execution*
