#!/usr/bin/env bash
set -Eeuo pipefail

REPO="herotux/Nabz"
API="https://api.github.com/repos/${REPO}/releases/latest"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

die() { printf 'خطا: %s\n' "$*" >&2; exit 1; }
for cmd in curl python3 sha256sum pacman sudo; do
  command -v "$cmd" >/dev/null 2>&1 || die "ابزار $cmd نصب نیست."
done

[[ "$(uname -m)" == "x86_64" ]] || die "این بسته فعلاً فقط برای x86_64 ساخته می‌شود."
[[ -n "${HOME:-}" ]] || die 'HOME تنظیم نشده است.'

printf 'دریافت آخرین نسخهٔ منتشرشدهٔ Nabz...\n'
curl --fail --silent --show-error --location --retry 3 "$API" -o "$TMP_DIR/release.json" \
  || die 'نسخهٔ منتشرشده‌ای پیدا نشد یا GitHub در دسترس نیست. ابتدا باید یک Release منتشر شود.'

readarray -t ASSETS < <(python3 - "$TMP_DIR/release.json" <<'PY'
import json, sys
release = json.load(open(sys.argv[1], encoding="utf-8"))
assets = release.get("assets", [])
packages = [a for a in assets if a.get("name", "").endswith("-x86_64.pkg.tar.zst")]
if not packages:
    raise SystemExit("در آخرین Release بستهٔ x86_64 وجود ندارد.")
package = packages[0]
checksums = [a for a in assets if a.get("name") == package["name"] + ".sha256"]
if not checksums:
    raise SystemExit("فایل checksum همراه بسته منتشر نشده؛ نصب برای جلوگیری از دریافت ناقص متوقف شد.")
print(package["browser_download_url"])
print(checksums[0]["browser_download_url"])
print(package["name"])
print(release.get("tag_name", "unknown"))
PY
) || die 'اطلاعات دارایی‌های Release معتبر نیست.'

PKG_URL="${ASSETS[0]}"
SUM_URL="${ASSETS[1]}"
PKG_NAME="${ASSETS[2]}"
TAG="${ASSETS[3]}"

printf 'نسخه: %s\n' "$TAG"
curl --fail --silent --show-error --location --retry 3 "$PKG_URL" -o "$TMP_DIR/$PKG_NAME" \
  || die 'دریافت بسته ناموفق بود.'
curl --fail --silent --show-error --location --retry 3 "$SUM_URL" -o "$TMP_DIR/$PKG_NAME.sha256" \
  || die 'دریافت checksum ناموفق بود.'
(cd "$TMP_DIR" && sha256sum --check "$PKG_NAME.sha256") \
  || die 'checksum بسته معتبر نیست؛ هیچ تغییری در سیستم انجام نشد.'

# Keep an independent SQLite snapshot before any package upgrade. The
# application data directory is never removed by this installer.
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DB="$DATA_HOME/com.herotux.nabz/nabz.sqlite3"
if [[ -s "$DB" ]]; then
  BACKUP_DIR="$DATA_HOME/com.herotux.nabz/backups"
  mkdir -p "$BACKUP_DIR"
  chmod 700 "$BACKUP_DIR"
  BACKUP="$BACKUP_DIR/pre-install-$(date -u +%Y%m%dT%H%M%SZ).sqlite3"
  python3 - "$DB" "$BACKUP" <<'PY'
import os, sqlite3, sys
source, target = sys.argv[1:]
src = sqlite3.connect("file:" + source + "?mode=ro", uri=True, timeout=30)
dst = sqlite3.connect(target)
try:
    src.backup(dst)
    result = dst.execute("PRAGMA integrity_check").fetchone()[0]
    if result != "ok":
        raise RuntimeError("SQLite integrity_check failed: " + str(result))
finally:
    dst.close()
    src.close()
os.chmod(target, 0o600)
print("پشتیبان پیش از نصب:", target)
PY
fi

printf 'نصب وابستگی‌ها از مخازن رسمی Arch/Manjaro...\n'
sudo pacman -S --needed webkit2gtk-4.1 gtk3 openssl
printf 'نصب/ارتقای Nabz...\n'
sudo pacman -U --needed "$TMP_DIR/$PKG_NAME"

printf '\nنصب تمام شد. دیتابیس کاربر حذف نشده است.\n'
printf 'برای اجرا از منوی برنامه‌ها «نبض» یا دستور nabz را اجرا کن.\n'
