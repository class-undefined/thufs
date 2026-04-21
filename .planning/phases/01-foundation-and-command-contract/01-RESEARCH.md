# Phase 1: Foundation and Command Contract - Research

**Researched:** 2026-04-21
**Domain:** CLI foundation for a Seafile-backed cloud drive client
**Confidence:** MEDIUM

<user_constraints>
## User Constraints (from CONTEXT.md)

**CRITICAL:** If CONTEXT.md exists from $gsd-discuss-phase, copy locked decisions here verbatim. These MUST be honored by the planner.

### Locked Decisions
- **D-01:** v1 authentication is token-driven rather than username/password login or browser-assisted login.
- **D-02:** The CLI should assume the user has already obtained a usable THU Cloud Drive token from outside `thufs`, then provide a CLI path to store and use it.
- **D-03:** Configuration is file-first for normal usage, with environment variables acting as an override layer.
- **D-04:** The configuration model should be designed for both daily terminal usage and script execution, without making environment variables the primary UX.
- **D-05:** Remote paths must always support an explicit fully-qualified form.
- **D-06:** A default repo may be configured as a convenience shortcut, but command semantics must not depend on that shortcut being present.
- **D-07:** Planner and implementer should treat the explicit form as canonical and the default-repo form as syntactic sugar.
- **D-08:** Default command output is human-readable.
- **D-09:** Commands should provide `--json` for machine-readable output.
- **D-10:** Normal results go to stdout, errors go to stderr, and exit codes must remain stable for automation.
- **D-11:** The command contract should leave room for `--quiet` and `--verbose` style controls without undermining the default human-readable behavior.
- **D-12:** High-frequency business commands stay short and flat: `push`, `pull`, `ls`, `share`.
- **D-13:** Management-oriented commands should be grouped, such as `auth ...` and `config ...`.
- **D-14:** The command tree should feel shell-first and compact for daily use, without letting administrative verbs sprawl into unrelated top-level commands.

### the agent's Discretion
- Exact config file format and filename
- Exact token-setting command spelling under `auth`
- Exact flag names for verbosity and JSON output
- Exact syntax of the fully-qualified remote path, as long as it preserves the explicit-vs-default-repo contract above

### Deferred Ideas (OUT OF SCOPE)
- None

</user_constraints>

<architectural_responsibility_map>
## Architectural Responsibility Map

Single-tier application with an external HTTP dependency:

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Command parsing and help output | Browser/Client | — | The local CLI binary owns user input parsing and help rendering |
| Token/config persistence | Browser/Client | Database/Storage | Config is stored locally on disk and read by the CLI runtime |
| Remote path normalization and output semantics | Browser/Client | — | These are user-facing contracts owned by the local command layer |
| Seafile-compatible transport | API/Backend | Browser/Client | Remote API behavior is server-defined, but the local client must encapsulate it cleanly |

</architectural_responsibility_map>

<research_summary>
## Summary

Phase 1 is less about feature delivery and more about freezing the behavioral contract that all later commands will inherit. The standard approach for a CLI of this type is a thin command layer, a dedicated config package that owns file/env precedence, and a separate service/client boundary so future transfer and share operations do not push implementation details into command handlers.

The most important architectural insight is that path semantics, token handling, and stdout/stderr discipline should be treated as first-class product behavior, not incidental details to improvise during command implementation. Later phases will depend on these contracts being stable; if they drift now, later transfer and share commands will accumulate compatibility debt.

**Primary recommendation:** Establish the CLI package boundaries, config precedence rules, and explicit remote-path contract in Phase 1, with tests that lock those behaviors before any Seafile-heavy command implementation begins.

> Execution note: implementation was ultimately carried out in Rust after user direction. The behavioral recommendations in this document still apply.
</research_summary>

