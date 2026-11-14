# A rehearsal of `converge boot` exits 3 whenever the profile declares SMU limits

- **Severity:** low
- **Category:** enhancement
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1, rehearsal root `~/machine/root`

## Intent

Rehearse a new profile that carries a ryzenadj envelope, and learn whether the rest of the pass
is sound. `README.md` promises: "`GENARCH_SETTINGS_ROOT=/path/to/copy` aims every live path at a
copy for a rehearsal. It does not execute external tools: a rehearsal never writes this machine's
SMU and never reloads its systemd or udev."

## Steps

```sh
# with [limits] present in profiles/qemu-i440fx/ryzenadj.toml
GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings converge boot; echo $?
```

## Expected

The engine refusing to run ryzenadj under a rehearsal root is the documented promise; the pass
should report that it was not measured, and the rehearsal should stay distinguishable from a
failed boot pass (exit 0 or a distinct marker).

## Actual

Every call is counted as a failure, the pass ends exit 3, and the wording ("ryzenadj failed",
"0 of 2 … may still stand") reads exactly like a real boot-pass failure. A porter rehearsing a
candidate profile cannot tell "refused on purpose" from "the pass is broken". Without any
ryzenadj declaration the same rehearsal exits 0.

## Evidence

```
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings converge boot; echo "exit=$?"
genarch-settings: ryzenadj failed: ryzenadj --stapm-limit=28000 --fast-limit=35000 --slow-limit=30000 (-1): rehearsal root: not executing ryzenadj
genarch-settings: ryzenadj failed: ryzenadj --power-saving (-1): rehearsal root: not executing ryzenadj
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: all 2 ryzenadj call(s) failed — no SMU setting was applied
exit=3
```

## What could not be determined

Whether a real root would fail the same way when ryzenadj is missing (as opposed to when it is
merely suppressed); this box has ryzenadj installed.
