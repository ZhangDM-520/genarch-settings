# One unclassifiable managed record aborts the entire apply

**Severity:** high
**Category:** bug

**Environment:** Arch guest; `genarch-settings 0.4.0-1`. Reproduced twice: on a copy of the
staging tree (`~/machine/root.noppd`) and on the real install. (`root.pristine` is a byte copy
of the delivered `~/machine/root` taken before any apply; `root.noppd` is a copy of it.) A
second reproduction uses a foreign config that contains both a commented default and an active
setting.

**Intent:** Apply the engine's base intent on a headless box. `power-profiles-daemon` is a
laptop daemon and is not installed; the package does not list it as a dependency, so its
absence should not stop the other 40 mirror targets, the 14 sysctl writes and the journald
line.

**Steps:**

Base-only reproduction (isolates the base layer, no profile):

```sh
cp -a ~/machine/root.pristine ~/machine/root.noppd
rm ~/machine/root.noppd/usr/lib/systemd/system/power-profiles-daemon.service
rm ~/machine/root.noppd/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=$HOME/machine/root.noppd genarch-settings report
GENARCH_SETTINGS_ROOT=$HOME/machine/root.noppd genarch-settings apply --dry-run
```

Real install (the package's shipped `zenbook-s16` selection):

```sh
sudo genarch-settings apply --dry-run
```

Second reproduction (a file with both the commented default and an active line):

```sh
printf '[Journal]\n#SystemMaxUse=\nStorage=auto\nSystemMaxUse=999M\n' \
  > ~/machine/root/etc/systemd/journald.conf
GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply
```

**Expected:** CONTEXT says the refusal is per write:

> A write the engine cannot classify is refused, not guessed (zero or several anchor matches).

`install.sh` says a missing dependency is not fatal:

> A missing runtime dependency is reported, never fatal: the report marks the rows it cannot
> judge as SKIPPED, and an install that refuses to proceed over a missing `ryzenadj` would be
> unable to fix anything.

and a profile mismatch is supposed to let the rest run:

> Selecting one for another laptop is not an error: `genarch-settings report` says PROFILE DRIFT
> and exits 1, and `apply` still runs, because the operator has to be able to see it and fix it.

I expected the unclassifiable record to be reported (as MANAGED drift or SKIPPED) and the other
changes to apply.

**Actual:** The whole intent is refused. No mirror, no sysctl and no managed line is written,
and the exit code is 3. The base layer alone cannot be applied on any host without
`power-profiles-daemon`, and the shipped profile cannot be applied on any host without
`/etc/asusd/asusd.ron`. On the real install, `apply` also refuses, contradicting "apply still
runs".

**Evidence:**

Base-only, no PPD:

```
SKIPPED PROFILE no profile selected (base intent only) findings#intent
DRIFT MIRROR 40 target(s) differ: /etc/NetworkManager/conf.d/99-eco-dns.conf ... findings#intent
DRIFT MANAGED unclassifiable: ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
DRIFT SYNC fs.file-max=9223372036854775807 want 2097152 kernel.hung_task_panic=0 want 1 kernel.kptr_restrict=0 want 2 kernel.nmi_watchdog=1 want 0 kernel.printk=4 4 1 7 want 3 3 3 3 kernel.sysrq=16 want 190 net.core.netdev_max_backlog=1000 want 4096 vm.dirty_background_bytes=0 want 67108864 vm.dirty_bytes=0 want 268435456 vm.dirty_expire_centisecs=3000 want 6000 vm.dirty_writeback_centisecs=500 want 6000 vm.page-cluster=3 want 0 vm.swappiness=60 want 150 vm.vfs_cache_pressure=100 want 50 findings#writeback
SKIPPED CEIL no ceiling classes declared (ceil.toml) findings#ceil
SKIPPED CO no curve-optimizer offset declared (the silicon is the profile's to describe) findings#co
SKIPPED SMU no SMU envelope declared (the silicon is the profile's to describe) findings#ppt
SKIPPED POWER no --power-saving declaration (the silicon is the profile's to describe) findings#eco
SKIPPED WIFI profile declares no wifi_interface findings#wifi
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
OK PRUNE 0 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=1 accepted=0 warn=0 drift=4 skipped=6 exit=1
REPORT_EXIT=1
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
DRY_EXIT=3
```

(The `MIRROR` line is truncated here for width; it lists 40 targets. The full line is in the
transcript.)

Real install:

```
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
```

Two-anchor case:

```
genarch-settings: refusing to apply: intent cannot be classified
  journald: 2 lines match anchor in /etc/systemd/journald.conf — refusing to choose
nothing was written
EXIT=3
```

The package's dependency list does not include PPD:

```
$ pacman -Qi genarch-settings | grep "Depends On"
Depends On      : gcc-libs  ryzenadj  systemd  iw  wireless-regdb
```

The escape hatch exists, but no shipped example shows its syntax: an operator can add an
operator `/etc/genarch-settings/intent.toml` with

```toml
[omit]
managed = ["ppd-conflict"]
```

and the same dry run then prints `PLAN 55 change(s)` and `DRY-RUN nothing written`, exit 0. I
found the syntax only by provoking TOML "unknown field" errors; no shipped example shows it.

**Why it matters:** the base intent is not applied on a headless box until an operator guesses
an `omit` tombstone, and the package's own install hook fails on every such host (issue 01).
A single optional vendor file being absent or ambiguous disables the other 55 changes.

**What I could not determine:** whether the intended semantics is "skip this record and report
it" or "refuse this record only"; the shipped text says "a write ... is refused", which suggests
the latter. Whether `asusd` is intended to be a hard requirement of the `zenbook-s16` profile
(the package does not depend on it).
