#!/usr/bin/env bash
# genarch — general installer (no pacman).
#
# Installs the engine, the intent (base + profiles) and the documentation from a source tree or
# from a tagged release tarball. Use this on a host without this recipe, or when you want the
# binary outside the package manager; on Arch the PKGBUILD beside this file is the supported
# path.
#
# The engine reads its intent from $INTENT_DIR (default /usr/lib/genarch) and its machine
# layer from <prefix>/etc/genarch/profile. Nothing selects a profile for you: a fresh host runs
# the base intent and reports PROFILE SKIPPED until an operator writes that line. The package
# ships this host's selection; the installer is the portable path, so it does not guess.
set -euo pipefail

REPO="https://github.com/ZhangDM-520/genarch-settings"
VERSION="0.2.0"
PREFIX="/usr"
# Fixed, not derived from --prefix: the engine resolves /usr/lib/genarch itself; a relocated
# intent is a run-time `GENARCH_INTENT` override, not an install layout.
INTENT_DIR="/usr/lib/genarch"
DOC_DIR=""
SOURCE=""
PROFILE=""
ENABLE=1
DRY=0
TMP=""

usage() {
  cat <<'USAGE'
usage: install.sh [options]

  --source DIR      build from DIR instead of downloading the tagged tarball
  --version VER     release to download (default: the version this script shipped with)
  --prefix PATH     where the binary goes (default /usr)
  --intent-dir DIR  where the intent goes (default /usr/lib/genarch)
  --doc-dir DIR     where the documentation goes (default <prefix>/share/doc/genarch)
  --profile NAME    select this machine profile in /etc/genarch/profile
                    (default: leave the selection alone; base only when unset)
  --no-units        install files only: no apply, no daemon-reload, no enable
  --dry-run         print every action; change nothing
  -h, --help        this text

Profiles live in <intent-dir>/profiles/NAME and declare the machine they are for ([requires]).
Selecting one for another laptop is not an error: `genarch report` says PROFILE DRIFT and
exits 1, and `apply` still runs, because the operator has to be able to see it and fix it.

The engine is idempotent: installing over an existing one re-plans and converges, it does not
reset anything.  Uninstall: disable genarch.service genarch-ceil.timer, remove the binary, the
intent directory and the documentation, then run `systemctl daemon-reload`.
USAGE
}

say() { printf 'genarch-install: %s\n' "$*"; }
die() { printf 'genarch-install: %s\n' "$*" >&2; exit 1; }
# `run` is the only place a mutation happens, so --dry-run is a property of the script rather
# than something every call site has to remember.
run() {
  if (( DRY )); then
    printf 'DRY-RUN'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

while (( $# )); do
  case "$1" in
    --source)     SOURCE="${2:?--source needs a directory}"; shift 2 ;;
    --version)    VERSION="${2:?--version needs a value}"; shift 2 ;;
    --prefix)     PREFIX="${2:?--prefix needs a path}"; shift 2 ;;
    --intent-dir) INTENT_DIR="${2:?--intent-dir needs a path}"; shift 2 ;;
    --doc-dir)    DOC_DIR="${2:?--doc-dir needs a path}"; shift 2 ;;
    --profile)    PROFILE="${2:?--profile needs a name}"; shift 2 ;;
    --no-units)   ENABLE=0; shift ;;
    --dry-run)    DRY=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    *)            printf 'genarch-install: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

(( EUID == 0 )) || (( DRY )) || die "must run as root (or pass --dry-run to preview)"
[[ -n $DOC_DIR ]] || DOC_DIR="$PREFIX/share/doc/genarch"

if [[ -z $SOURCE ]]; then
  if [[ -f ./Cargo.toml ]] && grep -q '^name = "genarch"' ./Cargo.toml; then
    SOURCE="$PWD"
    say "building the checkout at $SOURCE"
  else
    command -v curl >/dev/null 2>&1 || die "curl is required to fetch the release tarball"
    command -v tar  >/dev/null 2>&1 || die "tar is required to unpack the release tarball"
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    url="$REPO/archive/v$VERSION.tar.gz"
    say "fetching $url"
    # Download and unpack are temp-only, so they run for real even under --dry-run: the
    # source resolution has to be verifiable before the preview claims anything.
    curl -fsSL "$url" -o "$TMP/genarch.tar.gz"
    tar -xzf "$TMP/genarch.tar.gz" -C "$TMP"
    SOURCE="$TMP/genarch-settings-$VERSION"
  fi
