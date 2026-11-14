# genarch-settings

One Rust binary carries the declared intent, a read-only judge, the converge pass, the ryzenadj
settings and the systemd units. The intent is layered: a host-agnostic **base**, a machine
**profile**, and an operator override. This repository is the engine plus a profile for this laptop
model (ASUS Zenbook S 16 UM5606WA, Ryzen AI 9 HX 370); the ledger of *why* every knob has its
value stays in [`system-tuning`](https://github.com/ZhangDM-520/system-tuning), and `eco` there is
the rollback artifact. What a profile may ship, and where a value unique to one unit belongs
instead, is [`docs/PROFILES.md`](docs/PROFILES.md).

A second AMD laptop is a directory under `intent/profiles/` and one line in
`/etc/genarch-settings/profile`. A profile declares the machine it is for, and `report` says
`PROFILE DRIFT` (exit 1) when it is applied to another one.

## What it does

```
genarch-settings report      [--json]     read-only verdicts; unprivileged OK
genarch-settings apply       [--dry-run]  converge live to intent; root
genarch-settings harvest                  proposal: the intent change that would match live
genarch-settings units                    the units the intent expects enabled, one per line
genarch-settings converge    <boot|resume|ceil>   the scheduled passes
genarch-settings import      <state> <profile-dir>  derive a profile from a legacy shell state tree
genarch-settings completions <bash|zsh|fish>   print a completion script on stdout
```

Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3`
partial. `apply` exits 1 when the selected profile does not match the machine, even though it
applied — the writes landed against another machine's facts, and that is the drift to fix.
`apply`'s `0` means the plan it can reach was applied: rows whose `APPLY-SCOPE` is `none`
(`CEIL`, `TIMER`) are not part of that plan, so `report` may still show drift — a green `apply`
never implies a green report. `--dry-run` always exits 0: it is a preview, and an installer runs
it under `set -e`. It prints the refusal when the intent cannot be applied, but it writes
nothing and never fails the run. `apply` writes a dated pre-image beside every file it replaces,
keeps the source mode and owner, and refuses on any target it cannot classify.

The report is eleven rows. `--json` is schema 2: a top-level `profile` object (`null` when none
is selected), then `checks` and `summary`. The `APPLY-SCOPE` line says which rows `apply` can
reach (`full`, `file-only`), which are judged against the intent alone (`intent`), and which it
cannot touch (`none`). The last column of each row is a `findings#…` anchor into the judgement
ledger ([`system-tuning`](https://github.com/ZhangDM-520/system-tuning)), which records *why* a
value is what it is; the ledger is not shipped with the package, so the anchor names a section,
not a file on this host.

A profile whose `[requires]` do not match the machine is `PROFILE` drift. The plan is still
computed and printed, but if a record it carries cannot be classified — an absent managed
target, such as `asusd.ron` on a machine without the ASUS tooling — the whole apply is refused
with "nothing was written" and exit 3. The drift to fix is the selection.

`GENARCH_SETTINGS_ROOT=/path/to/copy` aims every live path at a copy for a rehearsal. It does
not execute external tools: a rehearsal never writes this machine's SMU and never reloads its
systemd or udev.

## Shell completions

The package and `install.sh` write the three standard files, generated from the binary's own
command tree, so a script can never describe a verb the binary does not have:

```
/usr/share/bash-completion/completions/genarch-settings
/usr/share/zsh/site-functions/_genarch-settings
/usr/share/fish/vendor_completions.d/genarch-settings.fish
```

On a host without either installer:

```sh
genarch-settings completions fish | source       # this shell only
genarch-settings completions fish > ~/.config/fish/completions/genarch-settings.fish
```

## Install

On Arch, with the recipe next to the `gsa` recipe set:

```sh
makepkg -f && sudo pacman -U genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
```

The package depends on `ryzenadj`, which Arch packages in the **AUR**, not the official
repositories: install it first (`yay -S ryzenadj` or equivalent) or `pacman -U` will refuse the
transaction with an unsatisfied dependency. `install.sh` checks for it and says so.

The package ships the `zenbook-s16` profile **and** its selection (`/etc/genarch-settings/profile`,
a pacman backup file). What that profile may and may not contain, and where a unit's own facts
belong instead, is [`docs/PROFILES.md`](docs/PROFILES.md). Without pacman — from this
checkout or straight from a tagged tarball:

```sh
sudo ./install.sh                    # builds, installs, converges, enables the units
sudo ./install.sh --profile zenbook-s16   # also select this machine's profile
./install.sh --help
```

`install.sh` is the portable path: it installs the base and every profile but selects none, so a
fresh host runs the base intent and reports `PROFILE SKIPPED` until an operator writes
`profile = "<name>"` into `/etc/genarch-settings/profile`.

The engine reads its intent from `/usr/lib/genarch-settings` (base + `profiles/<name>`), the operator
layer from `/etc/genarch-settings`, and records what it owns in `/var/lib/genarch-settings/manifest.toml`. Nothing
is pruned unless the manifest says the engine wrote it, and a path a deselected profile had
already written stays owned.

## Removal

`pacman -R genarch-settings` disables the units and removes the binary, the intent and the
documents. It does **not** remove what the engine deployed: every file listed in
`/var/lib/genarch-settings/manifest.toml` stays on disk, with a `<file>.bak-YYYYMMDD` pre-image
beside anything it replaced, and a managed record leaves the line it patched in place. Read the
manifest before removing; if the removal is permanent, delete those paths yourself. A reinstall
adopts the same manifest.

## Coverage

| Profile | Machine | Kind | What it judges |
|---|---|---|---|
| `zenbook-s16` | ASUS Zenbook S 16 UM5606WA (Ryzen AI 9 HX 370) | measured | the model: `PROFILE`, `CEIL`, `CO`, `SMU`, `POWER`, `MIRROR` (one device rule) |
| `framework-13-amd` | Framework Laptop 13, Ryzen 7040 (7640U / 7840U) | documented | `PROFILE`, `CEIL` (4.9 / 5.1 GHz bands) |
| `framework-16-amd` | Framework Laptop 16, Ryzen 7040 (7840HS / 7940HS) | documented | `PROFILE`, `CEIL` (5.1 / 5.2 GHz bands) |
| `elitebook-845-g11` | HP EliteBook 845 14" G11 (8840U/HS, 8640HS, 8440U) | documented | `PROFILE`, `CEIL` (three SKU bands) |
| `zenbook-14-um3406` | ASUS Zenbook 14 OLED UM3406HA (8840HS) | documented | `PROFILE`, `CEIL` (5.1 GHz) |
| `zephyrus-g14-2024-ga403ui` | ASUS ROG Zephyrus G14 2024 GA403UI (8945HS / 8845HS) | documented | `PROFILE`, `CEIL` (5.2 / 5.1 GHz) |
| `zephyrus-g14-2024-ga403uv` | ASUS ROG Zephyrus G14 2024 GA403UV (8945HS / 8845HS) | documented | `PROFILE`, `CEIL` (5.2 / 5.1 GHz) |
| `zephyrus-g14-2024-ga403uu` | ASUS ROG Zephyrus G14 2024 GA403UU (8945HS / 8845HS) | documented | `PROFILE`, `CEIL` (5.2 / 5.1 GHz) |

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
- `docs/PROFILES.md` — what a profile may ship, provenance classes, and how to add a machine
- `docs/DESIGN.md` — why the seam is where it is, and what is deliberately open
- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
- `SCOPE.md` / `CONTRIBUTING.md` — what belongs here, and the gate a change must pass

The package and `install.sh` install these flat under `/usr/share/doc/genarch-settings/`, where
`docs/PROFILES.md` becomes `PROFILES.md`. Every `docs/…` link in this README therefore names the
file's basename on an installed host.
