# Pitfalls Research

**Domain:** THU Cloud Drive CLI over Seafile-compatible APIs
**Researched:** 2026-04-21
**Confidence:** MEDIUM

## Critical Pitfalls

### Pitfall 1: Path semantics become ambiguous

**What goes wrong:**
Users cannot predict whether a command interprets a target as file vs directory, or how overwrites are handled.

**Why it happens:**
CLI tools often mirror backend API details instead of defining a clear user-facing contract.

**How to avoid:**
Define and document strict local/remote path rules early, and test them with realistic shell scenarios.

**Warning signs:**
Command docs need caveats to explain “special cases,” or tests start hardcoding ad hoc path exceptions.

**Phase to address:**
Phase 1

---

### Pitfall 2: Auth is “good enough” for manual use but bad for scripts

**What goes wrong:**
The tool works interactively once, but token storage, expiry handling, and non-interactive invocation are brittle.

**Why it happens:**
Too much focus goes into file operations before deciding how scripts should obtain credentials safely.

**How to avoid:**
Establish a minimal but explicit config/token model before implementing transfer commands.

**Warning signs:**
Commands require manual environment setup every time, or auth failures are not actionable.

**Phase to address:**
Phase 1

---

### Pitfall 3: Share workflow exposes backend details instead of user intent

**What goes wrong:**
Users can technically create links, but password/expiration behavior is confusing or inconsistent.

**Why it happens:**
The implementation follows raw API parameters without designing a clean CLI contract.

**How to avoid:**
Design `thufs share` around explicit user tasks and safe defaults, then map that cleanly to API calls.

**Warning signs:**
Flags are named after backend fields, or users need multiple invocations to get a usable share link.

**Phase to address:**
Phase 3

---

### Pitfall 4: Transfer success is reported before data safety is clear

**What goes wrong:**
Commands appear successful even when files were partially written, overwritten unexpectedly, or failed mid-stream.

**Why it happens:**
CLI implementations optimize for the happy path and under-specify failure and collision behavior.

**How to avoid:**
Make local write policy, temporary-file handling, and exit-code semantics explicit and test them.

**Warning signs:**
Interrupted downloads leave corrupt files with final names, or uploads report success without a confirmed remote result.

**Phase to address:**
Phase 2

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcoding THU endpoints in multiple places | Fast initial progress | Painful compatibility changes later | Never |
| Printing directly from service code | Less boilerplate | Unstable output and poor testability | Never |
| Skipping fixture-based API tests | Faster early coding | Fragile regressions in transfer/share behavior | Only for throwaway spikes |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Seafile auth | Assuming one login flow covers all usage | Isolate auth/config and verify scriptable behavior |
| Upload flow | Treating upload as a single raw POST without endpoint-specific setup | Encapsulate the full upload sequence in the API client |
| Share links | Assuming file and directory sharing semantics are identical | Design and validate the exact supported v1 object types |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Loading whole files into memory | Large transfers spike memory use | Stream file I/O | Medium to large files |
| Unbounded parallel transfers | Rate limits or unstable failures | Add concurrency limits and retries | Batch scripts |
| Repeated remote path discovery | Slow scripts with many operations | Cache within a command execution when safe | Multi-object operations |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Echoing tokens or secrets in normal output | Credential leak in shell history/logs | Redact secrets and separate stderr carefully |
| Creating share links with weak defaults | Accidental overexposure of files | Require explicit flags or safe defaults for password/expiry |
| Storing tokens with loose file permissions | Local credential exposure | Enforce restrictive config file permissions |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Backend-centric naming | Commands feel alien to THU users | Use THU-facing terminology in docs/help |
| Mixed human/script output | Automation becomes brittle | Keep predictable output modes and stderr discipline |
| Hidden overwrite behavior | Data loss or mistrust | Make overwrite policy explicit in flags and docs |

## "Looks Done But Isn't" Checklist

- [ ] **Auth:** often missing non-interactive behavior — verify scripted command usage works cleanly.
- [ ] **Push/Pull:** often missing collision policy — verify overwrite and destination handling.
- [ ] **Share:** often missing password/expiry ergonomics — verify one command can create a usable controlled link.
- [ ] **CLI UX:** often missing stable exit codes — verify failure cases are machine-detectable.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Path contract confusion | MEDIUM | Freeze semantics, update docs/tests, and avoid silent behavior changes |
| Weak auth/config design | MEDIUM | Add migration path for config layout and improve token handling |
| Corrupt transfer handling | HIGH | Introduce temp-file semantics, explicit retries, and regression tests |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Path ambiguity | Phase 1 | Command contract and path tests exist |
| Auth unsuitable for scripts | Phase 1 | Configured command works non-interactively |
| Unsafe transfer behavior | Phase 2 | Download/upload failure modes are tested |
| Share UX mismatch | Phase 3 | Share command supports usable password/expiry workflow |

## Sources

- Official Seafile API and CLI documentation
- User-provided workflow goals
- Common failure modes inferred from CLI integration work

---
*Pitfalls research for: THU Cloud Drive CLI over Seafile-compatible APIs*
*Researched: 2026-04-21*
