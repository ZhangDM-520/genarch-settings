# the pacman install deploys the two units but never enables them, so `TIMER` stays DRIFT with no shipped command to fix it

- **Severity**: medium
- **Category**: enhancement — the blocked workflow is "get a package-installed host to a clean report"
- **Environment**: Arch Linux 7.2.11-arch1-1 (QEMU), `genarch-settings 0.4.0-1`, installed with
  `pacman -U` from `~/payload`; rehearsal root for the deployment half

## Intent

Install with the package, converge, and reach a report that does not end in `exit=1` — or be told
the exact command that would get there.

## Steps

```
~/machine.sh ~/machine
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
genarch-settings apply >/dev/null
ls $HOME/machine/root/etc/systemd/system/genarch-settings*
genarch-settings report | grep TIMER
```

## Expected

The README documents `units` as feeding an installer ("the units the intent expects enabled, one
per line") and describes `install.sh` as the path that "builds, installs, converges, enables the
units". For the package path it stops at "converges"; the `TIMER` row then asks for something no
shipped command performs.

## Actual

```
$ ls $HOME/machine/root/etc/systemd/system/genarch-settings*
/home/arch/machine/root/etc/systemd/system/genarch-settings-ceil.service
/home/arch/machine/root/etc/systemd/system/genarch-settings-ceil.timer
/home/arch/machine/root/etc/systemd/system/genarch-settings.service
$ genarch-settings report | grep TIMER
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
```

The row's reach is `none`, so no `apply` will ever clear it; nothing in the engine calls
`systemctl enable` (a shim on `PATH` logging every external call stayed empty through `apply`,
`converge boot`, `resume` and `ceil`). On the real box the same thing happens one step earlier:
`pacman -U` runs a `systemctl enable` before the units exist and prints
`Failed to enable unit: Unit genarch-settings.service does not exist`, while the transaction still
exits 0.

## Evidence

Verbatim above, plus the install-time failure from retest item 1:

```
Failed to enable unit: Unit genarch-settings.service does not exist
PACMAN EXIT:0
```

## What could not be determined

Whether systemd accepts these unit files once enabled (I did not boot a systemd against the
rehearsal root), so this is about the missing step and the missing sentence, not about the units'
contents.
