#!/usr/bin/env bash
# set -euo pipefail

if [[ "${THUFS_E2E:-}" != "1" ]]; then
  echo "Refusing to run: set THUFS_E2E=1 to confirm real THU Cloud Drive operations." >&2
  exit 2
fi

if [[ -z "${THUFS_TOKEN:-}" ]]; then
  echo "THUFS_TOKEN is required. Obtain it from https://cloud.tsinghua.edu.cn/profile/#get-auth-token" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${THUFS_BIN:-$ROOT_DIR/target/debug/thufs}"
RUN_ID="${THUFS_E2E_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
REPO="${THUFS_E2E_REPO:-thufs-e2e-$RUN_ID}"
REMOTE_DIR="/flows/$RUN_ID"
REMOTE_FILE="$REMOTE_DIR/payload.txt"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/thufs-e2e.XXXXXX")"
CONFIG_DIR="$WORK_DIR/config"
UPLOAD_FILE="$WORK_DIR/payload.txt"
DOWNLOAD_FILE="$WORK_DIR/downloaded.txt"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

run() {
  echo "+ $*" >&2
  "$@"
}

if [[ ! -x "$BIN" ]]; then
  echo "Building thufs debug binary..."
  run cargo build --manifest-path "$ROOT_DIR/Cargo.toml"
fi

export THUFS_CONFIG_DIR="$CONFIG_DIR"
export THUFS_DEFAULT_REPO="$REPO"

printf 'thufs e2e payload\nrun_id=%s\n' "$RUN_ID" > "$UPLOAD_FILE"

echo "== thufs local E2E =="
echo "repo: $REPO"
echo "remote: repo:$REPO$REMOTE_FILE"
echo "work dir: $WORK_DIR"

run "$BIN" auth set-token "$THUFS_TOKEN"
run "$BIN" info
run "$BIN" repos
run "$BIN" mkrepo "$REPO"
run "$BIN" mkdir "repo:$REPO$REMOTE_DIR"
run "$BIN" upload --conflict overwrite "$UPLOAD_FILE" "repo:$REPO$REMOTE_FILE"
run "$BIN" ls "repo:$REPO$REMOTE_DIR"
run "$BIN" ls --time "repo:$REPO$REMOTE_DIR"

SHARE_JSON="$(run "$BIN" --json share "repo:$REPO$REMOTE_FILE")"
printf '%s\n' "$SHARE_JSON"
SHARE_TOKEN="$(printf '%s\n' "$SHARE_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("token") or "")')"
if [[ -z "$SHARE_TOKEN" ]]; then
  echo "share response did not include token; cannot test unshare" >&2
  exit 1
fi

run "$BIN" shares "repo:$REPO$REMOTE_FILE"
run "$BIN" unshare "$SHARE_TOKEN"
run "$BIN" download --mode sequential --conflict overwrite "repo:$REPO$REMOTE_FILE" "$DOWNLOAD_FILE"

if ! cmp -s "$UPLOAD_FILE" "$DOWNLOAD_FILE"; then
  echo "downloaded file differs from uploaded file" >&2
  exit 1
fi

echo "E2E completed successfully."
