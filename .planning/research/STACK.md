# Stack Research

**Domain:** Terminal-first cloud drive CLI on top of Seafile-compatible APIs
**Researched:** 2026-04-21
**Confidence:** MEDIUM

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Go | 1.25.x | Primary implementation language for the CLI | Produces fast single-binary tooling, has strong stdlib support for HTTP, files, archives, and concurrency, and is a common fit for Unix-style CLIs |
| Seafile Web API | v2.1 / Server 11.x-compatible endpoints | Backend integration surface for auth, directory listing, upload/download, and share-link operations | THU Cloud Drive is Seafile-based, so the safest integration path is to align with official Seafile API patterns rather than reverse-engineering browser behavior |
| POSIX shell compatibility mindset | n/a | UX constraint for flags, output, and exit behavior | The target audience uses shells and scripts, so operational predictability matters as much as raw API support |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/spf13/cobra` | v1.x | Command tree, flags, help, shell completion | Use for the top-level `thufs` command surface |
| `github.com/spf13/viper` | v1.x | Config loading from file and env | Use for token, server metadata, output preferences, and future extensibility |
| `github.com/stretchr/testify` | v1.x | Tests for API client and command behavior | Use in unit and integration-style tests |
| `golang.org/x/sync/errgroup` | current | Coordinated concurrent operations | Use for batched transfers or future parallel listing/download flows |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `gofmt` | Canonical formatting | Non-negotiable for repo consistency |
| `go test` | Unit/integration verification | Keep API client and path handling covered early |
| `golangci-lint` | Static analysis | Useful once codebase structure exists |

## Installation

```bash
# Core
go mod init github.com/zhitai/thufs

# Supporting
go get github.com/spf13/cobra@latest
go get github.com/spf13/viper@latest
go get golang.org/x/sync@latest

# Dev dependencies
go get github.com/stretchr/testify@latest
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Go | Python | Use Python only if rapid scripting and local interpreter dependency are acceptable |
| Cobra | `urfave/cli` | Use when a flatter, less opinionated CLI structure is preferred |
| Direct Seafile Web API client | Browser automation | Only if the target instance blocks documented API flows, which should be treated as an exception path |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Desktop sync-client architecture as the initial design | Over-optimizes for continuous synchronization, which is not the stated core use case | Explicit command-driven transfer operations |
| Generic multi-provider abstraction in v1 | Adds design weight before product fit for THU workflows is validated | THU-specific API and UX shaping over a Seafile-aware client |
| Parsing human-oriented command output internally | Fragile for scripting and testing | Structured internal data models with explicit renderers |

## Stack Patterns by Variant

**If THU Cloud Drive token-based auth is sufficient:**
- Use static token storage with explicit config path
- Because v1 is single-account and script-first

**If server quirks differ from stock Seafile behavior:**
- Isolate compatibility logic in a transport or API adapter layer
- Because product scope is THU-specific but implementation should stay maintainable

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| Go 1.25.x | Current Cobra/Viper/testify releases | Validate exact module versions when code is initialized |
| Seafile Web API v2.1 patterns | Seafile Server 11.x-derived deployments | THU-specific behavior still needs validation during implementation |

## Sources

- https://manual.seafile.com/latest/develop/web_api_v2.1/ — official Seafile API overview entry point
- https://help.seafile.com/syncing_client/linux-cli/ — official Seafile Linux CLI manual, useful as a UX reference point
- User-provided product intent — high-confidence source for scope and workflow priority

---
*Stack research for: Terminal-first cloud drive CLI on top of Seafile-compatible APIs*
*Researched: 2026-04-21*
