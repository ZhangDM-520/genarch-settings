# The only profile check the guide offers is a cargo test the installed package cannot run

- **Severity:** medium
- **Category:** doc
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1 installed with pacman; no source tree, no `cargo`

## Intent

Validate a hand-written profile before selecting it, as `PROFILES.md` instructs.

## Steps

```sh
grep -n "Adding a profile" /usr/share/doc/genarch-settings/PROFILES.md
command -v cargo
genarch-settings --help
```

## Expected

The sentence "Adding a profile is therefore: write the directory, run `cargo test --test
profiles`, fix what it says" describes the gate a porter is told to use. A porter working from
the installed package should be able to run it, or be told what replaces it.

## Actual

`cargo` is not installed and there is no `Cargo.toml` on this box: the gate is part of an
unshipped source tree. The binary has no verb that loads and validates a profile
(`report` needs a selected profile and a DMI, `apply --dry-run` prints a plan but exits 0 even
for a profile with no `[requires]` at all — see retest item 2). Nothing in the package enforces
the "documented profile carries no `mirror/`, no `[co]`" rule at runtime.

## Evidence

```
$ grep -n "Adding a profile" /usr/share/doc/genarch-settings/PROFILES.md
123:Adding a profile is therefore: write the directory, run `cargo test --test profiles`, fix what it
124:says. It fails on a *plausible* profile as well as a broken one, which is the point.
$ command -v cargo || echo "cargo: not installed"
cargo: not installed
$ genarch-settings --help | sed -n '/Commands:/,/Options:/p'
Commands:
  report       Judge the live system against intent. Read-only; runs unprivileged
  apply        Converge the live system to intent. Needs root; idempotent
  harvest      Print the intent change that would match live. Writes nothing
  units        Print the units the intent expects enabled, one per line
  converge     Run a scheduled pass. Called by systemd units and udev rules, not by hand
  completions  Print a completion script for one shell on stdout
  import       Derive a machine profile from a legacy `state/` tree (migration only)
  help         Print this message or the help of the given subcommand(s)
```

## What could not be determined

Whether the profile gate also rejects a `status = "measured"` profile that ships `mirror/`
files (it must, by the text) — untested, because the gate is not on this machine.
