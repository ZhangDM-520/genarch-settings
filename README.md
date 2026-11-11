# genarch

One Rust binary carries the declared intent, a read-only judge, the converge pass, the ryzenadj
settings and the systemd units. The intent is layered: a host-agnostic **base**, a machine
**profile**, and an operator override. This repository is the engine plus this host's profile
(ASUS Zenbook S 16 UM5606WA, Ryzen AI 9 HX 370); the ledger of *why* every knob has its value
stays in [`system-tuning`](https://github.com/ZhangDM-520/system-tuning), and `eco` there is the
rollback artifact.

A second AMD laptop is a directory under `intent/profiles/` and one line in
`/etc/genarch/profile` — the base carries no disk UUID, no user path and no silicon measurement.
A profile declares the machine it is for, and `report` says `PROFILE DRIFT` (exit 1) when it is
applied to another one.

## What it does

```
genarch report      [--json]     read-only verdicts; unprivileged OK
genarch apply       [--dry-run]  converge live to intent; root
genarch harvest                  proposal: the intent change that would match live
genarch units                    the units the intent expects enabled, one per line
genarch converge    <boot|resume|ceil>   the scheduled passes
genarch import      <state> <profile-dir>  derive a profile from a legacy shell state tree
```

Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3`
partial. `apply` exits 1 when the selected profile does not match the machine, even though it
applied — the writes landed against another machine's facts, and that is the drift to fix. `apply` writes a dated pre-image beside every file it replaces, keeps the source mode
and owner, and refuses on any target it cannot classify.

The report is eleven rows. `--json` is schema 2: a top-level `profile` object (`null` when none
is selected), then `checks` and `summary`. The `APPLY-SCOPE` line says which rows `apply` can
reach (`full`, `file-only`), which are judged against the intent alone (`intent`), and which it
cannot touch (`none`).

## Install

On Arch, with the recipe next to the `gsa` recipe set:

```sh
makepkg -f && sudo pacman -U genarch-0.2.0-1-x86_64.pkg.tar.zst
```

The package ships this host's profile **and** its selection (`/etc/genarch/profile`, a pacman
backup file). Without pacman — from this checkout or straight from a tagged tarball:

```sh
sudo ./install.sh                    # builds, installs, converges, enables the units
sudo ./install.sh --profile zenbook-s16   # also select this machine's profile
./install.sh --help
```

`install.sh` is the portable path: it installs the base and every profile but selects none, so a
fresh host runs the base intent and reports `PROFILE SKIPPED` until an operator writes
`profile = "<name>"` into `/etc/genarch/profile`.

The engine reads its intent from `/usr/lib/genarch` (base + `profiles/<name>`), the operator
layer from `/etc/genarch`, and records what it owns in `/var/lib/genarch/manifest.toml`. Nothing
is pruned unless the manifest says the engine wrote it, and a path belonging to a profile that is
merely not selected stays owned.

## Licence

MIT — see `LICENSE`. `SCOPE.md` says what belongs here; `CONTRIBUTING.md` says how a change is
judged (there is no CI — the gate is a local command).

## Documentation

- `AGENTS.md` — architecture rules and the pitfalls that paid for them
- `CONTEXT.md` — domain vocabulary (knob, intent, layer, profile, verdict, drift, apply, harvest)
- `docs/DESIGN.md` — why the seam is where it is, and what is deliberately open
- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
- `SCOPE.md` / `CONTRIBUTING.md` — what belongs here, and the gate a change must pass
