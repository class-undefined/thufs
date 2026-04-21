# Feature Research

**Domain:** Terminal-first THU Cloud Drive CLI
**Researched:** 2026-04-21
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Token-based login/configuration | CLI users need a way to authenticate once and script afterwards | LOW | Must stay simple in v1 because account model is single-account |
| Remote path listing | Users need to inspect cloud state before scripting transfers | LOW | `thufs ls` should support script-friendly output |
| File upload | Core user demand | MEDIUM | Must handle both single file and directory upload semantics clearly |
| File download | Core user demand | MEDIUM | Must resolve local overwrite and destination behavior explicitly |
| Share-link generation | Essential for operational collaboration | MEDIUM | v1 includes expiration and password support |
| Clear exit codes and error messages | Required for shell automation | LOW | Not optional for the target audience |

### Differentiators (Competitive Advantage)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| THU-specific defaults and terminology | Reduces friction compared with a generic Seafile client | LOW | Naming and docs should reflect THU Cloud Drive directly |
| Strong Unix-style command ergonomics | Makes the tool feel native in shell workflows | MEDIUM | Includes stable flags, stdout/stderr discipline, and composable output |
| Lightweight share workflow | Makes cloud sharing practical in scripts and terminal sessions | MEDIUM | Can become a standout workflow if output is copy-paste friendly |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full sync daemon | Sounds like the “complete” cloud-drive experience | Adds major state, conflict, watcher, and recovery complexity outside the stated v1 goal | Keep explicit `push`/`pull` operations first |
| Multi-provider cloud abstraction | Feels extensible | Forces lowest-common-denominator design before THU-specific UX is validated | Stay THU-only in v1 |
| Multi-account switching in v1 | Useful for power users | Adds config, auth, and UX branching before core workflows are proven | Single-account config now, profiles later if demand emerges |

## Feature Dependencies

```text
Authentication/config
    └──requires──> API client transport
                        ├──supports──> ls
                        ├──supports──> push
                        ├──supports──> pull
                        └──supports──> share

share
    └──requires──> remote path resolution

push/pull
    └──requires──> clear path semantics and overwrite policy
```

### Dependency Notes

- **All user-facing commands require authentication/config:** auth must be solved before command implementation is meaningful.
- **`share` requires remote object resolution:** the CLI must consistently locate a target file before generating links or applying share controls.
- **Transfer commands depend on path semantics:** ambiguity around local vs remote paths will create user-facing breakage faster than raw API bugs.

## MVP Definition

### Launch With (v1)

- [ ] Single-account authentication and config storage — required for all other commands
- [ ] `thufs ls` for remote inspection — required for scriptable discovery
- [ ] `thufs push` for upload workflows — explicitly requested
- [ ] `thufs pull` for download workflows — explicitly requested
- [ ] `thufs share` with password and expiration controls — explicitly requested
- [ ] Stable automation behavior — stdout/stderr/exit codes fit scripting usage

### Add After Validation (v1.x)

- [ ] Recursive directory convenience improvements — add if real user workflows need it
- [ ] JSON output modes for selected commands — add once command shapes stabilize
- [ ] Resume/retry ergonomics for large transfers — add when transfer reliability gaps are observed

### Future Consideration (v2+)

- [ ] Multi-account/profile switching — defer until core usage validates the need
- [ ] Generic Seafile instance support — defer until THU-specific workflow is strong
- [ ] Sync-oriented commands — defer unless demand materially changes

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Single-account auth/config | HIGH | LOW | P1 |
| Remote listing | HIGH | LOW | P1 |
| Upload | HIGH | MEDIUM | P1 |
| Download | HIGH | MEDIUM | P1 |
| Share-link creation with controls | HIGH | MEDIUM | P1 |
| JSON output refinements | MEDIUM | LOW | P2 |
| Multi-account support | MEDIUM | MEDIUM | P3 |
| Sync engine | LOW | HIGH | P3 |

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| Cloud transfers | Generic Seafile tooling exposes backend-centric verbs | Desktop clients optimize for sync and GUI | Build THU-focused terminal verbs around direct transfer jobs |
| Sharing | Many tools expose only raw link creation | GUI products bury options behind clicks | Make share password/expiration first-class CLI flags |
| CLI UX | Existing tools may reflect implementation details | Generic CLIs rarely target THU-specific workflows | Prioritize intuitive THU user mental model |

## Sources

- User initialization answers
- Official Seafile Linux CLI documentation
- Official Seafile Web API documentation

---
*Feature research for: Terminal-first THU Cloud Drive CLI*
*Researched: 2026-04-21*
