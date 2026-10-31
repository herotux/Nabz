#!/usr/bin/env bash
set -euo pipefail

command -v dpkg-deb >/dev/null
command -v dpkg >/dev/null
command -v sudo >/dev/null
command -v python3 >/dev/null

tmp="$(mktemp -d)"
trap 'sudo dpkg -r nabz-slint-lifecycle-test nabz-tauri-lifecycle-test >/dev/null 2>&1 || true; rm -rf "$tmp"' EXIT

export XDG_DATA_HOME="$tmp/xdg-data"
data_dir="$XDG_DATA_HOME/com.herotux.nabz.webprototype"
database="$data_dir/nabz.sqlite3"
mkdir -p "$data_dir"

# Seed a real SQLite file at the documented product-owned path.
python3 - "$database" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute("CREATE TABLE lifecycle_probe (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
db.execute("INSERT INTO lifecycle_probe(value) VALUES ('must-survive-uninstall')")
db.commit()
db.close()
PY

make_package() {
  local package_name="$1"
  local executable="$2"
  local package_root="$tmp/package-$package_name"
  mkdir -p "$package_root/DEBIAN" "$package_root/usr/bin"
  cat > "$package_root/DEBIAN/control" <<CONTROL
Package: $package_name
Version: 0.0.1
Section: misc
Priority: optional
Architecture: all
Maintainer: Nabz CI
Description: Disposable frontend lifecycle fixture
CONTROL
  printf '#!/bin/sh\necho lifecycle-fixture\n' > "$package_root/usr/bin/$executable"
  chmod 0755 "$package_root/usr/bin/$executable"
  dpkg-deb --build --root-owner-group "$package_root" "$tmp/$package_name.deb" >/dev/null
}

make_package nabz-slint-lifecycle-test nabz-slint-lifecycle-test
make_package nabz-tauri-lifecycle-test nabz-tauri-lifecycle-test

sudo dpkg -i "$tmp/nabz-slint-lifecycle-test.deb" "$tmp/nabz-tauri-lifecycle-test.deb" >/dev/null
dpkg-query -W -f='${Status}\n' nabz-slint-lifecycle-test | grep -q 'install ok installed'
dpkg-query -W -f='${Status}\n' nabz-tauri-lifecycle-test | grep -q 'install ok installed'

# Removing one UI must leave the other package and the product-owned database.
sudo dpkg -r nabz-slint-lifecycle-test >/dev/null
dpkg-query -W -f='${Status}\n' nabz-tauri-lifecycle-test | grep -q 'install ok installed'
test -s "$database"
python3 - "$database" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
value = db.execute("SELECT value FROM lifecycle_probe WHERE id=1").fetchone()[0]
assert value == "must-survive-uninstall", value
assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
db.close()
PY

# Removing the remaining UI and reinstalling the first one must still preserve data.
sudo dpkg -r nabz-tauri-lifecycle-test >/dev/null
test -s "$database"
sudo dpkg -i "$tmp/nabz-slint-lifecycle-test.deb" >/dev/null
test -x /usr/bin/nabz-slint-lifecycle-test
python3 - "$database" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
assert db.execute("SELECT value FROM lifecycle_probe WHERE id=1").fetchone()[0] == "must-survive-uninstall"
assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
db.close()
PY

echo "PASS: dpkg install/remove/reinstall lifecycle preserved the shared SQLite database."
