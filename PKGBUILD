# Maintainer: the host owner
pkgname=genarch-settings
pkgver=0.4.0
pkgrel=1
pkgdesc="Native tuning engine: layered intent, machine profiles, read-only judge, converge pass"
arch=('x86_64')
url="https://github.com/ZhangDM-520/genarch-settings"
license=('MIT')
# 0.4.0 renamed the package with the binary. The old name ships the same files at the same
# live paths minus the five renamed ones, so leaving it installed would be a second writer.
conflicts=('genarch')
replaces=('genarch')
depends=('gcc-libs' 'ryzenadj' 'systemd' 'iw' 'wireless-regdb')
makedepends=('cargo' 'rust')
install=genarch-settings.install
# The machine selection is operator state: an upgrade must not overwrite the choice.
backup=('etc/genarch-settings/profile')
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
  install -Dm755 target/release/genarch-settings "$pkgdir/usr/bin/genarch-settings"

  # The intent is package data: the engine reads it, never writes it. Copied whole, so a new
  # typed file or a new profile does not need an edit here.
  install -Dm644 Cargo.toml "$pkgdir/usr/lib/genarch-settings/package.toml"
  cp -a intent/. "$pkgdir/usr/lib/genarch-settings/"

  # This package was built for this host, so it selects this host's profile. A host whose DMI
  # does not match reports PROFILE DRIFT instead of silently applying another machine's facts.
  install -d "$pkgdir/etc/genarch-settings"
  printf 'profile = "zenbook-s16"\n' > "$pkgdir/etc/genarch-settings/profile"
  chmod 644 "$pkgdir/etc/genarch-settings/profile"

  # Completions are generated from the binary's own command tree (the `completions` verb is the
  # one home), so a new verb or flag can never leave a stale script in the package.
  local spec shell dest
  for spec in \
    "bash:usr/share/bash-completion/completions/genarch-settings" \
    "zsh:usr/share/zsh/site-functions/_genarch-settings" \
    "fish:usr/share/fish/vendor_completions.d/genarch-settings.fish"
  do
    shell="${spec%%:*}"
    dest="$pkgdir/${spec#*:}"
    install -Dm644 /dev/null "$dest"
    target/release/genarch-settings completions "$shell" > "$dest"
  done

  for doc in AGENTS.md CONTEXT.md SCOPE.md CONTRIBUTING.md LICENSE; do
    install -Dm644 "$doc" "$pkgdir/usr/share/doc/genarch-settings/$doc"
  done
  install -Dm644 docs/DESIGN.md "$pkgdir/usr/share/doc/genarch-settings/DESIGN.md"
  install -Dm755 install.sh "$pkgdir/usr/share/doc/genarch-settings/install.sh"
}
