# The documentation the product points at is not on the machine: every report anchor, `PROFILES.md`, the `PKGBUILD`, and there is no quick start

- **Severity**: medium
- **Category**: doc
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest, 2026-11-14.

## Intent

`CONTEXT.md` says a verdict is "a one-line explanation **and the doc anchor that owns the reasoning**", and `CONTEXT.md` calls `docs/PROFILES.md` "the contract". `install.sh` says "on Arch the PKGBUILD beside this file is the supported path". The card's question — does everything the tool points at actually exist, and is the quick start really a start — should be answerable from the installed material.

## Steps

1. `genarch-settings report` (every row carries a `findings#…` anchor)
2. `ls /usr/share/doc/genarch-settings`
3. `grep -n "PROFILES.md" /usr/share/doc/genarch-settings/CONTEXT.md /usr/share/doc/genarch-settings/AGENTS.md`
4. `find / -xdev \( -name PROFILES.md -o -name findings.md -o -name PKGBUILD \) 2>/dev/null`
5. `grep -n "PKGBUILD" /usr/share/doc/genarch-settings/install.sh`
6. `grep -n "cargo test\|makepkg -f\|target/debug" /usr/share/doc/genarch-settings/AGENTS.md`
7. `pacman -Ql genarch-settings | sed 's/.* //' | grep -E 'README|/man/|PROFILES|findings'`

## Expected

The anchor in a report row should resolve on the installed system; the files the docs name as their contract and their supported packaging path should exist; and there should be a user-facing quick start (README, man page, or a "start here" doc) rather than a copy of the source repository's developer notes.

## Actual

The package ships `AGENTS.md`, `CONTEXT.md`, `CONTRIBUTING.md`, `DESIGN.md`, `LICENSE`, `SCOPE.md`, `install.sh` — the repository's developer set. There is no README and no man page. Every one of the 11 report rows carries a `findings#…` anchor and there is no `findings.md` anywhere on the machine; `docs/PROFILES.md` is named as the contract and does not exist; `install.sh` points at a `PKGBUILD` "beside this file" that is not installed; `AGENTS.md`'s command table is `cargo test`, `makepkg -f`, `./target/debug/...` — all require a source checkout this machine does not have. `DESIGN.md` is installed at the doc root while the docs point at `docs/DESIGN.md`.

## Evidence

```
$ genarch-settings report | grep -o "findings#[a-z]*" | sort -u
findings#ceil
findings#co
findings#eco
findings#intent
findings#managed
findings#ppt
findings#timer
findings#wifi
findings#writeback
$ genarch-settings report | grep -c "findings#"
11
$ find / -xdev -name findings.md 2>/dev/null | wc -l
0
```

(The anchors are used on every row; no `findings.md` exists.)

```
$ ls /usr/share/doc/genarch-settings
AGENTS.md
CONTEXT.md
CONTRIBUTING.md
DESIGN.md
LICENSE
SCOPE.md
install.sh

$ grep -n "PROFILES.md" /usr/share/doc/genarch-settings/CONTEXT.md /usr/share/doc/genarch-settings/AGENTS.md
/usr/share/doc/genarch-settings/CONTEXT.md:32:ship in a profile, and where the rest goes, is `docs/PROFILES.md`. The base carries only what is
/usr/share/doc/genarch-settings/CONTEXT.md:39:published. `docs/PROFILES.md` is the contract.
/usr/share/doc/genarch-settings/AGENTS.md:29:| `cargo test --test profiles` | the shipped-profile gate: every profile loads and keeps its provenance contract (`docs/PROFILES.md`) |
/usr/share/doc/genarch-settings/AGENTS.md:112:| A **documented** profile carries no `mirror/`, no `[co]`, no wake/USB list, and must cite a source | A value a publication cannot state is a measurement; shipping it in a documented profile is how a "portable" profile silently becomes one laptop's. `tests/profiles.rs` refuses it, and `status = "documented"` without `source` fails at load. `docs/PROFILES.md` is the contract. |

$ find / -xdev \( -name PROFILES.md -o -name findings.md -o -name PKGBUILD \) 2>/dev/null | wc -l
0

$ grep -n "PKGBUILD" install.sh
6:# binary outside the package manager; on Arch the PKGBUILD beside this file is the supported

$ grep -n "cargo test\|makepkg -f\|target/debug" AGENTS.md
20:| `cargo test` | the gate: unit tests + `tests/converge.rs` (behavioural, fixture-hosted) |
22:| `makepkg -f` | the package (binary + intent + units + udev rules) |
23:| `./target/debug/genarch-settings report [--json]` | judge live against intent; unprivileged |
24:| `sudo ./target/debug/genarch-settings apply [--dry-run]` | converge live; `--dry-run` writes nothing |
25:| `./target/debug/genarch-settings harvest` | the intent change that would match live |
26:| `./target/debug/genarch-settings units` | the units the intent expects enabled (the installers consume this) |
27:| `./target/debug/genarch-settings completions <bash\|zsh\|fish>` | a completion script on stdout; the packagers write the files |
28:| `./target/debug/genarch-settings import <state> <profile-dir>` | derive a machine profile from a legacy shell `state/` tree |
29:| `cargo test --test profiles` | the shipped-profile gate: every profile loads and keeps its provenance contract (`docs/PROFILES.md`) |
30:| `sudo ./target/debug/genarch-settings converge <boot\|resume\|ceil>` | the scheduled passes |

$ pacman -Ql genarch-settings | sed 's/.* //' | grep -E 'README|/man/|PROFILES|findings' | wc -l
0
```

The only user-facing entry point is `genarch-settings --help`; there is no quick start.

## What could not be determined

- Whether the ledger and `PROFILES.md` are deliberately shipped in a separate package or only in the source repository; `SCOPE.md` says the ledger is out of scope, which makes the report's anchor unresolvable by design.
- Whether a man page is considered part of the package's scope; `pacman -Ql` contains no `man` path.
