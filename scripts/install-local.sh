#!/usr/bin/env bash
set -euo pipefail
APPIMAGE="${1:-}"
if [[ -z "$APPIMAGE" || ! -f "$APPIMAGE" ]]; then
  echo "Usage: $0 /path/to/Nabz-x86_64.AppImage" >&2
  exit 2
fi
install_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
mkdir -p "$install_dir"
install -m 0755 "$APPIMAGE" "$install_dir/Nabz.AppImage"
echo "Installed to $install_dir/Nabz.AppImage"
echo "Run it with: $install_dir/Nabz.AppImage"
