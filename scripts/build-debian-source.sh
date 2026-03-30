#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pkg_name="linmon"
version="$(dpkg-parsechangelog -SVersion -l"$root_dir/debian/changelog")"
upstream_version="${version%-*}"
work_dir="${1:-$(mktemp -d -t linmon-srcpkg-XXXXXX)}"
stage_dir="$work_dir/$pkg_name"
orig_dir="$work_dir/orig"
orig_tar="$work_dir/${pkg_name}_${upstream_version}.orig.tar.gz"

rm -rf "$stage_dir" "$orig_dir"
mkdir -p "$stage_dir" "$orig_dir"

tar \
  --exclude=.git \
  --exclude=target \
  --exclude=dist \
  --exclude=.github \
  --exclude='.DS_Store' \
  -C "$root_dir" \
  -cf - . | tar -C "$stage_dir" -xf -

mkdir -p "$orig_dir/${pkg_name}-${upstream_version}"
tar \
  --exclude=.git \
  --exclude=target \
  --exclude=dist \
  --exclude=debian \
  --exclude=.github \
  --exclude='.DS_Store' \
  -C "$root_dir" \
  -cf - . | tar -C "$orig_dir/${pkg_name}-${upstream_version}" -xf -

tar -C "$orig_dir" -czf "$orig_tar" "${pkg_name}-${upstream_version}"

(
  cd "$stage_dir"
  if [[ "$orig_tar" != "$(realpath ../$(basename "$orig_tar"))" ]]; then
    cp "$orig_tar" ../
  fi
  dpkg-buildpackage -S -sa -us -uc
)

echo "$work_dir"
