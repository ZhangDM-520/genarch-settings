# `harvest` prints nothing for sysctl and ceiling drift and exits 0, so it looks exactly like a converged host

- **severity:** medium
- **category:** bug
- **environment:** Arch Linux guest (QEMU, 2 vCPU), `genarch-settings 0.4.0-1`; converged copy of the staging tree (`GENARCH_SETTINGS_ROOT=~/ev6`).

## Intent

Use the verb the docs describe as the way to make the intent match live: drift a sysctl value and
a ceiling, then ask `harvest` what the intent change would be.

## Steps

```sh
cp -a ~/machine/root ~/ev6
printf '10' > ~/ev6/proc/sys/vm/swappiness
printf '4000000' > ~/ev6/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings report | grep -E "SYNC|CEIL|SUMMARY"
GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings harvest > ~/ev6.h.out 2> ~/ev6.h.err
echo "exit=$? stdout=$(wc -c < ~/ev6.h.out) stderr=$(wc -c < ~/ev6.h.err)"
cat ~/ev6.h.out ~/ev6.h.err

# contrast: a mirror drift does produce output
printf '# drift\n' >> ~/ev6/etc/tlp.d/01-eco.conf
GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings harvest | head -3
```

## Expected

`genarch-settings harvest --help`: "Print the intent change that would match live. Writes nothing"

`CONTRIBUTING.md`: "`harvest` is the only verb that proposes intent changes; it never writes."

`AGENTS.md`: "`./target/debug/genarch-settings harvest` | the intent change that would match live"

With SYNC and CEIL drift present, `harvest` should propose the `[values]` / `ceil.toml` change (or
say that it cannot represent the class), and its empty output should not be identical to the
converged case.

## Actual

`harvest` writes 0 bytes to stdout and 0 bytes to stderr and exits 0 while `report` shows two
drift rows. A mirror or managed drift does produce a proposal (`--- intent: … (live differs)` with
a `+` diff), which shows the verb works for file classes only; the value classes (SYNC, CEIL) are
silently omitted. On a converged host `harvest` also prints nothing and exits 0, so a script
cannot tell "nothing to harvest" from "harvest does not cover this drift".

## Evidence

```
COMMAND: GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings report | grep -E "SYNC|CEIL|SUMMARY"
DRIFT SYNC vm.swappiness=10 want 150 findings#writeback
DRIFT CEIL policy0=4000000 want 3500000 findings#ceil
SUMMARY ok=4 accepted=0 warn=0 drift=3 skipped=4 exit=1

COMMAND: GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings harvest > ~/ev6.h.out 2> ~/ev6.h.err
exit=0 stdout_bytes=0 stderr_bytes=0

COMMAND: cat ~/ev6.h.out ~/ev6.h.err
(no output)

COMMAND: printf '# drift\n' >> ~/ev6/etc/tlp.d/01-eco.conf
COMMAND: GENARCH_SETTINGS_ROOT=~/ev6 genarch-settings harvest | head -3
--- intent: /etc/tlp.d/01-eco.conf (live differs)
+ # Eco-first policy — identical on AC and battery (owner's call: chassis is
+ # thermally capped, curbing peak boost wins both watts and sustained perf).
```

## What could not be determined

- Whether `harvest` is intended to cover only file classes (mirrors and managed records). No
  shipped sentence says so; the help and `CONTRIBUTING.md` describe it without a class limit.
- Whether a converged host's `harvest` should exit differently (it also exits 0 with no output).
