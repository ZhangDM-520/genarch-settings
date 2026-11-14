# Installed documentation links a `docs/` tree and a rollback runbook that were not installed, and `install.sh` cites a PKGBUILD that is not shipped

**Severity:** low
**Category:** doc
**Environment:** genarch-settings 0.4.0-1 from the payload package; docs at `/usr/share/doc/genarch-settings/`.

## Intent

Read the installed documentation to learn how to install, roll back and add a machine, offline, on
a host with no source checkout.

## Steps

```
ls /usr/share/doc/genarch-settings
ls /usr/share/doc/genarch-settings/docs
grep -n '](docs/' /usr/share/doc/genarch-settings/README.md
grep -n 'docs/CUTOVER.md' /usr/share/doc/genarch-settings/README.md
grep -n PKGBUILD /usr/share/doc/genarch-settings/install.sh
```

## Expected

The README closes with:

> The package and `install.sh` install these flat under `/usr/share/doc/genarch-settings/`, where
> `docs/PROFILES.md` becomes `PROFILES.md`.

and its Documentation list includes:

> - `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback

`install.sh` says:

> on Arch the PKGBUILD beside this file is the supported path.

## Actual

- There is no `docs/` directory, so both README links (`docs/PROFILES.md`) are dead relative links.
  The closing paragraph explains the rename but does not repair the links.
- `CUTOVER.md` is not installed under any name, although it is the document promised for rollback —
  the one thing this operator needs after a bad rollout.
- No `PKGBUILD` is installed beside `install.sh`, so the "supported path" it points at cannot be
  read on the box.

## Evidence

```
$ ls /usr/share/doc/genarch-settings
AGENTS.md  CONTEXT.md  CONTRIBUTING.md  DESIGN.md  LICENSE  PROFILES.md  README.md  SCOPE.md  install.sh
$ ls /usr/share/doc/genarch-settings/docs
ls: cannot access '/usr/share/doc/genarch-settings/docs': No such file or directory
$ grep -n '](docs/' /usr/share/doc/genarch-settings/README.md
9:instead, is [`docs/PROFILES.md`](docs/PROFILES.md).
82:belong instead, is [`docs/PROFILES.md`](docs/PROFILES.md). Without pacman — from this
$ grep -n 'docs/CUTOVER.md' /usr/share/doc/genarch-settings/README.md
143:- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
$ test -e /usr/share/doc/genarch-settings/CUTOVER.md && echo present || echo missing
missing
$ grep -n PKGBUILD /usr/share/doc/genarch-settings/install.sh
6:# binary outside the package manager; on Arch the PKGBUILD beside this file is the supported
$ ls /usr/share/doc/genarch-settings/PKGBUILD
ls: cannot access '/usr/share/doc/genarch-settings/PKGBUILD': No such file or directory
```

## What could not be determined

- Whether `CUTOVER.md` is deliberately withheld from the package (it names a host-specific
  cutover) or simply missing from the packaging file list. The README lists it as shipped
  documentation, so as installed the document is wrong either way.
- Whether the README is expected to be read from the source checkout rather than the installed
  copy. The package installs it, so the installed copy is the one an operator has.
