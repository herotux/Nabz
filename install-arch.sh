#!/usr/bin/env bash
set -Eeuo pipefail

REPO="https://github.com/herotux/Nabz.git"
PACKAGING_BRANCH="packaging/slint-tauri-arch"
TMP_DIR=""

log() { printf '\n\033[1;32m[nabz]\033[0m %s\n' "$*"; }
fail() { printf '\n\033[1;31m[nabz: خطا]\033[0m %s\n' "$*" >&2; exit 1; }
cleanup() { [[ -z "$TMP_DIR" ]] || rm -rf -- "$TMP_DIR"; }
trap cleanup EXIT

[[ "$(uname -m)" == "x86_64" ]] || fail "فعلاً بسته‌های این نصب‌کننده برای x86_64 هستند."
[[ -r /etc/os-release ]] || fail "توزیع لینوکس شناسایی نشد."
. /etc/os-release
case " ${ID:-} ${ID_LIKE:-} " in
  *" arch "*|*" manjaro "*|*" endeavouros "*|*" cachyos "*) ;;
  *) fail "این نصب‌کننده فقط برای Arch Linux و توزیع‌های مبتنی بر آن مثل Manjaro است." ;;
esac
[[ "$EUID" -ne 0 ]] || fail "اسکریپت را با کاربر عادی اجرا کن؛ sudo فقط برای نصب وابستگی‌ها استفاده می‌شود."
command -v sudo >/dev/null 2>&1 || fail "sudo نصب نیست."
command -v pacman >/dev/null 2>&1 || fail "pacman پیدا نشد."

FRONTEND="${1:-}"
if [[ -z "$FRONTEND" ]]; then
  printf 'کدام رابط نبض نصب شود؟\n'
  printf '  1) Slint (بومی و سبک)\n'
  printf '  2) Tauri (رابط Web)\n'
  printf '  3) هر دو رابط\n'
  [[ -r /dev/tty ]] || fail "در اجرای بدون ترمینال، یکی از پارامترهای slint، tauri یا both را بده." 
  read -r -p 'انتخاب [1-3]: ' choice </dev/tty
  case "$choice" in
    1) FRONTEND=slint ;;
    2) FRONTEND=tauri ;;
    3) FRONTEND=both ;;
    *) fail "انتخاب نامعتبر است." ;;
  esac
fi
case "$FRONTEND" in slint|tauri|both) ;; *) fail "پارامتر معتبر: slint یا tauri یا both" ;; esac

PACKAGES=(base-devel git rust pkgconf fontconfig freetype2 libglvnd libx11 libxcb libxkbcommon wayland)
if [[ "$FRONTEND" == tauri || "$FRONTEND" == both ]]; then
  PACKAGES+=(nodejs npm gtk3 libayatana-appindicator librsvg openssl webkit2gtk-4.1)
fi

log "نصب ابزارهای ساخت و وابستگی‌های رابط انتخاب‌شده"
sudo pacman -S --needed --noconfirm "${PACKAGES[@]}"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nabz-arch.XXXXXXXX")"
log "دریافت فایل‌های بسته‌بندی"
git clone --depth 1 --single-branch --branch "$PACKAGING_BRANCH" "$REPO" "$TMP_DIR/packaging" || fail "دریافت شاخه بسته‌بندی از GitHub ناموفق بود."

build_install() {
  local frontend="$1"
  local build_dir="$TMP_DIR/build-$frontend"
  mkdir -p "$build_dir"
  cp "$TMP_DIR/packaging/packaging/arch/PKGBUILD.$frontend" "$build_dir/PKGBUILD"
  log "ساخت و نصب رابط $frontend از سورس معماری دو رابطی نبض"
  (
    cd "$build_dir"
    makepkg -si --noconfirm
  ) || fail "ساخت یا نصب رابط $frontend ناموفق بود. اطلاعات حسابداری حذف یا تغییر داده نشده‌اند."
}

if [[ "$FRONTEND" == both ]]; then
  build_install slint
  build_install tauri
else
  build_install "$FRONTEND"
fi

log "نصب رابط(های) انتخاب‌شده پایان یافت."
printf 'اجرای رابط بومی: nabz-slint\n'
printf 'اجرای رابط وب:   nabz-tauri\n'
printf 'داده‌های مشترک: ${XDG_DATA_HOME:-$HOME/.local/share}/com.herotux.nabz.webprototype/\n'
printf 'حذف بسته‌ها اطلاعات حسابداری را حذف نمی‌کند. برای جابه‌جایی رابط، رابط مقصد نیز باید نصب باشد.\n'
