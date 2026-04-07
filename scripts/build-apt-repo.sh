#!/usr/bin/env bash
set -euo pipefail

deb_path="${1:-}"
out_dir="${2:-dist/pages}"

if [[ -z "$deb_path" ]]; then
  echo "usage: $0 <deb_path> [out_dir]" >&2
  exit 1
fi

if [[ ! -f "$deb_path" ]]; then
  echo "deb not found: $deb_path" >&2
  exit 1
fi

if ! command -v dpkg-scanpackages >/dev/null 2>&1; then
  echo "dpkg-scanpackages is required" >&2
  exit 1
fi

if ! command -v apt-ftparchive >/dev/null 2>&1; then
  echo "apt-ftparchive is required" >&2
  exit 1
fi

repo_dir="$out_dir/apt"
mkdir -p "$repo_dir"
cp "$deb_path" "$repo_dir/"

(
  cd "$repo_dir"
  dpkg-scanpackages . /dev/null > Packages
  gzip -n -9c Packages > Packages.gz
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin="linmon" \
    -o APT::FTPArchive::Release::Label="linmon" \
    -o APT::FTPArchive::Release::Suite="stable" \
    -o APT::FTPArchive::Release::Codename="stable" \
    -o APT::FTPArchive::Release::Architectures="amd64" \
    -o APT::FTPArchive::Release::Components="main" \
    release . > Release
)

touch "$out_dir/.nojekyll"
cat > "$out_dir/index.html" <<'EOF'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>linmon apt repo</title>
</head>
<body>
<pre>deb [trusted=yes] https://ailuntx.github.io/linmon/apt ./</pre>
</body>
</html>
EOF

cat > "$repo_dir/index.html" <<'EOF'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>linmon apt repo</title>
</head>
<body>
<pre>deb [trusted=yes] https://ailuntx.github.io/linmon/apt ./</pre>
<pre>apt update && apt install linmon</pre>
</body>
</html>
EOF

echo "$repo_dir"
