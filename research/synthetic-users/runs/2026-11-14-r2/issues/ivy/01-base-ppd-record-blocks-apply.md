# The base intent's `ppd-conflict` record blocks apply on any host without power-profiles-daemon

**Severity:** high
**Category:** bug
**Environment:** Arch Linux, QEMU/KVM (product `Standard PC (i440FX + PIIX, 1996)`, `sys_vendor=QEMU`), genarch-settings 0.4.0-1, staging tree `~/machine/root` with the matching operator profile `vm-generic`. `power-profiles-daemon` is not installed (`pacman -Q power-profiles-daemon` → `error: package 'power-profiles-daemon' was not found`); the package does not depend on it.

## Intent

Install the engine on a headless box and run the base intent (no machine profile selected, or a
profile that matches this box), as the shipped README says a fresh host does.

## Steps

```
cp -a ~/machine/root.pristine ~/machine/root.ppd
rm -f ~/machine/root.ppd/usr/lib/systemd/system/power-profiles-daemon.service
GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings apply
GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings report
```

## Expected

The base is the host-agnostic layer; the README says a fresh host runs it:

> `install.sh` is the portable path: it installs the base and every profile but selects none, so a
> fresh host runs the base intent and reports `PROFILE SKIPPED` until an operator writes
> `profile = "<name>"` into `/etc/genarch-settings/profile`.

`PROFILES.md` says a profile ships no generic file because "the base already ships everything
generic". A base record for an optional desktop service should therefore not be able to stop the
base from applying on a server that does not run that service.

## Actual

`apply` refuses the entire plan with exit 3 and writes nothing. The profile matches; the blocker is
a single base managed record whose target is a unit from a package the engine neither depends on
nor mentions. The same block occurs on the real box during `pacman -U` (see issue 02): the install
prints `ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist`.

## Evidence

```
$ rm -f ~/machine/root.ppd/usr/lib/systemd/system/power-profiles-daemon.service
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings apply; echo EXIT=$?
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
```

The profile itself matches and the report still shows every other row as work to do:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings report
OK PROFILE profile 'vm-generic' matches product=Standard PC (i440FX + PIIX, 1996) sys_vendor=QEMU findings#intent
DRIFT MIRROR 40 target(s) differ: /etc/NetworkManager/conf.d/99-eco-dns.conf [...] findings#intent
DRIFT MANAGED unclassifiable: ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
DRIFT SYNC fs.file-max=9223372036854775807 want 2097152 [...] findings#writeback
[...]
SUMMARY ok=5 accepted=0 warn=0 drift=4 skipped=2 exit=1
```

Package metadata does not require the target package:

```
$ pacman -Qi genarch-settings | grep -E 'Depends On|Conflicts'
Depends On      : gcc-libs  ryzenadj  systemd  iw  wireless-regdb
Conflicts With  : genarch
$ pacman -Q power-profiles-daemon
error: package 'power-profiles-daemon' was not found
$ ls /usr/lib/systemd/system/power-profiles-daemon.service
ls: cannot access '/usr/lib/systemd/system/power-profiles-daemon.service': No such file or directory
```

Restoring only that one file in the same tree makes the same intent apply cleanly, so the absent
managed target is the whole blocker:

```
$ cp -a ~/machine/root.ppd ~/machine/root.ppd2
$ cp ~/machine/root.pristine/usr/lib/systemd/system/power-profiles-daemon.service \
    ~/machine/root.ppd2/usr/lib/systemd/system/
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd2 genarch-settings apply; echo EXIT=$?
  /usr/lib/systemd/system/power-profiles-daemon.service.bak-20261114
SUMMARY planned=56 applied=56 failed=0 exit=0
EXIT=0
```

## What could not be determined

- Whether the maintainer considers `power-profiles-daemon` a required host package. No shipped
  document says to install it, and it is absent from `Depends On`; the README's only prerequisite
  note is about `ryzenadj`.
- Whether the intended behaviour is to skip an absent optional target instead of refusing the
  plan. The README documents the refusal for a profile record (`asusd.ron`), not for a base
  record, and says "the drift to fix is the selection" — which does not apply here, since the
  selection matches.
