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

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required" >&2
  exit 1
fi

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${version#v}"
pkg_root="$(mktemp -d -t linmon-deb-XXXXXX)"
stage_dir="$pkg_root/linmon_${version}_amd64"
trap 'rm -rf "$pkg_root"' EXIT

mkdir -p "$root_dir/$out_dir"
mkdir -p \
  "$stage_dir/DEBIAN" \
  "$stage_dir/usr/bin" \
  "$stage_dir/usr/share/doc/linmon"

install -m755 "$bin_path" "$stage_dir/usr/bin/linmon"
install -m644 "$root_dir/README.md" "$stage_dir/usr/share/doc/linmon/README.md"
install -m644 "$root_dir/LICENSE" "$stage_dir/usr/share/doc/linmon/copyright"

cat > "$stage_dir/usr/share/doc/linmon/changelog.Debian" <<EOF
linmon (${version}) unstable; urgency=medium

  * Release ${version}.

 -- ailuntz <130897222+ailuntz@users.noreply.github.com>  $(date -R)
EOF
gzip -n -9 "$stage_dir/usr/share/doc/linmon/changelog.Debian"

installed_size="$(du -sk "$stage_dir" | awk '{print $1}')"
cat > "$stage_dir/DEBIAN/control" <<EOF
Package: linmon
Version: ${version}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: ailuntz <130897222+ailuntz@users.noreply.github.com>
Installed-Size: ${installed_size}
Homepage: https://github.com/ailuntx/linmon
Description: Linux CLI monitor for CPU and GPU
 Terminal monitor for CPU, memory, power and NVIDIA GPU metrics on Linux and WSL2.
EOF

deb_path="$root_dir/$out_dir/linmon_${version}_amd64.deb"
dpkg-deb --build "$stage_dir" "$deb_path" >/dev/null
echo "$deb_path"
