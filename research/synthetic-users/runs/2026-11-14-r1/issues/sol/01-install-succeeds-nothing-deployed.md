# `pacman -U` reports success while the package's post_install apply deployed nothing

**Severity:** High
**Category:** bug
**Environment:** Arch Linux guest, 2 vCPU, `genarch-settings 0.4.0-1` from `~/payload`, no
`power-profiles-daemon`, no `asusd`/`asusctl`. Package installed with `pacman -U --noconfirm`.

## Intent

Install the package the supported way and get a working engine: its mirrors, units and the
two expected-enabled units. `.INSTALL` says it does exactly that:

> `# Deploy the intent's mirrors and units, then enable what the package wires up. The units
> # themselves ship as intent (one home), so they only exist after an apply.`

and `install.sh` says:

> `The engine is idempotent: installing over an existing one re-plans and converges, it does not reset anything.`

## Steps

```sh
sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
systemctl is-enabled genarch-settings.service genarch-settings-ceil.timer
ls /etc/modprobe.d /etc/udev/rules.d
```

(A reinstall was used; the first install in `/var/log/pacman.log` shows the same failure.)

## Expected

The package deploys the intent and enables `genarch-settings.service` and
`genarch-settings-ceil.timer`, or the install fails so the operator knows. The help text's exit
contract is:

> `0 clean … 1 drift present or persisted after apply, 2 precondition failure, 3 apply step failed (partial)`

A privileged failure must not be turned into a successful exit by the installer.

## Actual

`apply` exits 3 and writes nothing. The scriptlet runs it as `... apply || true`, then
`systemctl enable` fails because the units were never deployed. `pacman` exits **0**.

## Evidence (verbatim)

```
:: Processing package changes...
reinstalling genarch-settings...
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
Failed to enable unit: Unit genarch-settings.service does not exist
:: Running post-transaction hooks...
(1/1) Arming ConditionNeedsUpdate...
PACMAN_EXIT=0
=== unit enabled? ===
not-found
not-found
=== /etc state ===
ls: cannot access '/etc/tlp.d': No such file or directory
/etc/modprobe.d:

/etc/udev/rules.d:
```

The scriptlet line (from the package's `.INSTALL`):

```
  /usr/bin/genarch-settings apply || true
```

Package dependencies (from `.PKGINFO` / `pacman -Qi`), none of which provide either missing
target:

```
Depends On      : gcc-libs  ryzenadj  systemd  iw  wireless-regdb
```

## What could not be determined

Whether the vendor laptop has `power-profiles-daemon` and `asusd` installed, and therefore
whether the same install succeeds there. On this clean box the package installs **nothing** and
says it succeeded.
