#!/usr/bin/env bash
# genarch-settings — general installer (no pacman).
#
# Installs the engine, the intent (base + profiles) and the documentation from a source tree or
# from a tagged release tarball. Use this on a host without this recipe, or when you want the
# binary outside the package manager; on Arch the PKGBUILD beside this file is the supported
# path.
#
# The engine reads its intent from $INTENT_DIR (default /usr/lib/genarch-settings) and its machine
# layer from <prefix>/etc/genarch-settings/profile. Nothing selects a profile for you: a fresh host runs
# the base intent and reports PROFILE SKIPPED until an operator writes that line. The package
# ships this host's selection; the installer is the portable path, so it does not guess.
set -euo pipefail

REPO="https://github.com/ZhangDM-520/genarch-settings"
VERSION="0.3.1"
PREFIX="/usr"
# Fixed, not derived from --prefix: the engine resolves /usr/lib/genarch-settings itself; a relocated
# intent is a run-time `GENARCH_SETTINGS_INTENT` override, not an install layout.
INTENT_DIR="/usr/lib/genarch-settings"
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
  --intent-dir DIR  where the intent goes (default /usr/lib/genarch-settings)
  --doc-dir DIR     where the documentation goes (default <prefix>/share/doc/genarch-settings)
  --profile NAME    select this machine profile in /etc/genarch-settings/profile
                    (default: leave the selection alone; base only when unset)
  --no-units        install files only: no apply, no daemon-reload, no enable
  --dry-run         print every action; change nothing
  -h, --help        this text

Profiles live in <intent-dir>/profiles/NAME and declare the machine they are for ([requires]).
Selecting one for another laptop is not an error: `genarch-settings report` says PROFILE DRIFT and
exits 1, and `apply` still runs, because the operator has to be able to see it and fix it.

The engine is idempotent: installing over an existing one re-plans and converges, it does not
reset anything.  Uninstall: `systemctl disable` the units `genarch-settings units` prints, remove the
binary, the intent directory and the documentation, then run `systemctl daemon-reload`.
USAGE
}

say() { printf 'genarch-settings-install: %s\n' "$*"; }
die() { printf 'genarch-settings-install: %s\n' "$*" >&2; exit 1; }
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

# 0.4.0 renamed the binary *and* the three live directories. The ownership record under
# /var/lib is what lets the new engine prune the five live files that changed name; without it
# they are orphaned and a stale unit stays a second writer for the same kernel attribute. Moved,
# never copied, and only when the destination is absent.
migrate_state() {
  local pair old new
  for pair in "/etc/genarch:/etc/genarch-settings" "/var/lib/genarch:/var/lib/genarch-settings"; do
    old="${pair%%:*}"
    new="${pair#*:}"
    if [[ -d $old && ! -e $new ]]; then
      say "migrating $old to $new (the manifest is the ownership record)"
      run mv "$old" "$new"
    fi
  done
}

# Completions come from the binary's own command tree, so the installed scripts cannot describe
# verbs or flags it does not have.
install_completions() {
  local bin="$1" root="$2" spec shell dest
  say "installing completions under $root/share"
  for spec in \
    "bash:share/bash-completion/completions/genarch-settings" \
    "zsh:share/zsh/site-functions/_genarch-settings" \
    "fish:share/fish/vendor_completions.d/genarch-settings.fish"
  do
    shell="${spec%%:*}"
    dest="$root/${spec#*:}"
    if (( DRY )); then
      printf 'DRY-RUN %q completions %s > %s\n' "$bin" "$shell" "$dest"
    else
      install -d "$(dirname "$dest")"
      "$bin" completions "$shell" > "$dest" || die "could not write $dest"
    fi
  done
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
    *)            printf 'genarch-settings-install: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

(( EUID == 0 )) || (( DRY )) || die "must run as root (or pass --dry-run to preview)"
[[ -n $DOC_DIR ]] || DOC_DIR="$PREFIX/share/doc/genarch-settings"
migrate_state

