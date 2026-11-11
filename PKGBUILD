# Maintainer: the host owner
pkgname=genarch
pkgver=0.2.2
pkgrel=1
pkgdesc="Native tuning engine: layered intent, machine profiles, read-only judge, converge pass"
arch=('x86_64')
url="https://github.com/ZhangDM-520/genarch-settings"
license=('MIT')
depends=('gcc-libs' 'ryzenadj' 'systemd' 'iw' 'wireless-regdb')
makedepends=('cargo' 'rust')
install=genarch.install
# The machine selection is operator state: an upgrade must not overwrite the choice.
backup=('etc/genarch/profile')
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

  # The intent is package data: the engine reads it, never writes it. Copied whole, so a new
  # typed file or a new profile does not need an edit here.
  install -Dm644 Cargo.toml "$pkgdir/usr/lib/genarch/package.toml"
  cp -a intent/. "$pkgdir/usr/lib/genarch/"

  # This package was built for this host, so it selects this host's profile. A host whose DMI
  # does not match reports PROFILE DRIFT instead of silently applying another machine's facts.
  install -d "$pkgdir/etc/genarch"
  printf 'profile = "zenbook-s16"\n' > "$pkgdir/etc/genarch/profile"
  chmod 644 "$pkgdir/etc/genarch/profile"

  for doc in AGENTS.md CONTEXT.md SCOPE.md CONTRIBUTING.md LICENSE; do
    install -Dm644 "$doc" "$pkgdir/usr/share/doc/genarch/$doc"
  done
  install -Dm644 docs/DESIGN.md "$pkgdir/usr/share/doc/genarch/DESIGN.md"
  install -Dm755 install.sh "$pkgdir/usr/share/doc/genarch/install.sh"
}