<standard_stack>
## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go | 1.25.x | Primary implementation language | Strong fit for single-binary CLI tooling and native filesystem/HTTP handling |
| `github.com/spf13/cobra` | v1.x | Command tree, help output, completions | Widely used and well-suited to hybrid flat/grouped CLI surfaces |
| Seafile Web API patterns | v2.1 / server-11.x style | Backend contract reference | Needed so Phase 1 abstractions match later command needs |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/spf13/viper` | v1.x | Config loading and environment overrides | Use if the project wants standard file/env precedence without hand-rolling parsing |
| `github.com/stretchr/testify` | v1.x | Assertions in tests | Use for table-driven behavior checks in config and path logic |
| `golang.org/x/sync/errgroup` | current | Coordinated concurrent work | Not required immediately, but useful once batched operations arrive |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Cobra | `urfave/cli` | `urfave/cli` is lighter, but Cobra gives clearer grouped help and established patterns |
| Viper | stdlib + custom parser | Custom config reduces dependencies, but Phase 1 then has to own precedence edge cases directly |

**Installation:**
```bash
go mod init github.com/zhitai/thufs
go get github.com/spf13/cobra@latest
go get github.com/spf13/viper@latest
go get github.com/stretchr/testify@latest
```
</standard_stack>

<architecture_patterns>
## Architecture Patterns

### System Architecture Diagram

```text
User command
  ↓
Cobra command layer
  ↓
App/service layer
  ├── config service
  ├── path contract service
  └── output/rendering policy
  ↓
Seafile client boundary
  ↓
THU Cloud Drive / Seafile server
```

### Recommended Project Structure
```text
cmd/
└── thufs/              # binary bootstrap

internal/
├── cli/                # cobra commands and shared flags
├── config/             # config file discovery, env overrides, token handling
├── contract/           # path and output behavior contracts
├── app/                # use-case services
└── seafile/            # remote API client boundary
```

### Pattern 1: Thin command handlers
**What:** `RunE` functions should parse arguments and delegate to services rather than embed business rules.
**When to use:** For every command, including `auth` and `config`.
**Example:**
```go
func newConfigShowCmd(app *app.App) *cobra.Command {
	return &cobra.Command{
		Use:   "show",
		Short: "Show active configuration",
		RunE: func(cmd *cobra.Command, args []string) error {
			result, err := app.ConfigService.Show(cmd.Context())
			if err != nil {
				return err
			}
			return app.Output.RenderConfig(cmd.OutOrStdout(), result)
		},
	}
}
```

### Pattern 2: Config source precedence as explicit code
**What:** File/env precedence is encoded in one place and tested directly.
**When to use:** Whenever loading token, default repo, or output preferences.
**Example:**
```go
type Sources struct {
	File Config
	Env  Config
}

func (s Sources) Resolve() Config {
	cfg := s.File
	if s.Env.Token != "" {
		cfg.Token = s.Env.Token
	}
	if s.Env.DefaultRepo != "" {
		cfg.DefaultRepo = s.Env.DefaultRepo
	}
	return cfg
}
```

### Anti-Patterns to Avoid
- **Hand-rolled ad hoc command output:** later commands cannot stay consistent if each command prints directly.
- **Embedding remote-path semantics in individual commands:** path behavior must live in one shared contract layer.
- **Treating secrets like ordinary config fields:** token printing and file-permission handling need explicit rules.
</architecture_patterns>

<dont_hand_roll>
## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Command routing and help tree | Custom flag parser | Cobra | Hybrid command surfaces and help output are already solved well |
| Config precedence | Ad hoc env/file merging in each command | Central config resolver | Prevents silent drift between commands |
| Structured rendering | Per-command `fmt.Println` logic | Shared output/render package | Preserves stdout/stderr discipline and future `--json` consistency |

**Key insight:** Phase 1 should centralize contracts, not scatter them. The cost of extra structure now is lower than retrofitting consistency across later transfer commands.
</dont_hand_roll>

<common_pitfalls>
## Common Pitfalls

### Pitfall 1: Config precedence drifts between commands
**What goes wrong:** `auth`, `config`, and future business commands resolve token/default repo differently.
**Why it happens:** Each command owns its own loading logic.
**How to avoid:** Create one resolver and test it before wiring multiple commands.
**Warning signs:** Commands disagree on active token or default repo.

### Pitfall 2: Explicit and default-repo path forms diverge
**What goes wrong:** Commands treat fully-qualified remote paths differently from shorthand forms.
**Why it happens:** Convenience syntax is added without a canonical internal representation.
**How to avoid:** Normalize both forms into one internal remote reference type.
**Warning signs:** `ls` and `pull` need separate path parsing logic.

### Pitfall 3: Output policy is underspecified
**What goes wrong:** Human-readable output blocks later automation or JSON support becomes inconsistent.
**Why it happens:** Early commands print “whatever looks fine” instead of following a renderer contract.
**How to avoid:** Separate normal result rendering, error rendering, and JSON serialization from the start.
**Warning signs:** Tests assert full printed strings from business logic instead of structured results.
</common_pitfalls>

<code_examples>
## Code Examples

Verified patterns from official sources:

### Cobra root command with grouped subcommands
```go
// Source: Cobra documentation pattern
root := &cobra.Command{Use: "thufs"}
root.AddCommand(newPushCmd(app), newPullCmd(app), newLsCmd(app), newShareCmd(app))
root.AddCommand(newAuthCmd(app), newConfigCmd(app))
```

### Table-driven precedence test
```go
func TestResolvePrefersEnv(t *testing.T) {
	s := Sources{
		File: Config{Token: "file-token"},
		Env:  Config{Token: "env-token"},
	}
	require.Equal(t, "env-token", s.Resolve().Token)
}
```

### Explicit remote reference normalization
```go
type RemoteRef struct {
	Repo string
	Path string
}

