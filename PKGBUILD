# Maintainer: the host owner
pkgname=genarch
pkgver=0.1.0
pkgrel=1
pkgdesc="Native tuning engine: declared intent, read-only judge, converge pass, ryzenadj settings"
arch=('x86_64')
url="https://github.com/ZhangDM-520/genarch-settings"
license=('GPL-2.0-or-later')
depends=('gcc-libs' 'ryzenadj' 'systemd' 'iw' 'wireless-regdb')
makedepends=('cargo' 'rust')
install=genarch.install
options=(!lto)

source=()
sha256sums=()

# Builds in place: this package ships the working tree, so a release is a tag of the repo.
build() {
  cd "$startdir"
  cargo build --release --locked
}

package() {
  cd "$startdir"
  install -Dm755 target/release/genarch "$pkgdir/usr/bin/genarch"

  # The intent is package data: the engine reads it, never writes it.
  install -Dm644 Cargo.toml "$pkgdir/usr/lib/genarch/package.toml"
  for f in intent.toml sysctl.toml ryzenadj.toml passes.toml; do
    install -Dm644 "intent/$f" "$pkgdir/usr/lib/genarch/$f"
  done
  cp -a intent/managed "$pkgdir/usr/lib/genarch/managed"
  cp -a intent/mirror "$pkgdir/usr/lib/genarch/mirror"

  install -Dm644 AGENTS.md "$pkgdir/usr/share/doc/genarch/AGENTS.md"
  install -Dm644 CONTEXT.md "$pkgdir/usr/share/doc/genarch/CONTEXT.md"
  install -Dm644 docs/DESIGN.md "$pkgdir/usr/share/doc/genarch/DESIGN.md"
}
