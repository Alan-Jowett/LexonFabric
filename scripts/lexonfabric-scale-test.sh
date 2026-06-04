#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/lexonfabric-scale-test.sh [--run-name NAME] [--sources-file PATH] RSYNC_URL [RSYNC_URL ...]

Examples:
  scripts/lexonfabric-scale-test.sh rsync.ietf.org::mailman-archive/ipsec/
  scripts/lexonfabric-scale-test.sh --sources-file examples/local/scale-test/rsync.sources.sample.txt

This script:
  1. fetches mailbox content from one or more rsync URLs
  2. discovers .mbox files in the fetched mirrors
  3. generates an indexer request file in the run directory
  4. runs the existing indexer via docker compose
  5. leaves summary/root handoff output in the run directory
EOF
}

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'error: required command not found: %s\n' "$command_name" >&2
    exit 1
  fi
}

sanitize_source_name() {
  local raw="$1"
  local sanitized
  sanitized="$(printf '%s' "$raw" | tr ':/' '__' | tr -cd 'A-Za-z0-9._-')"
  if [[ -z "$sanitized" ]]; then
    sanitized="source"
  fi
  printf '%s' "$sanitized"
}

json_escape() {
  local raw="$1"
  raw="${raw//\\/\\\\}"
  raw="${raw//\"/\\\"}"
  raw="${raw//$'\n'/\\n}"
  raw="${raw//$'\r'/\\r}"
  raw="${raw//$'\t'/\\t}"
  printf '%s' "$raw"
}

