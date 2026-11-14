# README points at `docs/CUTOVER.md`, which the package does not install

**severity:** medium
**category:** doc
**environment:** Arch Linux, genarch-settings 0.4.0-1 installed from `~/payload`

## Intent

Find the rollback runbook while the package is installed, not only from a git checkout.

## Steps

```sh
ls /usr/share/doc/genarch-settings/
grep -n CUTOVER /usr/share/doc/genarch-settings/README.md
pacman -Ql genarch-settings | grep -i cutover
```

## Expected

The shipped material promises the document beside the binary:

> `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback

> The package and `install.sh` install these flat under `/usr/share/doc/genarch-settings/`, where
> `docs/PROFILES.md` becomes `PROFILES.md`.

## Actual

No `CUTOVER.md` in the installed documents, and the package file list has no such path. The
README's Documentation section is the only pointer, and it leads nowhere. The rollback text is
exactly what an operator needs when the apply path misbehaves (`issues/partial-apply-reports-applied-zero.md`).

## Evidence

```
$ ls /usr/share/doc/genarch-settings/
AGENTS.md
CONTEXT.md
CONTRIBUTING.md
DESIGN.md
LICENSE
PROFILES.md
README.md
SCOPE.md
install.sh
$ grep -n CUTOVER /usr/share/doc/genarch-settings/README.md
143:- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
$ pacman -Ql genarch-settings | grep -i cutover
$
```

## What could not be determined

Whether the file exists in the source repository and was left out of the package on purpose.
The README also lists `docs/DESIGN.md`, which is installed as `DESIGN.md`, so this is not the
flat-rename rule.
