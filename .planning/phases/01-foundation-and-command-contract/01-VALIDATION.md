---
phase: 1
slug: foundation-and-command-contract
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `go test` |
| **Config file** | `go.mod` / `go.sum` (Wave 0 installs if missing) |
| **Quick run command** | `go test ./...` |
| **Full suite command** | `go test ./...` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test ./...`
- **After every plan wave:** Run `go test ./...`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | CONF-01, CLI-01 | T-1-01 / T-1-02 | bootstrap commands do not leak secrets and help output is deterministic | unit | `go test ./...` | ❌ W0 | ⬜ pending |
| 1-01-02 | 02 | 2 | CONF-01, CONF-02, CONF-03 | T-1-03 / T-1-04 | token storage uses restrictive permissions and config/env precedence is enforced | unit | `go test ./...` | ❌ W0 | ⬜ pending |
| 1-01-03 | 03 | 3 | CLI-02, CLI-03 | T-1-05 / T-1-06 | path parsing and stdout/stderr discipline are regression-tested | unit | `go test ./...` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `go.mod` — initialize module metadata for `thufs`
- [ ] `go.sum` — dependency lockfile after first dependency install
- [ ] `internal/.../*_test.go` — stubs covering config precedence, path parsing, and output behavior

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI help feels intuitive for shell users | CLI-01 | wording and discoverability need a human check | Run `thufs --help`, `thufs auth --help`, and `thufs config --help`; confirm top-level verbs are flat and management verbs are grouped |

*If none: "All phase behaviors have automated verification."*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
