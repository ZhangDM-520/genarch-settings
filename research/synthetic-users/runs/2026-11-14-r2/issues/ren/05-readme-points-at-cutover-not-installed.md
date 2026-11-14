# README's Documentation list points at `docs/CUTOVER.md`, which the package does not install

- **Severity:** low
- **Category:** doc
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1

## Intent

Follow the installed documentation from the README to the referenced pages; the README itself
says the package installs them flat under `/usr/share/doc/genarch-settings/`.

## Steps

```sh
sed -n '/## Documentation/,/^## /p' /usr/share/doc/genarch-settings/README.md
pacman -Ql genarch-settings | grep /usr/share/doc
```

## Expected

Every page the README lists exists, and links that point at `docs/<page>.md` resolve to the flat
copy the same README describes.

## Actual

`docs/CUTOVER.md` is listed but not installed at all; the inline links (`docs/PROFILES.md`,
`docs/DESIGN.md`) do not resolve from `/usr/share/doc/genarch-settings/README.md` because the
pages are flat there. The final paragraph explains the flattening, but the links above it are
still dead, and CUTOVER is simply absent.

## Evidence

```
$ sed -n '/## Documentation/,$p' /usr/share/doc/genarch-settings/README.md
## Documentation

- `AGENTS.md` — architecture rules and the pitfalls that paid for them
- `CONTEXT.md` — domain vocabulary (knob, intent, layer, profile, verdict, drift, apply, harvest)
- `docs/PROFILES.md` — what a profile may ship, provenance classes, and how to add a machine
- `docs/DESIGN.md` — why the seam is where it is, and what is deliberately open
- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
- `SCOPE.md` / `CONTRIBUTING.md` — what belongs here, and the gate a change must pass

The package and `install.sh` install these flat under `/usr/share/doc/genarch-settings/`, where
`docs/PROFILES.md` becomes `PROFILES.md`.

$ pacman -Ql genarch-settings | grep /usr/share/doc
genarch-settings /usr/share/doc/genarch-settings/AGENTS.md
genarch-settings /usr/share/doc/genarch-settings/CONTEXT.md
genarch-settings /usr/share/doc/genarch-settings/CONTRIBUTING.md
genarch-settings /usr/share/doc/genarch-settings/DESIGN.md
genarch-settings /usr/share/doc/genarch-settings/LICENSE
genarch-settings /usr/share/doc/genarch-settings/PROFILES.md
genarch-settings /usr/share/doc/genarch-settings/README.md
genarch-settings /usr/share/doc/genarch-settings/SCOPE.md
genarch-settings /usr/share/doc/genarch-settings/install.sh
```

## What could not be determined

Whether CUTOVER is deliberately excluded from the package; `README.md` says it is installed.
