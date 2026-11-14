# `harvest` prints nothing for sysctl drift, although it is the verb that proposes intent changes

**Severity:** Medium
**Category:** bug
**Environment:** Arch guest, rehearsal root `~/ev4` (a converged copy with two sysctls changed
by hand).

## Intent

`harvest` is the migration/settle verb. Its own help:

> `Print the intent change that would match live. Writes nothing`

`CONTRIBUTING.md`:

> `harvest is the only verb that proposes intent changes; it never writes.`

An operator who has accepted a live value (or is importing a box) runs `harvest` to get the
TOML change. A silent empty answer is read as "live already matches intent".

## Steps

```sh
R=~/ev4; rm -rf $R; cp -a ~/r14 $R
printf 60 > $R/proc/sys/vm/swappiness
printf 42 > $R/proc/sys/kernel/sysrq
GENARCH_SETTINGS_ROOT=$R genarch-settings report | grep SYNC
GENARCH_SETTINGS_ROOT=$R genarch-settings harvest; echo "EXIT=$?"
```

## Expected

Output proposing the intent change that would match live, e.g. the two `[values]` entries
`vm.swappiness = "60"` and `kernel.sysrq = "42"` (as the mirror and managed classes are printed
for their own drift).

## Actual

`report` says SYNC is drifted on exactly those two keys; `harvest` prints nothing and exits 0.
The same omission happens with every sysctl key, the class the engine otherwise treats as its
best-understood data. (`harvest` does print mirror and managed differences in the same tree.)

## Evidence (verbatim)

```
--- report:
DRIFT SYNC kernel.sysrq=42 want 190 vm.swappiness=60 want 150 findings#writeback
--- harvest (stdout+stderr):
EXIT=0
--- harvest help:
Print the intent change that would match live. Writes nothing
```

Contrast (excerpt: the mirror body is 30 lines): a modified mirror and a managed line are
printed by the same binary in the same root (`~/r16`):

```
--- intent: /etc/tlp.d/01-eco.conf (live differs)
+ # Eco-first policy — identical on AC and battery …
--- managed journald in /etc/systemd/journald.conf
- SystemMaxUse=1G
+ SystemMaxUse=256M
```

## What could not be determined

Whether the omission is deliberate (only file-shaped classes are harvestable). Nothing in the
shipped material says so, and the help text makes the broader promise. Whether `harvest` is
meant to emit a *valid* TOML fragment is also unclear: the mirror output is a unified diff, not
a `[omit]` entry.