func NormalizeRemote(input string, defaultRepo string) (RemoteRef, error) {
	// Canonical parsing logic lives here, not in command handlers.
	return RemoteRef{}, nil
}
```
</code_examples>

<sota_updates>
## State of the Art (2024-2025)

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fat CLI handlers | Thin command + service layering | Long-running CLI best practice | Easier testing and extensibility |
| Config hardcoded in `$HOME` logic only | Config plus env override layering | Current CLI expectation | Better CI/script support |
| Human-only CLI output | Human default + structured mode | Modern automation-first tooling | Stable machine consumption without degrading shell UX |

**New tools/patterns to consider:**
- Shared renderer interfaces to keep JSON and human output aligned
- Dedicated remote reference types instead of stringly typed path handling

**Deprecated/outdated:**
- Printing secrets in diagnostics
- Per-command config loading without a shared precedence contract
</sota_updates>

<open_questions>
## Open Questions

1. **Exact config file format**
   - What we know: file-first config is locked
   - What's unclear: whether TOML, YAML, or JSON is the best ergonomic default
   - Recommendation: decide during planning based on Go ecosystem fit and simplicity

2. **Exact explicit remote-path syntax**
   - What we know: fully-qualified remote syntax is required and canonical
   - What's unclear: final delimiter and repo/path encoding shape
   - Recommendation: planner should choose one syntax and add tests that freeze it
</open_questions>

<sources>
## Sources

### Primary (HIGH confidence)
- `.planning/phases/01-foundation-and-command-contract/01-CONTEXT.md` - locked user decisions
- `.planning/research/STACK.md` - project-level CLI stack direction
- `.planning/research/ARCHITECTURE.md` - project-level architecture guidance
- `.planning/research/PITFALLS.md` - project-level pitfalls and safety constraints

### Secondary (MEDIUM confidence)
- `https://help.seafile.com/syncing_client/linux-cli/` - CLI UX reference point
- `https://manual.seafile.com/latest/develop/web_api_v2.1/` - remote API contract reference

### Tertiary (LOW confidence - needs validation)
- Inferred Go CLI implementation patterns from standard ecosystem usage
</sources>

<metadata>
## Metadata

**Research scope:**
- Core technology: Go CLI foundation
- Ecosystem: Cobra, config precedence, renderer separation
- Patterns: command layering, canonical path normalization, contract testing
- Pitfalls: output drift, auth drift, path ambiguity

**Confidence breakdown:**
- Standard stack: MEDIUM - library direction is strong, exact versions can move
- Architecture: MEDIUM - structure is clear, fine-grained package names remain flexible
- Pitfalls: HIGH - directly grounded in this project's stated risks
- Code examples: MEDIUM - representative patterns, not final implementation

**Research date:** 2026-04-21
**Valid until:** 2026-05-21
</metadata>

---
*Phase: 01-foundation-and-command-contract*
*Research completed: 2026-04-21*
*Ready for planning: yes*
