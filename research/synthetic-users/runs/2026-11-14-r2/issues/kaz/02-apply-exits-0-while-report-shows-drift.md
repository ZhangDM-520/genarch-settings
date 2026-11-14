# `apply` exits 0 and says `failed=0` while the host it just converged still reports two DRIFT rows

- **Severity**: medium
- **Category**: bug
- **Environment**: Arch Linux 7.2.11-arch1-1 (QEMU), `genarch-settings 0.4.0-1`;
  rehearsal root `~/machine/root` with the `vm-generic` profile (two fake cpufreq policies)

## Intent

Use `apply`'s exit status as the answer to "is this host converged" — in a hook, a unit, or my own
`if apply; then echo clean`.

## Steps

```
~/machine.sh ~/machine
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
genarch-settings apply; echo "EXIT:$?"
genarch-settings report; echo "EXIT:$?"
```

## Expected

The README, "What it does":

> Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3` partial.

and, on the same page, that the ceiling row is part of the judgement (`CEIL` is one of the eleven
rows). A host that still answers "DRIFT" to `report` is not `0` clean.

## Actual

```
BACKUPS 2
  /etc/systemd/journald.conf.bak-20261114
  /usr/lib/systemd/system/power-profiles-daemon.service.bak-20261114
SUMMARY planned=56 applied=56 failed=0 exit=0
EXIT:0
DRIFT CEIL policy0=4000000 want 3000000 policy1=4000000 want 3000000 findings#ceil
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
SUMMARY ok=7 accepted=0 warn=0 drift=2 skipped=2 exit=1
EXIT:1
```

Five seconds apart: `apply` says clean, `report` says drift. `apply` does print
`none=PROFILE,CEIL,SMU,TIMER` in its `APPLY-SCOPE` line, so it knows — but the number a script
reads says otherwise.

## Evidence

Reproduced three times on fresh trees; the JSON agrees with the table:

```
$ genarch-settings report --json | python3 -c 'import json,sys;d=json.load(sys.stdin);print([(c["id"],c["class"],c["reach"]) for c in d["checks"] if c["id"] in ("CEIL","TIMER")])'
[('CEIL', 'DRIFT', 'none'), ('TIMER', 'DRIFT', 'none')]
```

`converge ceil` fixes the CEIL row (`policy0 4000000 -> 3000000`), so the drift is real and
reachable — just not by the verb whose exit code claims convergence.

## What could not be determined

Whether the second apply that a timer or udev rule performs is meant to be the only path to a
clean host; the README's contract says `0` means clean, and this host is not.
