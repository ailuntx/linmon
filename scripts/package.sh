#!/usr/bin/env bash
set -euo pipefail

version="${1:-}"
bin_path="${2:-target/x86_64-unknown-linux-musl/release/linmon}"
out_dir="${3:-dist}"

if [[ -z "$version" ]]; then
  echo "usage: $0 <version> [bin_path] [out_dir]" >&2
  exit 1
fi

if [[ ! -f "$bin_path" ]]; then
  echo "binary not found: $bin_path" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 1
fi

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage_dir="$(mktemp -d -t linmon-package-XXXXXX)"
trap 'rm -rf "$stage_dir"' EXIT

mkdir -p "$root_dir/$out_dir"
cp "$bin_path" "$stage_dir/linmon"
chmod +x "$stage_dir/linmon"
cp "$root_dir/README.md" "$stage_dir/README.md"
cp "$root_dir/LICENSE" "$stage_dir/LICENSE"

zip_path="$root_dir/$out_dir/linmon-${version}-linux-x86_64.zip"
python3 - "$zip_path" "$stage_dir" <<'PY'
import pathlib
import sys
import zipfile

zip_path = pathlib.Path(sys.argv[1])
stage_dir = pathlib.Path(sys.argv[2])

with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
    for path in sorted(stage_dir.rglob("*")):
        if path.is_file():
            zf.write(path, path.relative_to(stage_dir))
PY

echo "$zip_path"
