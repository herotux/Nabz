#!/usr/bin/env bash
# Fail a Linux package build if a frontend installer ships user data or has
# maintainer scripts that can delete the shared Nabz data directory.
set -euo pipefail

if [[ $# -ne 1 || ! -f "$1" ]]; then
  echo "Usage: $0 /path/to/package.deb" >&2
  exit 2
fi
command -v dpkg-deb >/dev/null 2>&1 || {
  echo "dpkg-deb is required" >&2
  exit 2
}

package=$1
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

dpkg-deb --extract "$package" "$tmp/root"
mkdir -p "$tmp/control"
dpkg-deb --control "$package" "$tmp/control"

# A frontend package must never contain a user's live database or its data tree.
if find "$tmp/root" -type f \( -name 'nabz.sqlite3' -o -name 'nabz.sqlite3-*' -o -name '*.sqlite3-wal' -o -name '*.sqlite3-shm' \) -print -quit | grep -q .; then
  echo "Unsafe package: contains a Nabz SQLite database or sidecar file" >&2
  exit 1
fi
if find "$tmp/root" -type d -name 'com.herotux.nabz.webprototype' -print -quit | grep -q .; then
  echo "Unsafe package: contains the shared per-user data directory" >&2
  exit 1
fi

# Maintainer scripts may clean package-owned files, never shared user data.
for hook in preinst postinst prerm postrm config; do
  file="$tmp/control/$hook"
  [[ -f "$file" ]] || continue
  if grep -Eiq '(^|[;&|[:space:]])(rm|rmdir|find)[^#]*(com\.herotux\.nabz\.webprototype|nabz\.sqlite3|XDG_DATA_HOME|\.local/share)' "$file"; then
    echo "Unsafe package: $hook references shared Nabz data in a destructive command" >&2
    exit 1
  fi
done

echo "Data-safety check passed: $package"
