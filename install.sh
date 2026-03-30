#!/usr/bin/env bash
set -euo pipefail

repo="__REPOSITORY__"
repo_placeholder="__REPOSITORY_PLACEHOLDER__"
keep_temp=0
dry_run=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      repo="${2:?missing repo}"
      shift 2
      ;;
    --keep-temp)
      keep_temp=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

if [[ "$repo" == "$repo_placeholder" ]]; then
  echo "repo not configured, pass --repo owner/name or use the release asset version" >&2
  exit 1
fi

base_url="https://github.com/$repo/releases/latest/download"
zip_url="$base_url/linmon-linux-x86_64.zip"
hash_url="$base_url/linmon-linux-x86_64.zip.sha256"
tmp_dir="$(mktemp -d -t linmon-install-XXXXXX)"
zip_path="$tmp_dir/linmon-linux-x86_64.zip"
extract_dir="$tmp_dir/payload"

cleanup() {
  if [[ "$keep_temp" -eq 0 ]]; then
    rm -rf "$tmp_dir"
  fi
}
trap cleanup EXIT

if [[ "$dry_run" -eq 1 ]]; then
  echo "zip:  $zip_url"
  echo "hash: $hash_url"
  exit 0
fi

calc_sha256() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    echo "no sha256 tool found" >&2
    exit 1
  fi
}

extract_zip() {
  local src="$1"
  local dst="$2"
  mkdir -p "$dst"
  if command -v unzip >/dev/null 2>&1; then
    unzip -q "$src" -d "$dst"
    return
  fi

  if command -v python3 >/dev/null 2>&1; then
    python3 - "$src" "$dst" <<'PY'
import sys
from zipfile import ZipFile

with ZipFile(sys.argv[1]) as zf:
    zf.extractall(sys.argv[2])
PY
    return
  fi

  echo "need unzip or python3 to extract package" >&2
  exit 1
}

curl -fsSL "$zip_url" -o "$zip_path"
expected_hash="$(curl -fsSL "$hash_url" | awk '{print $1}')"
actual_hash="$(calc_sha256 "$zip_path")"
if [[ "$expected_hash" != "$actual_hash" ]]; then
  echo "sha256 mismatch: expected $expected_hash got $actual_hash" >&2
  exit 1
fi

extract_zip "$zip_path" "$extract_dir"

if [[ ! -f "$extract_dir/linmon" ]]; then
  echo "linmon not found in package" >&2
  exit 1
fi

chmod +x "$extract_dir/linmon"
"$extract_dir/linmon" bootstrap

echo "installed: $HOME/.local/bin/linmon"
echo "open a new shell and run: linmon"
