# `converge boot` exits 0 when every `ryzenadj` write failed

**Severity:** Medium
**Category:** bug
**Environment:** Arch guest (no AMD SMU). Rehearsal root `~/ev5` with the `zenbook-s16`
profile selected; a failing `ryzenadj` shim on `PATH` to remove the hardware explanation.

## Intent

`AGENTS.md` (shipped):

> `A refused write in a pass is a failure (exit 3), never .is_ok()` — `The pass code swallowed sysfs/procfs errors, so a pass exited 0 while the governor/wake/USB/sysctl value never moved.`

`DESIGN.md` says the boot/resume split exists so the SMU work lands after the platform-profile
handlers. The systemd unit is what an operator and `systemctl status` see:

> `ExecStart=/usr/bin/genarch-settings converge boot`

## Steps

```sh
# ~/r40 = a converged copy of ~/machine/root with sys/class/dmi/id/{product_name,sys_vendor}
# set to the Zenbook strings, profile = "zenbook-s16", and an /etc/asusd/asusd.ron containing
# the five anchors the profile patches.
R=~/ev5; rm -rf $R; cp -a ~/r40 $R        # zenbook profile; fake DMI; asusd.ron
printf '#!/bin/sh\necho "ryzenadj: simulated failure" >&2\nexit 1\n' > ~/shim/ryzenadj
chmod +x ~/shim/ryzenadj
GENARCH_SETTINGS_ROOT=$R PATH=$HOME/shim:$PATH genarch-settings converge boot; echo "EXIT=$?"

# contrast: the same pass with one unwritable sysctl
printf 60 > $R/proc/sys/vm/swappiness; chmod 444 $R/proc/sys/vm/swappiness
GENARCH_SETTINGS_ROOT=$R PATH=$HOME/shim:$PATH genarch-settings converge boot; echo "EXIT=$?"
```

## Expected

Exit 3 (partial) when every SMU write failed, as the pass does for a sysfs/procfs write. The
unit is not marked successful when the firmware limits were never written.

## Actual

The pass logs `12 of 12 ryzenadj call(s) failed — firmware limits may still stand` (twice, for
the settle repeat) and exits **0**. The same pass with one unwritable sysctl exits 3. The
`genarch-settings.service` unit runs this command and so records a successful boot pass on a
machine whose curve offsets and power limit were never applied.

## Evidence (verbatim; the `...` elides the other nine identical failure lines)

```
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFB (1): ryzenadj: simulated failure
genarch-settings: ryzenadj failed: ryzenadj --set-coper=0x0000FFEB (1): ryzenadj: simulated failure
...
genarch-settings: smu: 12 of 12 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: 12 of 12 ryzenadj call(s) failed — firmware limits may still stand
EXIT=0
--- contrast: an unwritable sysctl path in the same pass
EXIT=3
```

The real (unshimmed) `ryzenadj` fails the same way on this VM and the pass is still 0:

```
$ genarch-settings converge boot
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (1): Error: Unable to get SMU object (no AMD SMU on this machine)
...
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
EXIT=0
```

## What could not be determined

Whether a real SMU machine with a broken `ryzenadj` takes the same path (the shim shows the
code path but not the hardware). Whether the SMU failure is deliberately non-fatal: `install.sh`
says a missing `ryzenadj` is "reported, never fatal", but no shipped text says a *failed* write
is a successful pass.
