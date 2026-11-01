# Maintainer: Hamid Saidy <herotux@users.noreply.github.com>
pkgname=nabz
pkgver=0.1.0.r1.g0000000
pkgrel=1
pkgdesc="Persian RTL accounting and shop-management application"
arch=('x86_64')
url="https://github.com/herotux/Nabz"
license=('MIT')
depends=('gcc-libs' 'glibc' 'libglvnd' 'libx11' 'libxcb' 'libxkbcommon' 'wayland' 'fontconfig' 'freetype2')
makedepends=('git' 'rust' 'pkgconf')
optdepends=('cups: printer discovery and printing')
provides=('nabz')
conflicts=('nabz-git')
source=("nabz::git+https://github.com/herotux/Nabz.git#branch=main")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/nabz"
  local version count revision
  version="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -n1)"
  count="$(git rev-list --count HEAD)"
  revision="$(git rev-parse --short=7 HEAD)"
  printf '%s.r%s.g%s' "${version//-/_}" "$count" "$revision"
}

prepare() {
  cd "$srcdir/nabz"
  bash scripts/fetch-font.sh
}

build() {
  cd "$srcdir/nabz"
  cargo build --release --locked
}

check() {
  cd "$srcdir/nabz"
  cargo test --locked
}

package() {
  cd "$srcdir/nabz"
  install -Dm755 target/release/nabz "$pkgdir/usr/bin/nabz"
  install -Dm644 packaging/nabz.desktop "$pkgdir/usr/share/applications/nabz.desktop"
  install -Dm644 assets/nabz.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/nabz.svg"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
