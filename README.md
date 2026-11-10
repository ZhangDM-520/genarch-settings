# genarch

The tuning engine for one Arch host (ASUS Zenbook S 16 UM5606WA, Ryzen AI 9 HX 370, CachyOS).
One Rust binary carries the declared intent, a read-only judge, the converge pass, the ryzenadj
settings and the systemd units.

It is the second generation of a shell tool (`eco`, still present in
[`system-tuning`](https://github.com/ZhangDM-520/system-tuning) as the rollback artifact). The
ledger — why every knob has its value — stays in that repository; this one carries the mechanism.

## What it does

```
genarch report      [--json]     read-only verdicts; unprivileged OK
genarch apply       [--dry-run]  converge live to intent; root
genarch harvest                  proposal: the intent change that would match live
genarch converge    <boot|resume|ceil>   the scheduled passes
```

Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3`
partial. `apply` writes a dated pre-image beside every file it replaces, keeps the source mode
and owner, and refuses on any target it cannot classify.

## Install

On Arch, with the recipe next to the `gsa` recipe set:

```sh
makepkg -f && sudo pacman -U genarch-*.pkg.tar.zst
```

Without pacman — from this checkout or straight from a tagged tarball:

```sh
sudo ./install.sh            # builds, installs, converges, enables the units
./install.sh --help
```

The engine reads its intent from `/usr/lib/genarch` and records what it owns in
`/var/lib/genarch/manifest.toml`.

## Documentation

- `AGENTS.md` — architecture rules and the pitfalls that paid for them
- `CONTEXT.md` — domain vocabulary (knob, intent, verdict, drift, apply, harvest, pass)
- `docs/DESIGN.md` — why the seam is where it is, and what is deliberately open
- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
