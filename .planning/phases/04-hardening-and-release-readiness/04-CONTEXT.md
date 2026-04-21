---
phase: 04-hardening-and-release-readiness
status: discussed
created: 2026-04-21
---

# Phase 4 Context: Hardening And Release Readiness

## Goal

Make the v1 CLI coherent for real shell use: commands are wired through services, tests cover critical contracts, README documents current behavior, and release metadata is sufficient for local builds.

## Hardening Focus

- Replace `thufs ls` placeholder CLI data with real Seafile client calls.
- Keep deterministic tests for local validation paths that should fail before network calls.
- Document auth, config, remote paths, and all v1 business commands.
- Preserve known limitation: upstream Seafile endpoint compatibility still needs live THU Cloud Drive validation.

