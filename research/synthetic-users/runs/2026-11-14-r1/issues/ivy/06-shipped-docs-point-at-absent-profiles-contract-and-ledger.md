# Shipped docs point at docs/PROFILES.md, the findings ledger and a PKGBUILD that are not in the package

**Severity:** medium
**Category:** doc

**Environment:** Arch guest; `genarch-settings 0.4.0-1`. Installed docs are under
`/usr/share/doc/genarch-settings/`.

**Intent:** Add a profile for this headless box. The shipped text says a second machine is "a
new directory under `profiles/` plus one line", and that `docs/PROFILES.md` is the contract for
what may ship in a profile. I need that contract to write one; I also want to resolve the
`findings#...` anchor printed on every report row.

**Steps:**

```sh
ls /usr/share/doc/genarch-settings/
grep -n 'PROFILES.md' /usr/share/doc/genarch-settings/*.md
pacman -Ql genarch-settings | grep -F 'PROFILES.md'
genarch-settings report
/usr/share/doc/genarch-settings/install.sh --dry-run
```

**Expected:** CONTEXT.md:

> It does not carry one unit's own identity; what may ship in a profile, and where the rest
> goes, is `docs/PROFILES.md`.
>
> ... `docs/PROFILES.md` is the contract.

AGENTS.md:

> `cargo test --test profiles` | the shipped-profile gate: every profile loads and keeps its
> provenance contract (`docs/PROFILES.md`)

and on the report surface, each row names a doc anchor (`findings#intent`, `findings#managed`,
...). `install.sh` says:

> on Arch the PKGBUILD beside this file is the supported path.

**Actual:** The documentation directory has no `docs/` subtree, no `PROFILES.md` and no
`findings.md`; the package contains no file matching `PROFILES.md`; and there is no PKGBUILD
beside the installed `install.sh` (only the package files and the binary are available, and
`install.sh` needs a source tree or a GitHub download plus `cargo`). The report's `doc` fields
are dead pointers on the installed system.

**Evidence:**

```
$ ls /usr/share/doc/genarch-settings/
AGENTS.md
CONTEXT.md
CONTRIBUTING.md
DESIGN.md
LICENSE
SCOPE.md
install.sh

$ grep -n 'PROFILES.md' /usr/share/doc/genarch-settings/*.md
/usr/share/doc/genarch-settings/AGENTS.md:29:| `cargo test --test profiles` | the shipped-profile gate: every profile loads and keeps its provenance contract (`docs/PROFILES.md`) |
/usr/share/doc/genarch-settings/AGENTS.md:112:| A **documented** profile carries no `mirror/`, ... `docs/PROFILES.md` is the contract. |
/usr/share/doc/genarch-settings/CONTEXT.md:32:ship in a profile, and where the rest goes, is `docs/PROFILES.md`. The base carries only what is
/usr/share/doc/genarch-settings/CONTEXT.md:39:published. `docs/PROFILES.md` is the contract.

$ pacman -Ql genarch-settings | grep -F 'PROFILES.md'
(no output)

$ genarch-settings report
DRIFT PROFILE ... findings#intent
DRIFT MIRROR ... findings#intent
...
OK CO intent declares coall -5 + 8 per-core offset(s) findings#co
```

Every row ends in a `findings#...` anchor; `findings.md` is not installed.

The installer, run from the installed copy:

```
$ /usr/share/doc/genarch-settings/install.sh --dry-run
genarch-settings-install: fetching https://github.com/ZhangDM-520/genarch-settings/archive/v0.4.0.tar.gz
genarch-settings-install: cargo is required to build (install the rust toolchain)
EXIT=1
```

It is not usable from the package alone, and the PKGBUILD it calls the supported path is absent.

**Why it matters:** the profile contract is the one document a second-machine operator needs
("A second machine is a profile directory plus one selection line"), and it is the document
that says which fields a profile may carry, whether `status` is mandatory, and what a
documented profile may ship. I had to infer the contract by provoking loader errors and
copying the rehearsal profile. The per-row anchors are the report's explanation surface; on
the installed system they resolve to nothing, so "months later" the report explains itself
with a pointer I cannot follow.

**What I could not determine:** whether the ledger is deliberately out-of-package (the
`system-tuning` repository is out of scope in SCOPE.md). If so, the report should not print an
unresolvable anchor, or the package should ship a short legend. Whether `docs/PROFILES.md` is
simply missing from the package file list or was never written.
