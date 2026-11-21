#!/usr/bin/env bash
set -Eeuo pipefail

die() { printf 'خطا: %s\n' "$*" >&2; exit 1; }
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "$(uname -m)" == "aarch64" ]] || die "این بسته مخصوص Raspberry Pi OS 64-bit با معماری ARM64 است."
[[ -r /etc/os-release ]] || die "تشخیص سیستم‌عامل ممکن نیست."
. /etc/os-release
[[ "${ID:-}" == "debian" || "${ID_LIKE:-}" == *debian* ]] || die "این نصب‌کننده برای Raspberry Pi OS / Debian طراحی شده است."
[[ "${HOME:-}" && -d "$HOME" ]] || die "پوشه خانگی کاربر پیدا نشد."

command -v sudo >/dev/null 2>&1 || die "sudo نصب نیست."
command -v apt-get >/dev/null 2>&1 || die "apt-get در دسترس نیست."
[[ -x "$SCRIPT_DIR/bin/nabz" ]] || die "فایل اجرایی بسته پیدا نشد."
[[ -s "$SCRIPT_DIR/share/icons/nabz.png" ]] || die "آیکون بسته پیدا نشد."

printf 'نصب وابستگی‌های لازم از مخازن Raspberry Pi OS...\n'
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 librsvg2-2 libssl3

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -m 0755 "$SCRIPT_DIR/bin/nabz" "$BIN_DIR/nabz"
install -m 0644 "$SCRIPT_DIR/share/icons/nabz.png" "$ICON_DIR/nabz.png"
install -m 0644 "$SCRIPT_DIR/share/applications/nabz.desktop" "$APP_DIR/nabz.desktop"

# Ensure the desktop launcher resolves the per-user binary and icon.
sed -i "s|^Exec=nabz$|Exec=$BIN_DIR/nabz|" "$APP_DIR/nabz.desktop"
sed -i "s|^Icon=nabz$|Icon=$ICON_DIR/nabz.png|" "$APP_DIR/nabz.desktop"

printf '\nنصب نبض کامل شد.\n'
printf 'اجرا: %s/nabz\n' "$BIN_DIR"
printf 'اگر دستور nabz در ترمینال شناخته نشد، یک بار خارج و دوباره وارد شو یا مسیر بالا را اجرا کن.\n'
printf 'اطلاعات برنامه در پوشه داده‌های کاربر باقی می‌ماند و این نصب‌کننده آن را حذف نمی‌کند.\n'
