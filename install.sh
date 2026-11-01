#!/usr/bin/env bash
set -Eeuo pipefail

REPO="https://github.com/herotux/Nabz.git"
BRANCH="main"
APP_NAME="نبض"

log() { printf '\n\033[1;32m[nabz]\033[0m %s\n' "$*"; }
fail() { printf '\n\033[1;31m[nabz: خطا]\033[0m %s\n' "$*" >&2; exit 1; }
cleanup() { [[ -z "${WORK_DIR:-}" ]] || rm -rf -- "$WORK_DIR"; }
trap cleanup EXIT

[[ "$(uname -m)" == "x86_64" ]] || fail "فعلاً نصب‌کننده فقط برای x86_64 آماده شده است."
[[ -r /etc/os-release ]] || fail "توزیع لینوکس شناسایی نشد."
. /etc/os-release
case " ${ID:-} ${ID_LIKE:-} " in
  *" arch "*|*" manjaro "*|*" endeavouros "*|*" cachyos "*) ;;
  *) fail "این نصب‌کننده برای Arch Linux و توزیع‌های مبتنی بر آن (از جمله Manjaro) است." ;;
esac

if [[ "$EUID" -eq 0 ]]; then
  fail "اسکریپت را با کاربر عادی اجرا کن؛ sudo را خودش فقط برای نصب بسته‌ها فراخوانی می‌کند."
fi
command -v sudo >/dev/null 2>&1 || fail "sudo نصب نیست. ابتدا sudo را نصب و دسترسی کاربر را تنظیم کن."
command -v pacman >/dev/null 2>&1 || fail "pacman پیدا نشد؛ این توزیع پشتیبانی نمی‌شود."

log "نصب ابزارهای لازم از مخازن رسمی"
sudo pacman -S --needed --noconfirm base-devel git rust pkgconf

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nabz-install.XXXXXXXX")"
log "دریافت آخرین سورس نبض از شاخه $BRANCH"
git clone --depth 1 --single-branch --branch "$BRANCH" "$REPO" "$WORK_DIR/Nabz" || fail "دریافت سورس از GitHub ناموفق بود؛ اتصال اینترنت را بررسی کن."

cd "$WORK_DIR/Nabz"
[[ -s PKGBUILD ]] || fail "فایل PKGBUILD در سورس وجود ندارد."
log "ساخت و نصب بسته بومی Arch با makepkg"
makepkg -si --noconfirm || fail "ساخت یا نصب بسته ناموفق بود. اطلاعات کاربر و دیتابیس دست‌نخورده باقی مانده‌اند."

log "نصب نبض با موفقیت پایان یافت."
printf 'برای اجرا: nabz\n'
printf 'پایگاه داده در ~/.local/share/nabz/nabz.sqlite3 نگهداری می‌شود و این نصب‌کننده آن را حذف نمی‌کند.\n'
