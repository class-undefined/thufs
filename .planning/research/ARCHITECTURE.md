# Architecture Research

**Domain:** CLI client for THU Cloud Drive over Seafile-compatible APIs
**Researched:** 2026-04-21
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                      CLI Command Layer                      │
├─────────────────────────────────────────────────────────────┤
│  push      pull      ls      share      auth/config        │
├─────────────────────────────────────────────────────────────┤
│                    Command Service Layer                    │
├─────────────────────────────────────────────────────────────┤
│  path resolver   transfer service   share service          │
│  output renderer config loader      error mapping          │
├─────────────────────────────────────────────────────────────┤
│                     Seafile API Client                      │
├─────────────────────────────────────────────────────────────┤
│  auth      repos/dirs      upload/download      share      │
├─────────────────────────────────────────────────────────────┤
│                 Local OS / THU Cloud Drive                  │
│  filesystem  config store  stdout/stderr  remote storage   │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| CLI command layer | Parse args/flags and invoke workflows | Cobra commands with minimal logic |
| Service layer | Enforce product semantics and user-facing behavior | Plain Go services with typed request/response structs |
| API client | Encapsulate Seafile HTTP endpoints and compatibility quirks | `net/http` client with endpoint-focused methods |
| Output/rendering | Human and machine-friendly formatting | Dedicated renderer package, no direct printing in business logic |
| Config/auth | Persist token and server metadata | Config file + env overrides |

## Recommended Project Structure

```text
cmd/
├── thufs/             # CLI bootstrap

internal/
├── cli/               # command definitions and flag parsing
├── app/               # application services / use cases
├── seafile/           # API client and endpoint handling
├── config/            # config loading and token storage
├── output/            # text/json rendering helpers
├── transfer/          # upload/download orchestration
└── share/             # share-link operations

test/
└── fixtures/          # sample API payloads and command scenarios
```

### Structure Rationale

- **`cmd/`**: keeps binary bootstrap thin and conventional.
- **`internal/cli/`**: isolates command composition from backend logic.
- **`internal/seafile/`**: gives a clear boundary around THU/Seafile API behavior.
- **`internal/app/`**: prevents command handlers from accumulating business logic.
- **`internal/output/`**: keeps script-facing output stable and testable.

## Architectural Patterns

### Pattern 1: Thin Commands, Rich Services

**What:** command handlers translate flags into typed service requests.
**When to use:** all user-facing commands.
**Trade-offs:** slightly more structure up front, much easier testing and evolution later.

### Pattern 2: Endpoint-Oriented API Client

**What:** model Seafile operations as explicit client methods instead of generic request builders everywhere.
**When to use:** auth, listing, upload/download, sharing.
**Trade-offs:** more methods to maintain, but much clearer failure handling and compatibility patches.

### Pattern 3: Renderer Separation

**What:** domain logic returns structured results; renderer decides stdout/stderr formatting.
**When to use:** any command that may later need machine-readable output.
**Trade-offs:** slightly more plumbing, far better script stability.

## Data Flow

### Request Flow

```text
User command
    ↓
CLI parser
    ↓
Service request
    ↓
Seafile API client
    ↓
THU Cloud Drive / Seafile server
    ↓
Structured response
    ↓
Renderer
    ↓
stdout/stderr + exit code
```

### Key Data Flows

1. **Upload flow:** local path validation → remote target resolution → upload-link/API operation → result rendering.
2. **Download flow:** remote target resolution → download request/link retrieval → local write policy enforcement → result rendering.
3. **Share flow:** remote file resolution → share creation with options → normalized share output.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single-user CLI | Simple synchronous flow is acceptable |
| Frequent scripted batch jobs | Add concurrency controls and retry policy around transfer operations |
| Large transfer workloads | Add resumability, chunk-aware progress, and stronger temporary-file handling |

### Scaling Priorities

1. **First bottleneck:** transfer reliability and error recovery.
2. **Second bottleneck:** path resolution and batch command ergonomics for scripted workflows.

## Anti-Patterns

### Anti-Pattern 1: Business Logic in Cobra Commands

**What people do:** implement all logic inside `RunE`.
**Why it's wrong:** makes tests brittle and mixes parsing with behavior.
**Do this instead:** push behavior into service packages and keep commands thin.

### Anti-Pattern 2: Treating Remote Paths Like Local Paths

**What people do:** assume identical normalization and overwrite semantics.
**Why it's wrong:** remote storage APIs often differ in path rules and conflict behavior.
**Do this instead:** centralize remote path resolution and collision policy.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| THU Cloud Drive / Seafile server | HTTP API client with token auth | Core dependency for all operations |
| Local filesystem | Read/write with explicit safety checks | Needed for push/pull workflows |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `cli` ↔ `app` | typed request structs | keeps flags separate from behavior |
| `app` ↔ `seafile` | client interfaces | useful for tests and compatibility shims |
| `app` ↔ `output` | structured result objects | keeps output policy testable |

## Sources

- Official Seafile Web API documentation
- Official Seafile Linux CLI documentation
- User-provided product scope and workflow priorities

---
*Architecture research for: CLI client for THU Cloud Drive over Seafile-compatible APIs*
*Researched: 2026-04-21*
