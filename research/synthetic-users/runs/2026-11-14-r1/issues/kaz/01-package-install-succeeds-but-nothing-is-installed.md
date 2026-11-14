# `pacman -U` exits 0 while the engine applies nothing and no unit is installed

- **Severity**: high
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest, package installed from `~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst`, 2026-11-14.

## Intent

The package is the supported install path ("on Arch the PKGBUILD beside this file is the supported path" — `install.sh`), and its scriptlet knows the units exist only after an apply:

> Deploy the intent's mirrors and units, then enable what the package wires up. The units
> themselves ship as intent (one home), so they only exist after an apply.

A package transaction that applies nothing, installs no unit and enables nothing is not a successful install.

## Steps

1. `sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst`
2. `systemctl is-enabled genarch-settings.service`
3. `ls /etc/systemd/system/genarch-settings.service`
4. `ls /var/lib/genarch-settings`
5. Inspect the scriptlet: `bsdtar -xOf ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst .INSTALL`

## Expected

The transaction should not report a successful install when its own converge step refused and no unit exists; at minimum it should end with an unmistakable "installed but not active" message and not attempt to enable units that were never created. The install must not leave the host with an installed package, no configuration and no unit while returning success.

## Actual

Pacman reports success (exit `0`). The scriptlet's `apply` refuses (exit `3`), prints "nothing was written", and the following `systemctl enable` fails with "Unit genarch-settings.service does not exist". Both failures are swallowed by `|| true`. After the transaction: no unit file, no manifest, `systemctl is-enabled` says `not-found`, and only `/var/lib/genarch-settings/lock` exists. The package path also runs `apply` (which writes `/etc` as root) without the `apply --dry-run` preview that `install.sh` performs.

## Evidence

```
$ sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst 2>&1 | tail -12; echo "pacman_exit=${PIPESTATUS[0]}"
...
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
pacman_exit=0
```

```
$ systemctl is-enabled genarch-settings.service; echo $?
not-found
4

$ ls /etc/systemd/system/genarch-settings.service
ls: cannot access '/etc/systemd/system/genarch-settings.service': No such file or directory

$ ls /var/lib/genarch-settings
lock
```

The scriptlet's own comments and code:

```
post_install() {
  # Deploy the intent's mirrors and units, then enable what the package wires up. The units
  # themselves ship as intent (one home), so they only exist after an apply. ...
  /usr/bin/genarch-settings apply || true
  systemctl daemon-reload
  udevadm control --reload || true
  enabled=$(units)
  if [ -n "$enabled" ]; then
    systemctl enable $enabled || true
  fi
}
```

`pacman -Qi genarch-settings` reports `Optional Deps: None`, `Depends On: gcc-libs ryzenadj systemd iw wireless-regdb`; neither `power-profiles-daemon` nor `asusd` is installed, which is why the apply refuses (see the `apply` issue).

## What could not be determined

- The transaction output of the original installation, before this review; only the reinstall path is reproduced.
- Whether the package is meant to ship a unit file directly instead of relying on apply; the scriptlet comment says it is not.
