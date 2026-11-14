# `GENARCH_SETTINGS_ROOT` does not isolate external commands: `converge boot` still runs the real `ryzenadj` (SMU writes)

- **Severity**: medium
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest with `ryzenadj 0.19.0-1`, no AMD SMU; rehearsal tree `~/machine/root`, 2026-11-14.

## Intent

The rehearsal root is described as a copy that makes writes safe:

- `~/machine/README`: "point every live path at a copy. Nothing under `root/` is the real machine, so writes there are safe and need no privilege."
- Shipped `AGENTS.md`: "Rehearsal without touching the machine: `GENARCH_SETTINGS_ROOT=/path/to/copy` aims every live path at a copy of the tree".

The variable redirects paths; it should not leave a command that programs the real silicon running.

## Steps

1. Run a pass against the rehearsal root:
   `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot`
2. Observe that the real `/usr/bin/ryzenadj` is invoked (its own error message appears).
3. Prove the invocation with a temporary wrapper on `PATH`:
   `mkdir -p ~/bin; printf '#!/bin/sh\necho CALLED ryzenadj $* >> /home/arch/calls.log\nexit 0\n' > ~/bin/ryzenadj; chmod +x ~/bin/ryzenadj`
   `rm -f ~/calls.log; PATH=$HOME/bin:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot; cat ~/calls.log`
4. Compare with `apply`, which does not call `ryzenadj` (the SMU work is pass-only).

## Expected

A rehearsal should either skip the external SMU tool (and say so) or refuse the pass under `GENARCH_SETTINGS_ROOT`. If the external call is intentional, the rehearsal documentation must say that only paths are redirected, not the SMU.

## Actual

`converge boot` resolves `ryzenadj` from `PATH` and runs it for real with `GENARCH_SETTINGS_ROOT` set. On this guest it fails only because there is no SMU; on a real Zenbook the same command would set the curve offset, power table and power-saving flag on the live firmware while the operator is "rehearsing" against a copy. The pass still exits 0 while every SMU call failed.

## Evidence

Real binary, no wrapper:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: ryzenadj failed: ryzenadj --power-saving (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: sysctl re-asserted: vm.swappiness
$ echo $?
0
```

Wrapper proof that it is the engine's external call, not something else:

```
$ PATH=$HOME/bin:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot
genarch-settings: smu: 2 ryzenadj call(s) applied (CO -3, 0 per-core offset(s), limits false, power-saving true)
genarch-settings: sysctl re-asserted: vm.swappiness
$ cat ~/calls.log
CALLED ryzenadj --set-coall=0x0000FFFD
CALLED ryzenadj --power-saving
```

The `apply` command under the same root does not invoke it:

```
$ PATH=$HOME/bin:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply >/dev/null; echo $?
0
$ cat ~/calls.log
cat: /home/arch/calls.log: No such file or directory
```

The external call is specific to the pass.

## What could not be determined

- Whether a real SMU would accept these writes; this guest has no AMD SMU, so the failure is hardware absence, not proof of the write. The invocation itself is proven.
- Whether `converge` is considered part of the supported rehearsal surface; the README shows only `report`/`apply`, while `AGENTS.md` says "rehearsal without touching the machine" without excluding passes. The `converge` help says "Called by systemd units and udev rules, not by hand".
