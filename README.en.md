# thufs

[![GitHub stars](https://img.shields.io/github/stars/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/stargazers)
[![GitHub license](https://img.shields.io/github/license/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/blob/master/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/class-undefined/thufs/ci.yml?branch=master&style=flat-square&label=CI)](https://github.com/class-undefined/thufs/actions/workflows/ci.yml)
[![Publish crate](https://img.shields.io/github/actions/workflow/status/class-undefined/thufs/publish-crate.yml?branch=master&style=flat-square&label=crate)](https://github.com/class-undefined/thufs/actions/workflows/publish-crate.yml)
[![Crates.io](https://img.shields.io/crates/v/thufs?style=flat-square)](https://crates.io/crates/thufs)
[![GitHub last commit](https://img.shields.io/github/last-commit/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/commits/master)
[![GitHub issues](https://img.shields.io/github/issues/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/issues)
[![Rust](https://img.shields.io/badge/Rust-2024%20Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Backend: Seafile](https://img.shields.io/badge/Backend-Seafile-ff6b6b?style=flat-square)](https://www.seafile.com/)
[![中文 README](https://img.shields.io/badge/README-%E4%B8%AD%E6%96%87-blue?style=flat-square)](./README.md)

`thufs` is a shell-first CLI for THU Cloud Drive users. It is built on top of Seafile APIs and focuses on the core file operations that terminal users actually need: upload, download, list, inspect account info, and create share links.

Instead of being a heavy sync client, `thufs` is designed for SSH sessions, remote servers, cluster jobs, and automation scripts where predictable command behavior matters more than desktop-style synchronization.

## Highlights

- `info` to inspect the current token's account info
- `repos` or `libraries` to list visible repositories
- `ls` to list remote directories, including repo roots
- `upload` to send a local file to THU Cloud Drive
- `download` to fetch a remote file locally
- `share` to create share links with optional password and expiry
- `mkrepo` or `mklib` to create a library explicitly
- `mkdir` to create remote directories recursively
- `push` and `pull` kept as compatibility aliases
- progress bars on TTY
- `--progress jsonl` for machine-readable streaming progress events on stderr
- best-effort resumable upload and download
- unified `--conflict` strategy
- default conflict behavior is `uniquify`
- JSON output includes final resolved paths for scripting
- `upload` auto-creates missing libraries and parent directories
- `ls` includes update timestamps

## Installation

Install it with Cargo:

```bash
cargo install thufs --locked
thufs --help
```

The minimum supported Rust version is `1.85`. If plain `cargo install thufs` resolves dependencies that require a newer Rust version, use `--locked`.

You can also build from source:

```bash
git clone git@github.com:class-undefined/thufs.git
cd thufs
cargo build --release
./target/release/thufs --help
```

On macOS, you can also download the prebuilt binary archive for your CPU architecture from GitHub Releases.

If Rust downloads are slow in your network environment:

```bash
export RUSTUP_DIST_SERVER="https://rsproxy.cn"
export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
```

For local development:

```bash
cargo run -- --help
```

## Authentication And Config

`thufs` is token-driven. It does not implement username/password login or browser-based auth flows.

Recommended way to obtain a token:

1. Sign in with your Tsinghua account at:

```text
https://cloud.tsinghua.edu.cn/profile/#get-auth-token
```

2. In the `Web API Auth Token` section, click "Generate"
3. Copy the generated token information and use it with `thufs auth set-token`

Set a token:

```bash
thufs auth set-token <seafile-api-token>
```

Inspect the active configuration:

```bash
thufs config show
thufs --json config show
```

Default config path:

```text
~/.config/thufs/config.json
```

Supported environment overrides:

- `THUFS_TOKEN`
- `THUFS_DEFAULT_REPO`
- `THUFS_OUTPUT`
- `THUFS_CONFIG_DIR`

The model is file-first configuration with environment overrides for temporary sessions and scripts.

## Remote Path Syntax

The canonical explicit remote form is:

```text
repo:<library>/<path>
```

Here `library` means the Seafile repository or library name visible to the current account.

Example:

```text
repo:course-lib/slides/week1.pdf
```

This means:

```text
/slides/week1.pdf inside the library named course-lib
```

Valid examples:

```bash
thufs ls repo:course-lib/slides
thufs download repo:course-lib/slides/week1.pdf
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload ./report.pdf repo:course-lib
```

If `THUFS_DEFAULT_REPO` or config `default_repo` is set, shorthand paths are allowed:

```bash
thufs ls slides
thufs download slides/week1.pdf
thufs upload ./report.pdf submissions/
```

`repo:<library>` is also accepted for uploads and means "upload into the repo root using the local file name".

## Quick Start

Account info:

```bash
thufs info
thufs --json info
```

Visible repositories:

```bash
thufs repos
thufs libraries
thufs --json repos
```

List a directory:

```bash
thufs ls repo:course-lib/slides
thufs ls course-lib
thufs --json ls repo:course-lib/slides
```

File sizes in `ls` are displayed adaptively as `B`, `KB`, `MB`, `GB`, and so on.
If you want update timestamps, pass `--time` or `-t`.

```bash
thufs ls -t repo:course-lib/slides
thufs ls --time course-lib
```

Upload a file:

```bash
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload ./report.pdf repo:course-lib
thufs upload ./report.pdf submissions/
```

If the target library does not exist, `upload` creates it automatically.
If parent directories are missing, `upload` creates them before sending the file.

Create a library or directory explicitly:

```bash
thufs mkrepo course-lib
thufs mklib course-lib
thufs mkdir repo:course-lib/slides/week1
thufs mkdir submissions/week1
```

Download a file:

```bash
thufs download repo:course-lib/slides/week1.pdf
thufs download repo:course-lib/slides/week1.pdf ./week1.pdf
thufs download repo:course-lib/slides/week1.pdf ./downloads/
thufs download https://cloud.tsinghua.edu.cn/f/abc123XYZ_/
thufs download "https://cloud.tsinghua.edu.cn/f/abc123XYZ_/?dl=1"
thufs download --share abc123XYZ_
thufs download --mode sequential repo:course-lib/slides/week1.pdf
thufs download --mode parallel --workers 8 repo:course-lib/slides/week1.pdf
```

If the local path is omitted, `download` saves into the current directory using the remote file name.
Public shared files are also supported: pass a full share URL, and query strings like `?dl=1` are ignored so only the token is used, or use `--share <hashcode>`.

Create a share link:

```bash
thufs share repo:course-lib/slides/week1.pdf
thufs share --password secret --expire-days 7 repo:course-lib/slides/week1.pdf
thufs --json share repo:course-lib/slides/week1.pdf
thufs shares repo:course-lib/slides/week1.pdf
thufs shares --page 1 --per-page 50
thufs shares --all
thufs unshare <share-token>
```

## Conflict Strategy

`upload` and `download` support:

```bash
--conflict <policy>
```

Available policies:

- `uniquify` creates a new sibling name such as `report-(1).pdf`
- `overwrite` replaces the existing target
- `fail` exits immediately on conflict
- `prompt` enables an explicit interactive choice

Default policy:

```text
uniquify
```

Examples:

```bash
thufs upload --conflict overwrite ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload --conflict uniquify ./report.pdf repo:course-lib/submissions/report.pdf
thufs download --conflict fail repo:course-lib/slides/week1.pdf ./week1.pdf
```

## JSON Output For Scripts

Human-readable output is the default. Add `--json` for structured output.

Transfer results include explicit path fields such as:

- `requested_remote_path`
- `final_remote_path`
- `requested_local_path`
- `final_local_path`
- `remote_name`
- `local_name`
- `overwritten`
- `uniquified`

This makes it easy for scripts to detect whether a conflict was resolved by renaming and where the final file ended up.

Example:

```bash
FINAL_PATH="$(thufs --json download repo:course-lib/slides/week1.pdf | jq -r '.final_local_path')"
echo "saved to: $FINAL_PATH"
```

## Transfer Behavior

- progress bars are shown when stderr is a TTY
- `--progress jsonl` streams JSON Lines progress events to stderr for GUIs, task queues, and scripts that need precise transfer percentages
- `--progress none` disables transfer progress output
- `download` uses `.thufs-part` temporary files and resumes when the server supports range requests
- `download` prefers parallel ranged download by default and automatically falls back to sequential mode when the endpoint or current resume state does not allow it
- `upload` performs best-effort resume using Seafile's uploaded-bytes mechanism

Machine-readable progress examples:

```bash
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf --progress jsonl
thufs download repo:course-lib/slides/week1.pdf ./week1.pdf --progress jsonl
```

Each line is a standalone JSON event with:

- `event`
- `operation`
- `path`
- `transferred_bytes`
- `total_bytes`
- `percent`

Download mode can also be controlled explicitly:

- `--mode auto` default behavior, prefer parallel and fall back when needed
- `--mode parallel` require parallel ranged download and fail if unsupported
- `--mode sequential` always use single-stream download
- `--workers N` set the number of parallel workers; only used by parallel download

## Command Overview

| Command | Purpose |
| --- | --- |
| `thufs info` | Show account info for the current token |
| `thufs repos` | List visible repositories |
| `thufs ls <remote>` | List a remote directory |
| `thufs mkrepo <name>` | Create a library |
| `thufs mkdir <remote>` | Create a remote directory |
| `thufs upload <local> <remote>` | Upload a local file |
| `thufs download <remote> [local]` | Download a remote file |
| `thufs share <remote>` | Create a share link |
| `thufs shares [remote]` | List share links with pagination; no remote means all links |
| `thufs unshare <token>` | Delete a share link |
| `thufs auth set-token <token>` | Store a token |
| `thufs config show` | Show the active configuration |

## Scope

`thufs` intentionally does not aim to provide:

- full bidirectional sync
- recursive directory transfer
- multi-account profile management
- generic support for arbitrary Seafile instances
- GUI or desktop resident sync behavior

It is a focused terminal tool, not a general-purpose sync platform.

## Development

Run tests:

```bash
cargo test
```

Format code:

```bash
cargo fmt
```

Inspect command help:

```bash
cargo run -- --help
cargo run -- upload --help
cargo run -- download --help
```

## License

This project is licensed under the [MIT License](./LICENSE).
