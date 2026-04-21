# thufs

`thufs` is a shell-first CLI for THU Cloud Drive users who want predictable terminal workflows instead of sync-heavy desktop behavior.

## Phase 1 Contract

- Authentication is token-driven. `thufs` stores a token obtained outside the CLI and does not implement username/password login.
- Configuration is file-first. Environment variables override the resolved config, but local config remains the default operator path.
- Management commands are grouped under `thufs auth` and `thufs config`. Business verbs such as `push`, `pull`, `ls`, and `share` stay flat.
- Human-readable output is the default. Add `--json` for machine-readable output.
- Normal command results go to stdout. Errors go to stderr.

## Commands

```text
thufs auth set-token <token>
thufs config show
thufs --json config show
```

## Config

The default config file is `~/.config/thufs/config.json` on Unix-like systems.

Supported environment overrides:

- `THUFS_TOKEN`
- `THUFS_DEFAULT_REPO`
- `THUFS_OUTPUT`
- `THUFS_CONFIG_DIR`

## Remote Path

The canonical explicit form is:

```text
repo:<library>/<path>
```

Examples:

```text
repo:course-lib/slides/week1.pdf
repo:backup/code/main.rs
```

If a `default repo` is configured, shorthand paths such as `notes/todo.md` resolve against that repo. Without a default repo, explicit `repo:<library>/<path>` form is required.
