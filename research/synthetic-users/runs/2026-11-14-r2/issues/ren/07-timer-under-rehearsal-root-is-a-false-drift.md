# Under a rehearsal root, TIMER is reported DRIFT even when the machine's units are enabled

- **Severity:** medium
- **Category:** bug
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1, rehearsal root `~/t8`; real host systemd running

## Intent

Rehearse the second machine with `GENARCH_SETTINGS_ROOT=$HOME/t8` and judge how far the tree is
from the intent. `CONTEXT.md`: "`SKIPPED` means unverified, not fine" — an unobserved row is
`SKIPPED`, not a claim about the machine.

## Steps

```sh
sudo cp .../mirror/etc/systemd/system/genarch-settings.service .../genarch-settings-ceil.timer /etc/systemd/system/
sudo systemctl enable genarch-settings.service genarch-settings-ceil.timer
genarch-settings report | grep TIMER                 # real root
GENARCH_SETTINGS_ROOT=$HOME/t8 genarch-settings report | grep TIMER   # rehearsal root of the same host
```

## Expected

The `TIMER` row should be judged the same way, or be `SKIPPED` when a rehearsal root cannot
observe the host's systemd. Either way a rehearsal must not assert "not enabled" about a machine
whose units *are* enabled.

## Actual

With the units enabled on this machine, the real-root report says `OK TIMER … enabled`; the same
machine with a rehearsal root says `DRIFT TIMER not enabled`. The row is therefore unsatisfiable
in every rehearsal (a converged rehearsal tree always ends `SUMMARY … exit=1`), and the word
"not enabled" states a fact the engine did not read. `strace -f` over the rehearsal report shows
no `systemctl` execution, so the row is not measured at all.

## Evidence

```
$ genarch-settings report | grep -E "TIMER|SUMMARY" | cut -c1-100
OK TIMER ["genarch-settings.service", "genarch-settings-ceil.timer"] enabled findings#timer
SUMMARY ok=4 accepted=0 warn=0 drift=4 skipped=3 exit=1

$ GENARCH_SETTINGS_ROOT=$HOME/t8 genarch-settings report | grep -E "TIMER|SUMMARY"
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
SUMMARY ok=4 accepted=0 warn=0 drift=1 skipped=6 exit=1

$ GENARCH_SETTINGS_ROOT=$HOME/t8 strace -f -e trace=execve -o ~/t8e.strace genarch-settings report >/dev/null 2>&1
$ grep execve ~/t8e.strace
4776  execve("/usr/bin/genarch-settings", ["genarch-settings", "report"], 0x7ffed4d71f00 /* 18 vars */) = 0
```

(The unit files and `multi-user.target.wants` / `timers.target.wants` links also existed inside
`~/t8`, so the rehearsal tree itself was in the enabled state.)

## What could not be determined

How the real-root `TIMER` check reads enablement (no `systemctl` exec and no D-Bus connection is
visible); the asymmetry between the two runs is the evidence, not the mechanism.
