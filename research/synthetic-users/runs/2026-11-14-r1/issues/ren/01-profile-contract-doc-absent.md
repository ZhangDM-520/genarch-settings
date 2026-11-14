# The profile/layer contract a porter is told to follow (`docs/PROFILES.md`) is not installed, and the shipped docs point at paths that do not exist

- **severity:** high
- **category:** doc
- **environment:** Arch Linux guest (QEMU, 2 vCPU, kernel 7.2.11-arch1-1), `genarch-settings 0.4.0-1`, `ryzenadj 0.19.0-1`. Engine runs use a copy of the staging tree (`GENARCH_SETTINGS_ROOT=~/machine/root`) unless a path says otherwise. The guest has no real SMU, no wireless interface and no cpufreq outside the staging tree.

## Intent

Port the engine to a second machine model using only the files the package installs: read the
documented profile contract, write `profiles/<name>/`, select it with one line, then use the
layer overrides (one value, one tombstone).

## Steps

```sh
ls -la /usr/share/doc/genarch-settings/
ls /usr/share/doc/genarch-settings/docs
find / -xdev \( -name PROFILES.md -o -name findings.md \) 2>/dev/null
grep -rn "docs/" /usr/share/doc/genarch-settings/*.md
grep -rn "PROFILES.md" /usr/lib/genarch-settings/profiles/*/intent.toml
grep -n -A8 "installing documentation" /usr/share/doc/genarch-settings/install.sh
```

Tombstone syntax probe (operator layer over a converged staging copy; see the last section):

```sh
cp -a ~/machine/root ~/ev9
O=~/ev9/etc/genarch-settings
printf '[omit]\nmirror = ["etc/tmpfiles.d/50-eco-thp.conf"]\n' > $O/intent.toml
GENARCH_SETTINGS_ROOT=~/ev9 genarch-settings report
printf '[omit]\nmirror = ["/etc/tmpfiles.d/50-eco-thp.conf"]\n' > $O/intent.toml
GENARCH_SETTINGS_ROOT=~/ev9 genarch-settings report
printf '[omit]\nmanaged = ["/etc/systemd/journald.conf"]\n' > $O/intent.toml
GENARCH_SETTINGS_ROOT=~/ev9 genarch-settings report
printf '[omit]\nmanaged = ["journald"]\n' > $O/intent.toml
GENARCH_SETTINGS_ROOT=~/ev9 genarch-settings report
```

## Expected

The package tells the porter that the profile contract (what may ship in a profile, provenance,
and the operator overlay) is in a file, and that a second machine is a directory plus one line:

- `SCOPE.md`: "**Second machine** — a new directory under `profiles/` plus one line in `/etc/genarch-settings/profile`."
- `CONTEXT.md`: "It does not carry one unit's own identity; what may ship in a profile, and where the rest goes, is `docs/PROFILES.md`."
- `CONTEXT.md`: "A profile also states how it came to be: **documented** ... or **measured** ... `docs/PROFILES.md` is the contract."
- `CONTEXT.md`: "mirrors and managed records are whole-file with an explicit `omit = [...]` tombstone, which reaches only the layers below it."
- `install.sh` usage: "Profiles live in <intent-dir>/profiles/NAME and declare the machine they are for ([requires])."

A guide is followable only with the files the package actually installs.

## Actual

`docs/PROFILES.md` is installed nowhere, `docs/` does not exist, `DESIGN.md` is installed at the
top level while two documents send the reader to `docs/DESIGN.md`, and every report verdict points
at a `findings#…` anchor in a document that is not on the machine. `install.sh`'s documentation
loop does not include `PROFILES.md` even though it copies `docs/DESIGN.md` from the source tree.

For the tombstone, the only statement is the prose above: no shipped file gives the key. The key
is a **live path** for `mirror` and a **record name** for `managed`; both wrong forms fail with
"No layer below it ships/declares", which does not say what the key should be. I found the two
forms by trial (three attempts for `mirror`, two for `managed`).

## Evidence

