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
applied — the writes landed against another machine's facts, and that is the drift to fix.
`--dry-run` always exits 0: it is a preview, and an installer runs it under `set -e`. `apply` writes a dated pre-image beside every file it replaces, keeps the source mode
and owner, and refuses on any target it cannot classify.

The report is eleven rows. `--json` is schema 2: a top-level `profile` object (`null` when none
is selected), then `checks` and `summary`. The `APPLY-SCOPE` line says which rows `apply` can
reach (`full`, `file-only`), which are judged against the intent alone (`intent`), and which it
cannot touch (`none`).

## Install

On Arch, with the recipe next to the `gsa` recipe set:

```sh
makepkg -f && sudo pacman -U genarch-0.2.2-1-x86_64.pkg.tar.zst
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

## Coverage

| Profile | Machine | Kind | What it judges |
|---|---|---|---|
| `zenbook-s16` | ASUS Zenbook S 16 UM5606WA (Ryzen AI 9 HX 370) | measured | everything — this host |
| `framework-13-amd` | Framework Laptop 13, Ryzen 7040 (7640U / 7840U) | documented | `PROFILE`, `CEIL` (4.9 / 5.1 GHz bands) |
| `framework-16-amd` | Framework Laptop 16, Ryzen 7040 (7840HS / 7940HS) | documented | `PROFILE`, `CEIL` (5.1 / 5.2 GHz bands) |
| `elitebook-845-g11` | HP EliteBook 845 14" G11 (8840U/HS, 8640HS, 8440U) | documented | `PROFILE`, `CEIL` (three SKU bands) |
| `zenbook-14-um3406` | ASUS Zenbook 14 OLED UM3406HA (8840HS) | documented | `PROFILE`, `CEIL` (5.1 GHz) |
| `zephyrus-g14-2024-ga403ui` | ASUS ROG Zephyrus G14 2024 GA403UI (8945HS) | documented | `PROFILE`, `CEIL` (5.2 GHz) |
| `zephyrus-g14-2024-ga403uv` | ASUS ROG Zephyrus G14 2024 GA403UV (8945HS) | documented | `PROFILE`, `CEIL` (5.2 GHz) |

A **documented** profile carries only values a publication states, so it ships no file, sets no
curve offset and declares no power envelope — `CO`, `POWER`, `SMU` and `MIRROR` have nothing to
judge and say so. That is the honest shape of coverage from documentation alone:
`docs/PROFILES.md` explains the three provenance classes and what a machine needs before it can
have a profile.

**Deliberately not covered:** Lenovo ThinkPad Z13 Gen 2. Its DMI `product_name` is the exact MTM
(`21JVCTO1WW`), and PSREF lists 1,276 MTMs with two different CPUs, so exact matching cannot
express the model — see "When a machine cannot be covered" in `docs/PROFILES.md`.

## Licence

MIT — see `LICENSE`. `SCOPE.md` says what belongs here; `CONTRIBUTING.md` says how a change is
judged (there is no CI — the gate is a local command).

## Documentation

- `AGENTS.md` — architecture rules and the pitfalls that paid for them
- `CONTEXT.md` — domain vocabulary (knob, intent, layer, profile, verdict, drift, apply, harvest)
- `docs/DESIGN.md` — why the seam is where it is, and what is deliberately open
- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
- `SCOPE.md` / `CONTRIBUTING.md` — what belongs here, and the gate a change must pass
