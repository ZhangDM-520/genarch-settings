# A failed write makes `apply` exit 3 with no plan, no summary and no account of what was applied

- **Severity**: medium
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest; reproductions use write-safe rehearsal trees under `~/machine`, 2026-11-14.

## Intent

`apply` is supposed to be a plan then an execution of exactly that plan, and a partial result is a first-class outcome:

- `CONTEXT.md`: "**Plan** — the complete list of divergences between intent and live, computed before anything is written. `apply --dry-run` prints it; `apply` executes exactly it."
- `genarch-settings --help`: "Exit codes: 0 clean ..., 1 drift present or persisted after apply, 2 precondition failure, 3 apply step failed (partial)."

A failed apply therefore needs to say what was planned, what succeeded and what failed — otherwise "recover from an apply that went wrong" is guesswork.

## Steps

Reproduction A — one target refuses the rename after another target has already been applied:

1. `cp -a ~/machine/root ~/machine/root-part`
2. `echo "# local-edit" >> ~/machine/root-part/etc/tlp.d/01-eco.conf`
3. `echo "# local-edit" >> ~/machine/root-part/etc/udev/rules.d/61-eco-ioschedulers.rules`
4. `sudo chattr +i ~/machine/root-part/etc/udev/rules.d/61-eco-ioschedulers.rules`
5. `GENARCH_SETTINGS_ROOT=$HOME/machine/root-part genarch-settings apply > out 2> err; echo $?`
6. `wc -c < out`, `cat err`, then compare the tlp target with the intent mirror and the udev target with its local edit.
7. Undo the immutable bit: `sudo chattr -i ...`
8. Contrast: `GENARCH_SETTINGS_ROOT=$HOME/machine/root-part genarch-settings apply --dry-run`

Reproduction B — a sysctl the kernel does not expose:

9. `cp -a ~/machine/root ~/machine/root-sk`
10. write `~/machine/root-sk/etc/genarch-settings/sysctl.toml` as
    ```toml
    [values]
    "kernel.does_not_exist" = "1"
    ```
11. `GENARCH_SETTINGS_ROOT=$HOME/machine/root-sk genarch-settings apply; echo $?`
12. `grep -n does_not_exist ~/machine/root-sk/etc/sysctl.d/90-genarch-settings.conf`

## Expected

The plan printed before any write; on a write failure, a summary naming the applied and failed steps (and what is left to do). If the engine re-plans, it should say so. A kernel attribute that cannot be written should not abort the rest of the plan without a report.

## Actual

On failure `apply` prints **nothing on stdout** (0 bytes), writes one raw OS error to stderr, and exits `3`. No plan, no `SUMMARY`, no "nothing was written" reassurance (unlike the classification refusal, which does say it). In reproduction A the earlier mirror *was* fully applied before the later one failed, so the host is left partially converged with no account of it. In reproduction B the sysctl projection file is written first (it now contains the invalid key) and the runtime sysctl write then aborts the apply. The same tree prints a normal plan under `--dry-run`.

## Evidence

Reproduction A:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-part genarch-settings apply > out 2> err; echo $?
3
$ wc -c < out
0
$ cat err
genarch-settings: rename /home/arch/machine/root-part/etc/udev/rules.d/61-eco-ioschedulers.rules: Operation not permitted (os error 1)
$ diff -u ~/machine/root/usr/lib/genarch-settings/mirror/etc/tlp.d/01-eco.conf ~/machine/root-part/etc/tlp.d/01-eco.conf && echo IDENTICAL
IDENTICAL
$ grep -c local-edit ~/machine/root-part/etc/udev/rules.d/61-eco-ioschedulers.rules
1
```

The earlier target was applied, the later one was not, and the command printed none of that.

`--dry-run` on the same tree does print the plan:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-part genarch-settings apply --dry-run
PROFILE vm-generic
PLAN 1 change(s)
  OWNED /etc/udev/rules.d/61-eco-ioschedulers.rules
APPLY-SCOPE profile=vm-generic full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
DRY-RUN nothing written
$ echo $?
0
```

Reproduction B:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root-sk genarch-settings apply; echo $?
genarch-settings: write /home/arch/machine/root-sk/proc/sys/kernel/does_not_exist: No such file or directory (os error 2)
3
$ grep -n does_not_exist ~/machine/root-sk/etc/sysctl.d/90-genarch-settings.conf
4:kernel.does_not_exist = 1
```

The projection (an engine-owned file) was already written; the runtime write failed; no summary says either. The next `report` shows `DRIFT SYNC kernel.does_not_exist=unreadable want 1` and `OK MIRROR`, i.e. a partially converged host.

## What could not be determined

- Whether the engine re-plans after a partial failure; the code path is not observable from outside and no summary is printed.
- Whether `converge boot` has the same silence; it exits 3 on the same unexposed sysctl but prints the raw error only.
