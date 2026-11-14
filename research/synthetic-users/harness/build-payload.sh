#!/usr/bin/env bash
# build-payload.sh — produce the ONLY thing a synthetic user ever receives.
#
# A payload is a directory containing the Arch package built from a specific
# commit, the environment's `ryzenadj` provider (a stub: this VM has no SMU),
# plus MANIFEST.txt. Nothing else: no source tree, no .git, no harness scripts.
#
# Usage:
#   ./build-payload.sh [REF] [OUTDIR]     # default: HEAD
set -euo pipefail

REF=${1:-HEAD}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
CACHE="$HERE/../.cache"
COMMIT=$(git -C "$ROOT" rev-parse "$REF")
SHORT=$(git -C "$ROOT" rev-parse --short "$REF")
VERSION=$(git -C "$ROOT" show "$COMMIT:Cargo.toml" | sed -n 's/^version = "\(.*\)"/\1/p')
OUT=${2:-"$CACHE/payload/$SHORT-$(date +%Y%m%d)"}

log() { printf '[payload] %s\n' "$*" >&2; }

if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
    log "note: working tree is dirty; the payload is built from $COMMIT, not the worktree"
fi

WORK="$CACHE/build-$SHORT"
rm -rf "$WORK" "$OUT"
mkdir -p "$WORK" "$OUT"

SRC="$WORK/genarch-settings-$VERSION"
log "exporting $SHORT ($VERSION)"
git -C "$ROOT" archive --prefix="genarch-settings-$VERSION/" "$COMMIT" | tar -x -C "$WORK"
git -C "$ROOT" show "$COMMIT:PKGBUILD" > "$SRC/PKGBUILD"
git -C "$ROOT" show "$COMMIT:genarch-settings.install" > "$SRC/genarch-settings.install"

log "building genarch-settings (this is the packaging path a user installs)"
( cd "$SRC" && makepkg -f >build.log 2>&1 ) || {
    log "makepkg failed; last lines:"; tail -30 "$SRC/build.log" >&2; exit 1
}

# The dependency `ryzenadj` is AUR-only. This VM has no AMD SMU, so the
# environment provides a provider package whose binary fails exactly as the
# real tool does without the PCI device. It is not the subject under test.
STUB="$WORK/ryzenadj-stub"
mkdir -p "$STUB/usr/bin"
cat > "$STUB/ryzenadj" <<'EOF'
#!/bin/sh
echo "Error: Unable to get SMU object (no AMD SMU on this machine)" >&2
exit 1
EOF
chmod 755 "$STUB/ryzenadj"
( cd "$WORK" && cat > stub.PKGBUILD <<'EOF'
pkgname=ryzenadj
pkgver=0.19.0
pkgrel=1
pkgdesc="environment stub: the real tool is AUR-only and needs the machine's SMU"
arch=('x86_64')
provides=('ryzenadj')
conflicts=()
package() {
  install -Dm755 "$startdir/ryzenadj-stub/ryzenadj" "$pkgdir/usr/bin/ryzenadj"
}
EOF
  mv stub.PKGBUILD PKGBUILD.stub
  mkdir -p stubbuild && cp PKGBUILD.stub stubbuild/PKGBUILD && cp -a ryzenadj-stub stubbuild/
  ( cd stubbuild && makepkg -f >build.log 2>&1 ) || { tail -20 stubbuild/build.log >&2; exit 1; }
)

pkgs=()
for p in "$SRC"/*.pkg.tar.zst "$WORK"/stubbuild/*.pkg.tar.zst; do
    [ -e "$p" ] || { log "no package produced"; exit 1; }
    cp "$p" "$OUT/"
    pkgs+=("$(basename "$p")")
done

{
    echo "commit:       $COMMIT"
    echo "ref:          $REF"
    echo "version:      $VERSION"
    echo "built_at:     $(date -Is)"
    echo "host_kernel:  $(uname -r)"
    echo "guest:        Arch cloud image, kernel of the round recorded in the run manifest"
    echo "packages:"
    for f in "${pkgs[@]}"; do
        printf '  %-52s %s\n' "$f" "$(sha256sum "$OUT/$f" | cut -d' ' -f1)"
    done
    echo
    echo "payload_contents (the entire clean-room surface):"
    ( cd "$OUT" && find . -mindepth 1 -not -name MANIFEST.txt | sort )
} > "$OUT/MANIFEST.txt"

rm -rf "$WORK"
log "payload at $OUT"
cat "$OUT/MANIFEST.txt" >&2
