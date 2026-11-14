# One absent managed target refuses the entire plan, and the package depends on neither file it patches

**Severity:** High
**Category:** bug
**Environment:** Arch guest, `genarch-settings 0.4.0-1`. Neither `power-profiles-daemon` nor
`asusd` is installed. Rehearsal roots `~/r42` (asusd.ron missing, ppd present) and `~/r43`
(ppd unit missing, asusd present), both otherwise converged.

## Intent

`CONTEXT.md` says the base layer is only what is true of every host:

> `The base carries only what is true of any AMD laptop.`

and `AGENTS.md` says an unclassifiable write is refused per record:

> `A write the engine cannot classify is refused, not guessed (zero or several anchor matches).`

A missing *target file* is not a zero-anchor match; it is a host that does not have the optional
package the drop-in is for. The rest of the plan (40 mirrors, 16 sysctls, the units) is
independent of it. `install.sh` promises the base-only host works:

> `Nothing selects a profile for you: a fresh host runs the base intent and reports PROFILE SKIPPED until an operator writes that line.`

## Steps

```sh
# ~/r14 is a converged copy of ~/machine/root
# on the real (stock) host, with the shipped intent
sudo genarch-settings apply --dry-run; echo "EXIT=$?"

# only the profile's asusd target absent; ppd present
R=~/r42; cp -a ~/r14 $R; rm -f $R/etc/asusd/asusd.ron
GENARCH_SETTINGS_ROOT=$R genarch-settings apply --dry-run; echo "EXIT=$?"

# only the base ppd target absent; asusd present
R=~/r43; cp -a ~/r14 $R; rm -f $R/usr/lib/systemd/system/power-profiles-daemon.service
GENARCH_SETTINGS_ROOT=$R genarch-settings apply --dry-run; echo "EXIT=$?"

# base only (no selection), ppd absent
R=~/r53; cp -a ~/r14 $R; rm -f $R/etc/genarch-settings/profile \
    $R/usr/lib/systemd/system/power-profiles-daemon.service
GENARCH_SETTINGS_ROOT=$R genarch-settings apply --dry-run; echo "EXIT=$?"
```

## Expected

The base intent is deployable on a stock Arch host: either the ppd record is skipped/reported
as SKIPPED (it is a drop-in for an optional package) and the rest applies, or the package
*depends* on the packages whose files it patches so the target exists. `report` already calls
the condition DRIFT (exit 1), which an operator reads as "apply will fix it".

## Actual

`apply` refuses **everything** — including the 40 mirrors, the sysctl projection, the units and
the planned prunes — and exits 3 although **nothing** was written. `report` reports the same
condition as `DRIFT` (exit 1). The package declares dependencies on `gcc-libs`, `ryzenadj`,
`systemd`, `iw`, `wireless-regdb` only; it neither ships nor depends on
`power-profiles-daemon` or `asusd`.

## Evidence (verbatim)

Real host:

```
$ sudo genarch-settings apply --dry-run; echo "EXIT=$?"
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

The same condition in `report`:

```
DRIFT MANAGED unclassifiable: asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist; asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist; asusd-epp: target /etc/asusd/asusd.ron does not exist; asusd-profile-ac: target /etc/asusd/asusd.ron does not exist; asusd-profile-bat: target /etc/asusd/asusd.ron does not exist; ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
SUMMARY ok=3 accepted=0 warn=0 drift=5 skipped=3 exit=1
```

One record absent is enough (ppd present):

```
$ GENARCH_SETTINGS_ROOT=~/r42 genarch-settings apply --dry-run; echo "EXIT=$?"
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
EXIT=3
```

Even with **no profile selected** (the base intent only, as on a fresh `install.sh` host), the
base `ppd-conflict` record blocks the whole apply when ppd is absent (`~/r53`):

```
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
SKIPPED PROFILE no profile selected (base intent only) findings#intent
```

Dependencies:

```
$ pacman -Qi genarch-settings | grep -E "Depends|Optional"
Depends On      : gcc-libs  ryzenadj  systemd  iw  wireless-regdb
Optional Deps   : None
$ pacman -Q power-profiles-daemon asusd
error: package 'power-profiles-daemon' was not found
error: package 'asusd' was not found
```

Exit-code contract (from `genarch-settings --help`):

```
Exit codes: 0 clean …, 1 drift present or persisted after apply, 2 precondition failure, 3 apply step failed (partial).
```

## What could not be determined

Whether the vendor laptop has both packages installed. If it does, the defect surfaces only on a
second machine or after a package removal. The engine's all-or-nothing choice is defensible in
isolation; the package making its own base intent unapplicable on a stock host is not.