if [[ -z $SOURCE ]]; then
  if [[ -f ./Cargo.toml ]] && grep -q '^name = "genarch-settings"' ./Cargo.toml; then
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
    curl -fsSL "$url" -o "$TMP/genarch-settings.tar.gz"
    tar -xzf "$TMP/genarch-settings.tar.gz" -C "$TMP"
    SOURCE="$TMP/genarch-settings-$VERSION"
  fi
fi

[[ -f "$SOURCE/Cargo.toml" ]] || die "no Cargo.toml under $SOURCE"
grep -q '^name = "genarch-settings"' "$SOURCE/Cargo.toml" || die "$SOURCE is not the genarch-settings tree"
# The version is the tree's, not a second copy of it. Only the fetch path above needs the
# constant this script shipped with.
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$SOURCE/Cargo.toml" | head -n1)"
[[ -n $VERSION ]] || die "no version in $SOURCE/Cargo.toml"

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
BIN="$SOURCE/target/release/genarch-settings"
[[ -x $BIN ]] || die "the build produced no binary at $BIN"

say "installing the binary to $PREFIX/bin"
run install -d "$PREFIX/bin"
run install -Dm755 "$BIN" "$PREFIX/bin/genarch-settings"

install_completions "$PREFIX/bin/genarch-settings" "$PREFIX"

say "installing the intent to $INTENT_DIR"
# The whole tree, not a file list: a new typed file or a new profile must not need an
# installer edit, and the profile layer is intent too. Deleting first keeps a stale mirror
# from surviving an upgrade.
run install -d "$INTENT_DIR"
for dir in managed mirror profiles; do
  run rm -rf "$INTENT_DIR/$dir"
done
run cp -a "$SOURCE/intent/." "$INTENT_DIR/"

# The pre-0.4.0 tree is not ours to keep: leaving it makes /usr/lib/genarch a second copy of the
# intent that no reader consults. package.toml proves the directory is this project's.
if [[ -f $PREFIX/lib/genarch/package.toml ]]; then
  say "removing the legacy intent tree $PREFIX/lib/genarch"
  run rm -rf "$PREFIX/lib/genarch"
fi

say "installing documentation to $DOC_DIR"
for doc in AGENTS.md CONTEXT.md SCOPE.md CONTRIBUTING.md LICENSE; do
  run install -Dm644 "$SOURCE/$doc" "$DOC_DIR/$doc"
done
run install -Dm644 "$SOURCE/docs/DESIGN.md" "$DOC_DIR/DESIGN.md"
run install -Dm755 -- "$0" "$DOC_DIR/install.sh"

# The selection is operator state: written only when asked, and never silently replaced.
# `/usr` is the real root, so the selection lands in /etc; a staged prefix keeps it staged.
SELECTION="${PREFIX%/usr}/etc/genarch-settings/profile"
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
  # A legacy timer firing between the rename and the apply would write through a unit this
  # install is about to prune. `systemctl disable` needs the unit file still on disk, so it runs
  # first; a host that never had the old engine has no such units and that is not an error.
  say "disabling the pre-0.4.0 units"
  run systemctl disable --now genarch.service genarch-ceil.timer 2>/dev/null || true

  say "converging the host to the intent"
  run "$PREFIX/bin/genarch-settings" apply --dry-run
  run "$PREFIX/bin/genarch-settings" apply
  run systemctl daemon-reload
  run udevadm control --reload
  # The unit list has one home: ask the intent, do not restate it here. A real run asks the
  # intent just installed (the copy above already happened); a dry run can only ask the
  # installed one, so it never answers from a stale build left in the source tree.
  UNITS_CMD="$BIN"
  (( DRY )) && UNITS_CMD="$PREFIX/bin/genarch-settings"
  [[ -x $UNITS_CMD ]] || UNITS_CMD="$PREFIX/bin/genarch-settings"
  mapfile -t UNITS < <("$UNITS_CMD" units 2>/dev/null || true)
  if (( ${#UNITS[@]} )); then
    run systemctl enable --now "${UNITS[@]}"
  else
    say "warning: the intent declares no units, or the engine cannot read it; nothing enabled"
  fi
fi

say "done — read-only check: $PREFIX/bin/genarch-settings report"
(( DRY )) || "$PREFIX/bin/genarch-settings" report || true
