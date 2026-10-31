#!/usr/bin/env bash
set -euo pipefail

guard="scripts/verify-deb-data-safety.sh"
command -v dpkg-deb >/dev/null
test -x "$guard" || chmod +x "$guard"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

build_fixture() {
  local name="$1"
  local root="$tmp/$name"
  mkdir -p "$root/DEBIAN" "$root/usr/bin"
  cat > "$root/DEBIAN/control" <<CONTROL
Package: nabz-guard-$name
Version: 0.0.1
Section: misc
Priority: optional
Architecture: all
Maintainer: Nabz CI
Description: Disposable data-safety guard fixture
CONTROL
  printf '#!/bin/sh\nexit 0\n' > "$root/usr/bin/$name"
  chmod 0755 "$root/usr/bin/$name"
  dpkg-deb --build --root-owner-group "$root" "$tmp/$name.deb" >/dev/null
}

build_fixture clean
bash "$guard" "$tmp/clean.deb"

# A packaged SQLite database must be rejected.
build_fixture bundled-db
mkdir -p "$tmp/bundled-db/usr/share/nabz"
: > "$tmp/bundled-db/usr/share/nabz/nabz.sqlite3"
dpkg-deb --build --root-owner-group "$tmp/bundled-db" "$tmp/bundled-db.deb" >/dev/null
if bash "$guard" "$tmp/bundled-db.deb"; then
  echo "FAIL: guard accepted a package containing a SQLite database" >&2
  exit 1
fi

# A packaged shared-data directory must be rejected even if it is empty.
build_fixture bundled-data-dir
mkdir -p "$tmp/bundled-data-dir/home/test/.local/share/com.herotux.nabz.webprototype"
dpkg-deb --build --root-owner-group "$tmp/bundled-data-dir" "$tmp/bundled-data-dir.deb" >/dev/null
if bash "$guard" "$tmp/bundled-data-dir.deb"; then
  echo "FAIL: guard accepted a package containing the shared data directory" >&2
  exit 1
fi

# A destructive maintainer script targeting shared user data must be rejected.
build_fixture destructive-hook
cat > "$tmp/destructive-hook/DEBIAN/postrm" <<'HOOK'
#!/bin/sh
rm -rf "$HOME/.local/share/com.herotux.nabz.webprototype"
HOOK
chmod 0755 "$tmp/destructive-hook/DEBIAN/postrm"
dpkg-deb --build --root-owner-group "$tmp/destructive-hook" "$tmp/destructive-hook.deb" >/dev/null
if bash "$guard" "$tmp/destructive-hook.deb"; then
  echo "FAIL: guard accepted a destructive postrm script" >&2
  exit 1
fi

echo "PASS: package data-safety guard accepts a clean package and rejects all unsafe fixtures."
