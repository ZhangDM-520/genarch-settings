# `apply` refuses every change when one managed target is missing — the shipped base and profile assume power-profiles-daemon and asusd

- **Severity**: high
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest (2 vCPU, no `power-profiles-daemon`, no `asusd`), 2026-11-14. The clean reproduction uses the write-safe rehearsal tree `~/machine/root`.

## Intent

The engine exists to converge the live host, and a machine that is not the profile's model must still be diagnosable and fixable. The shipped `install.sh` says:

> Selecting one for another laptop is not an error: `genarch-settings report` says PROFILE DRIFT and
> exits 1, and `apply` still runs, because the operator has to be able to see it and fix it.

`CONTEXT.md` adds that the base carries "only what is true of any AMD laptop" and that with no profile "absent sections do nothing". The shipped base record `managed/ppd-conflict.toml` targets `power-profiles-daemon.service`, and the shipped profile `zenbook-s16` has five `managed` records targeting `/etc/asusd/asusd.ron`. Neither `power-profiles-daemon` nor `asusd` is a package dependency (`pacman -Qi` lists `gcc-libs ryzenadj systemd iw wireless-regdb`, Optional Deps: None).

## Steps

On the installed machine:

1. `genarch-settings report`
2. `sudo genarch-settings apply --dry-run`

Clean reproduction of the base-layer cause alone, on a rehearsal copy that still has a profile that **matches** its DMI:

3. `cp -a ~/machine/root ~/machine/root-ppd`
4. `rm -f ~/machine/root-ppd/usr/lib/systemd/system/power-profiles-daemon.service`
5. `GENARCH_SETTINGS_ROOT=$HOME/machine/root-ppd genarch-settings report`
6. `GENARCH_SETTINGS_ROOT=$HOME/machine/root-ppd genarch-settings apply --dry-run`

The only escape I found is an operator `[omit]` tombstone; the docs name `omit` but show no example of its syntax:

7. write `~/machine/root-ppd/etc/genarch-settings/intent.toml` as
   ```toml
   [omit]
   managed = ["ppd-conflict"]
   ```
   and rerun step 6.

## Expected

Apply should run (as `install.sh` promises) and the unclassifiable record should not block the unrelated mirrors and sysctls. If a record cannot be classified, the engine should say how to exclude it.

## Actual

Any absent managed target makes `apply` refuse the **whole** intent, even with a profile that matches the machine, even though these targets belong to optional packages. A host without `power-profiles-daemon` (not a dependency) can therefore never apply anything, and the operator cannot even preview the plan (`apply --dry-run` takes the same refusal path). The only documented hint is the prose word "`omit`". The refusal also exits `3`, while the tool's own legend calls that "apply step failed (partial)" and reserves `2` for "precondition failure" — nothing was written.

## Evidence

Installed machine, wrong profile (the package selects `zenbook-s16` on a QEMU box):

```
$ genarch-settings report
DRIFT PROFILE profile 'zenbook-s16' is not for this machine: product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA; sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC. findings#intent
...
DRIFT MANAGED unclassifiable: asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist; asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist; asusd-epp: target /etc/asusd/asusd.ron does not exist; asusd-profile-ac: target /etc/asusd/asusd.ron does not exist; asusd-profile-bat: target /etc/asusd/asusd.ron does not exist; ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
...
SUMMARY ok=3 accepted=0 warn=0 drift=5 skipped=3 exit=1

$ sudo genarch-settings apply --dry-run; echo $?
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
3
```

Clean reproduction with the profile that matches this VM, only `power-profiles-daemon` removed:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-ppd2 genarch-settings report
OK PROFILE profile 'vm-generic' matches product=Standard PC (i440FX + PIIX, 1996) sys_vendor=QEMU findings#intent
OK MIRROR 39 mirror target(s) byte-equal findings#intent
DRIFT MANAGED unclassifiable: ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
OK SYNC 16 runtime sysctl value(s) match intent findings#writeback
OK CEIL all 2 policies capped vm 3000000 findings#ceil
OK CO intent declares coall -3 + 0 per-core offset(s) findings#co
SKIPPED SMU no SMU envelope declared (the silicon is the profile's to describe) findings#ppt
OK POWER intent declares the SMU power-saving flag findings#eco
SKIPPED WIFI iw unavailable or interface absent findings#wifi
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
OK PRUNE 41 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=7 accepted=0 warn=0 drift=2 skipped=2 exit=1
$ echo $?
1

$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-ppd2 genarch-settings apply --dry-run; echo $?
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
3
```

With `[omit] managed = ["ppd-conflict"]` in the operator layer, apply runs:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-ppd genarch-settings apply --dry-run; echo $?
PROFILE vm-generic
PLAN 0 change(s)
APPLY-SCOPE profile=vm-generic full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
DRY-RUN nothing written
0
```

(`PLAN 0` because that copy was already converged; the point is that it is no longer refused.)

## What could not be determined

- Whether the intended Zenbook, with both `asusd` and `power-profiles-daemon` installed, applies cleanly — neither package is available in this guest.
- Whether a missing target is meant to be a hard error (the shipped `AGENTS.md` says "zero or several anchor matches are hard errors", which describes a file that exists but does not match, not a file that is absent).
- Whether the package is expected to depend on `power-profiles-daemon`/`asusd`; `pacman -Qi` says it does not.