```
COMMAND: ls -la /usr/share/doc/genarch-settings/
total 60
drwxr-xr-x 1 root root   136 Nov 14 14:34 .
drwxr-xr-x 1 root root   660 Nov 14 14:34 ..
-rw-r--r-- 1 root root 15530 Nov 14 14:29 AGENTS.md
-rw-r--r-- 1 root root  4710 Nov 14 14:29 CONTEXT.md
-rw-r--r-- 1 root root  1982 Nov 14 14:29 CONTRIBUTING.md
-rw-r--r-- 1 root root 11692 Nov 14 14:29 DESIGN.md
-rw-r--r-- 1 root root  1068 Nov 14 14:29 LICENSE
-rw-r--r-- 1 root root   876 Nov 14 14:29 SCOPE.md
-rwxr-xr-x 1 root root 10804 Nov 14 14:29 install.sh

COMMAND: ls /usr/share/doc/genarch-settings/docs
ls: cannot access '/usr/share/doc/genarch-settings/docs': No such file or directory

COMMAND: find / -xdev \( -name PROFILES.md -o -name findings.md \) 2>/dev/null
(no output)

COMMAND: grep -rn "docs/" /usr/share/doc/genarch-settings/*.md
/usr/share/doc/genarch-settings/AGENTS.md:12:Vocabulary: `CONTEXT.md`. Interface reasoning: `docs/DESIGN.md`. Knob judgement and evidence live
/usr/share/doc/genarch-settings/AGENTS.md:13:in `system-tuning/docs/findings.md` — this repository does not restate a verdict, and it does not
/usr/share/doc/genarch-settings/AGENTS.md:29:| `cargo test --test profiles` | the shipped-profile gate: every profile loads and keeps its provenance contract (`
/usr/share/doc/genarch-settings/CONTEXT.md:32:ship in a profile, and where the rest goes, is `docs/PROFILES.md`. The base carries only what is
/usr/share/doc/genarch-settings/CONTEXT.md:39:published. `docs/PROFILES.md` is the contract.
/usr/share/doc/genarch-settings/CONTEXT.md:61:`ceil`. The split is a safety property, not an optimisation (see `docs/DESIGN.md`).
/usr/share/doc/genarch-settings/CONTRIBUTING.md:14:test reads `system-tuning/docs/findings.md` through `SYSTEM_TUNING_ROOT` (default
/usr/share/doc/genarch-settings/SCOPE.md:8:  (`system-tuning/docs/findings.md`), the benchmarks, and any second engine writing these paths.

COMMAND: grep -rn "PROFILES.md" /usr/lib/genarch-settings/profiles/*/intent.toml
/usr/lib/genarch-settings/profiles/elitebook-845-g11/intent.toml:7:# Documented, not measured (`docs/PROFILES.md`).
/usr/lib/genarch-settings/profiles/framework-13-amd/intent.toml:12:# Documented, not measured (`docs/PROFILES.md`).
/usr/lib/genarch-settings/profiles/framework-16-amd/intent.toml:9:# guessed, because none of those can be published. `docs/PROFILES.md` is the contract.
/usr/lib/genarch-settings/profiles/zenbook-14-um3406/intent.toml:7:# Documented, not measured (`docs/PROFILES.md`).
/usr/lib/genarch-settings/profiles/zephyrus-g14-2024-ga403ui/intent.toml:11:# Documented, not measured (`docs/PROFILES.md`).
/usr/lib/genarch-settings/profiles/zephyrus-g14-2024-ga403uu/intent.toml:11:# Documented, not measured (`docs/PROFILES.md`).
/usr/lib/genarch-settings/profiles/zephyrus-g14-2024-ga403uv/intent.toml:11:# Documented, not measured (`docs/PROFILES.md`).

COMMAND: grep -n -A8 "installing documentation" /usr/share/doc/genarch-settings/install.sh
192:say "installing documentation to $DOC_DIR"
193-for doc in AGENTS.md CONTEXT.md SCOPE.md CONTRIBUTING.md LICENSE; do
194-  run install -Dm644 "$SOURCE/$doc" "$DOC_DIR/$doc"
195-done
196-run install -Dm644 "$SOURCE/docs/DESIGN.md" "$DOC_DIR/DESIGN.md"
197-run install -Dm755 -- "$0" "$DOC_DIR/install.sh"
```

Tombstone probe (the successful forms are the last `mirror` and the last `managed`):

```
COMMAND: [omit] mirror = ["etc/tmpfiles.d/50-eco-thp.conf"]
genarch-settings: /home/arch/ev9/etc/genarch-settings: [omit] mirror names etc/tmpfiles.d/50-eco-thp.conf, which no layer below it ships
COMMAND: [omit] mirror = ["50-eco-thp.conf"]
genarch-settings: /home/arch/ev9/etc/genarch-settings: [omit] mirror names 50-eco-thp.conf, which no layer below it ships
COMMAND: [omit] mirror = ["/etc/tmpfiles.d/50-eco-thp.conf"]
OK MIRROR 38 mirror target(s) byte-equal findings#intent
COMMAND: [omit] managed = ["/etc/systemd/journald.conf"]
genarch-settings: /home/arch/ev9/etc/genarch-settings: [omit] managed names '/etc/systemd/journald.conf', which no layer below it declares
COMMAND: [omit] managed = ["journald"]
OK MANAGED 1 record(s) hold their line findings#managed
```

## What could not be determined

- Whether the judgement ledger (`system-tuning/docs/findings.md`) is deliberately distributed
  outside the package. `SCOPE.md` puts it out of scope, but every `report` row still prints a
  `findings#…` anchor, so on a machine with only this package no verdict can be traced.
- Whether `docs/PROFILES.md` exists in the source tree (there is no source on this machine); only
  that neither the package nor `install.sh` puts it on a host.
