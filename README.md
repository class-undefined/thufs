# thufs

`thufs` is a shell-first CLI for THU Cloud Drive users who want predictable terminal workflows instead of sync-heavy desktop behavior. It is intentionally THU-only, single-account, and focused on flat business verbs such as `info`, `repos`, `ls`, `upload`, `download`, and `share`.

## Install

Build from source:

```bash
cargo build --release
./target/release/thufs --help
```

During Rust installation in restricted networks, the project can use the mirror variables:

```bash
export RUSTUP_DIST_SERVER="https://rsproxy.cn"
export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
```

## Auth And Config

Authentication is token-driven. `thufs` stores a token obtained outside the CLI and does not implement username/password login.

```bash
thufs auth set-token <seafile-api-token>
thufs config show
thufs --json config show
```

The default config file is `~/.config/thufs/config.json` on Unix-like systems.

Supported environment overrides:

- `THUFS_TOKEN`
- `THUFS_DEFAULT_REPO`
- `THUFS_OUTPUT`
- `THUFS_CONFIG_DIR`

Configuration is file-first; environment variables override the resolved config for scripts and temporary sessions.

## Remote Paths

The canonical explicit remote form is:

```text
repo:<library>/<path>
```

Examples:

```bash
thufs ls repo:course-lib/slides
thufs push report.pdf repo:course-lib/submissions/report.pdf
thufs pull repo:course-lib/slides/week1.pdf ./week1.pdf
thufs share repo:course-lib/slides/week1.pdf
```

If `THUFS_DEFAULT_REPO` or config `default_repo` is set, shorthand paths such as `slides/week1.pdf` resolve against that library. Without a default repo, use explicit `repo:<library>/<path>`.

## Commands

List a remote directory:

```bash
thufs ls repo:course-lib/slides
thufs --json ls repo:course-lib/slides
```

Show the current token's account information:

```bash
thufs info
thufs --json info
```

List visible repositories or libraries:

```bash
thufs repos
thufs libraries
thufs --json repos
```

Upload a single local file:

```bash
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload ./report.pdf repo:course-lib
thufs upload ./report.pdf submissions/
thufs upload --overwrite ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload --rename ./report.pdf repo:course-lib/submissions/report.pdf
```

Download a single remote file:

```bash
thufs download repo:course-lib/slides/week1.pdf
thufs download repo:course-lib/slides/week1.pdf ./week1.pdf
thufs download --overwrite repo:course-lib/slides/week1.pdf ./week1.pdf
thufs download --rename repo:course-lib/slides/week1.pdf ./week1.pdf
```

If `download` omits the local path, `thufs` saves into the current directory using the remote file name.

If `upload` points at a repo root or a remote directory ending with `/`, `thufs` uses the local file name automatically.

`upload` and `download` support these conflict controls:

- `--overwrite` replace the existing target
- `--rename` choose a unique sibling name
- `--fail` fail immediately instead of prompting

In an interactive terminal, if none of these flags are provided and the target already exists, `thufs` prompts for overwrite or rename. In non-interactive mode, `upload` defaults to automatic rename and `download` requires an explicit policy flag.

## Transfer UX

- `upload` and `download` show a progress bar when stderr is a TTY.
- `push` and `pull` remain supported as compatibility aliases for `upload` and `download`.
- `download` resumes from an existing partial `.thufs-part` file using HTTP range requests when the server supports it.
- `upload` performs a best-effort resumable upload against Seafile's documented uploaded-bytes mechanism.

Create a share link:

```bash
thufs share repo:course-lib/slides/week1.pdf
thufs share --password secret --expire-days 7 repo:course-lib/slides/week1.pdf
thufs --json share repo:course-lib/slides/week1.pdf
```

Human-readable output is the default. Add `--json` for machine-readable output. Normal command results go to stdout; errors go to stderr.

## Scope

`thufs` does not implement full sync, recursive directory transfer, multi-account profiles, or generic Seafile instance targeting in v1.

## Known Limitations

The Seafile HTTP choreography is isolated in `src/seafile.rs` and follows upstream Seafile API shapes, including `POST /api/v2.1/share-links/`. Before an initial public release, these endpoint assumptions should be validated against a real THU Cloud Drive account and token.