wait_for_tcp_port() {
  local host="$1"
  local port="$2"
  local timeout_secs="$3"
  local start_ts
  start_ts="$(date +%s)"

  while true; do
    if bash -c "</dev/tcp/${host}/${port}" >/dev/null 2>&1; then
      return 0
    fi

    if (( "$(date +%s)" - start_ts >= timeout_secs )); then
      printf 'error: timed out waiting for %s:%s to accept TCP connections\n' "$host" "$port" >&2
      return 1
    fi

    sleep 1
  done
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

require_command rsync
require_command docker
require_command date
require_command find
require_command sort
require_command sleep

SOURCES_FILE=""
RUN_NAME=""
declare -a RSYNC_URLS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sources-file)
      [[ $# -ge 2 ]] || { printf 'error: --sources-file requires a path\n' >&2; exit 1; }
      SOURCES_FILE="$2"
      shift 2
      ;;
    --run-name)
      [[ $# -ge 2 ]] || { printf 'error: --run-name requires a value\n' >&2; exit 1; }
      RUN_NAME="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --*)
      printf 'error: unknown option: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
    *)
      RSYNC_URLS+=("$1")
      shift
      ;;
  esac
done

if [[ -n "$SOURCES_FILE" ]]; then
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" || "$line" == \#* ]] && continue
    RSYNC_URLS+=("$line")
  done <"$SOURCES_FILE"
fi

if [[ ${#RSYNC_URLS[@]} -eq 0 ]]; then
  printf 'error: provide at least one rsync URL or a --sources-file\n' >&2
  usage >&2
  exit 1
fi

if [[ -z "$RUN_NAME" ]]; then
  RUN_NAME="$(date -u '+%Y%m%dT%H%M%SZ')"
fi

RUN_ROOT_REL="examples/local/scale-test/runs/${RUN_NAME}"
RUN_ROOT="${REPO_ROOT}/${RUN_ROOT_REL}"
FETCHED_DIR="${RUN_ROOT}/fetched"
REQUEST_PATH="${RUN_ROOT}/request.json"
SUMMARY_PATH="${RUN_ROOT}/summary.json"
BLOCK_STORE_DIR="${RUN_ROOT}/block-store"
SOURCES_LOG="${RUN_ROOT}/sources.txt"
CONTAINER_REQUEST="/workspace/${RUN_ROOT_REL}/request.json"
CONTAINER_SUMMARY="/workspace/${RUN_ROOT_REL}/summary.json"

mkdir -p "$FETCHED_DIR"
mkdir -p "$BLOCK_STORE_DIR"

printf '%s\n' "${RSYNC_URLS[@]}" >"$SOURCES_LOG"

printf 'Creating run directory: %s\n' "$RUN_ROOT_REL"

declare -a MAILBOX_PATHS=()
declare -a DISCOVERED_MONTHS=()

for index in "${!RSYNC_URLS[@]}"; do
  source_url="${RSYNC_URLS[$index]}"
  source_num=$((index + 1))
  source_name="$(sanitize_source_name "$source_url")"
  source_dir="${FETCHED_DIR}/$(printf '%02d' "$source_num")-${source_name}"

  mkdir -p "$source_dir"
  printf 'Fetching [%02d/%02d]: %s\n' "$source_num" "${#RSYNC_URLS[@]}" "$source_url"
  rsync -avz --delete "${source_url%/}/" "${source_dir}/"

  while IFS= read -r mailbox_path; do
    rel_to_run="${mailbox_path#${RUN_ROOT}/}"
    MAILBOX_PATHS+=("$rel_to_run")
    filename="$(basename "$mailbox_path")"
    month="${filename%.mbox}"
    DISCOVERED_MONTHS+=("$month")
  done < <(find "$source_dir" -type f -name '*.mbox' | LC_ALL=C sort)
done

if [[ ${#MAILBOX_PATHS[@]} -eq 0 ]]; then
  printf 'error: no .mbox files were discovered in fetched rsync mirrors\n' >&2
  exit 1
fi

{
  printf '{\n'
  printf '  "environment": {\n'
  printf '    "kind": "local",\n'
  printf '    "block_store_root": "block-store",\n'
  printf '    "embedding": {\n'
  printf '      "base_url": "http://stapi:8080",\n'
  printf '      "model": "all-MiniLM-L6-v2",\n'
  printf '      "request_timeout_secs": 30,\n'
  printf '      "max_retries": 10,\n'
  printf '      "retry_delay_ms": 1000\n'
  printf '    }\n'
  printf '  },\n'
  printf '  "embedding_spec": {\n'
  printf '    "dims": 384,\n'
  printf '    "encoding": "f32le"\n'
  printf '  },\n'
  printf '  "block_size_target": 65536,\n'
  printf '  "items": [\n'

  for index in "${!MAILBOX_PATHS[@]}"; do
    mailbox_path="${MAILBOX_PATHS[$index]}"
    month="${DISCOVERED_MONTHS[$index]}"
    printf '    {\n'
    printf '      "kind": "mailbox",\n'
    printf '      "path": "%s",\n' "$(json_escape "$mailbox_path")"
    printf '      "metadata": {\n'
    printf '        "month": "%s"\n' "$(json_escape "$month")"
    printf '      }\n'
    if [[ "$index" -eq $((${#MAILBOX_PATHS[@]} - 1)) ]]; then
      printf '    }\n'
    else
      printf '    },\n'
    fi
  done

  printf '  ]\n'
  printf '}\n'
} >"$REQUEST_PATH"

printf 'Discovered %d mailbox files\n' "${#MAILBOX_PATHS[@]}"
printf 'Generated request: %s\n' "${REQUEST_PATH#${REPO_ROOT}/}"

(cd "$REPO_ROOT" && docker compose up -d stapi)
wait_for_tcp_port 127.0.0.1 8080 60
(cd "$REPO_ROOT" && docker compose run --rm --no-deps indexer run --request "$CONTAINER_REQUEST" --summary-out "$CONTAINER_SUMMARY")

printf 'Summary written to: %s\n' "${SUMMARY_PATH#${REPO_ROOT}/}"
printf 'Block store written to: %s\n' "${BLOCK_STORE_DIR#${REPO_ROOT}/}"