fi

[[ -f "$SOURCE/Cargo.toml" ]] || die "no Cargo.toml under $SOURCE"
grep -q '^name = "genarch"' "$SOURCE/Cargo.toml" || die "$SOURCE is not the genarch tree"

# Build inputs. A missing runtime dependency is reported, never fatal: the report marks the
# rows it cannot judge as SKIPPED, and an install that refuses to proceed over a missing
# `ryzenadj` would be unable to fix anything.
command -v cargo >/dev/null 2>&1 || die "cargo is required to build (install the rust toolchain)"
for dep in ryzenadj iw systemctl udevadm sudo; do
  command -v "$dep" >/dev/null 2>&1 \
    || say "warning: $dep not found — dependent rows will be SKIPPED or inert"
done

if [[ -n $PROFILE && ! -d $SOURCE/intent/profiles/$PROFILE ]]; then
  die "no profile '$PROFILE' in $SOURCE/intent/profiles"
fi

say "building (release, locked)"
run cargo build --release --locked --manifest-path "$SOURCE/Cargo.toml"
BIN="$SOURCE/target/release/genarch"
[[ -x $BIN ]] || die "the build produced no binary at $BIN"

say "installing the binary to $PREFIX/bin"
run install -d "$PREFIX/bin"
run install -Dm755 "$BIN" "$PREFIX/bin/genarch"

say "installing the intent to $INTENT_DIR"
# The whole tree, not a file list: a new typed file must not need an installer edit, and the
# profile layer is intent too. Deleting first keeps a stale mirror from surviving an upgrade.
run install -d "$INTENT_DIR"
for dir in managed mirror profiles; do
  run rm -rf "$INTENT_DIR/$dir"
done
for file in intent.toml sysctl.toml ryzenadj.toml passes.toml ceil.toml; do
  if [[ -f $SOURCE/intent/$file ]]; then
    run install -Dm644 "$SOURCE/intent/$file" "$INTENT_DIR/$file"
  fi
done
for dir in managed mirror profiles; do
  run cp -a "$SOURCE/intent/$dir" "$INTENT_DIR/$dir"
done

say "installing documentation to $DOC_DIR"
for doc in AGENTS.md CONTEXT.md SCOPE.md CONTRIBUTING.md LICENSE; do
  run install -Dm644 "$SOURCE/$doc" "$DOC_DIR/$doc"
done
run install -Dm644 "$SOURCE/docs/DESIGN.md" "$DOC_DIR/DESIGN.md"
run install -Dm755 -- "$0" "$DOC_DIR/install.sh"

# The selection is operator state: written only when asked, and never silently replaced.
# `/usr` is the real root, so the selection lands in /etc; a staged prefix keeps it staged.
SELECTION="${PREFIX%/usr}/etc/genarch/profile"
if [[ -n $PROFILE ]]; then
  if [[ -f $SELECTION ]] && ! grep -qx "profile = \"$PROFILE\"" "$SELECTION"; then
    die "$SELECTION already selects another profile; edit it by hand (one machine, one layer)"
  fi
  say "selecting profile '$PROFILE' in $SELECTION"
  run install -d "$(dirname "$SELECTION")"
  if (( DRY )); then
    printf 'DRY-RUN write profile = "%s" to %s\n' "$PROFILE" "$SELECTION"
  else
    printf 'profile = "%s"\n' "$PROFILE" > "$SELECTION"
  fi
elif [[ -f $SELECTION ]]; then
  say "keeping the existing selection in $SELECTION"
else
  say "no profile selected: the base intent applies and PROFILE reports SKIPPED"
  say "select one later by writing profile = \"NAME\" into $SELECTION"
fi

if (( ENABLE )); then
  say "converging the host to the intent"
  run "$PREFIX/bin/genarch" apply --dry-run
  run "$PREFIX/bin/genarch" apply
  run systemctl daemon-reload
  run udevadm control --reload
  run systemctl enable --now genarch.service genarch-ceil.timer
fi

say "done — read-only check: $PREFIX/bin/genarch report"
(( DRY )) || "$PREFIX/bin/genarch" report || true
