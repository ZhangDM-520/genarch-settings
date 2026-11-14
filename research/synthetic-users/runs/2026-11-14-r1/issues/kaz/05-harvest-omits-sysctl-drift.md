# `harvest` prints nothing for a drifted sysctl although its help promises "the intent change that would match live"

- **Severity**: medium
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest; reproduction on the rehearsal tree `~/machine/root`, 2026-11-14.

## Intent

The verb's whole contract is its help line:

> Print the intent change that would match live. Writes nothing

`CONTRIBUTING.md` repeats: "`harvest` is the only verb that proposes intent changes; it never writes." A sysctl value is a knob whose value lives in the intent (`sysctl.toml`), so a drifted sysctl is exactly the case this verb exists for.

## Steps

1. `echo 60 > ~/machine/root/proc/sys/vm/swappiness`
2. `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report | grep SYNC`
3. `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings harvest | wc -c`
4. For contrast, in the same tree make a mirror differ and rerun `harvest` (it does print a proposal for that).

## Expected

A proposal to change the intent's `vm.swappiness` from `"150"` to `"60"` (the same kind of output the verb produces for a mirror or a managed record).

## Actual

`harvest` prints zero bytes. It handles mirror drift and managed-line drift, but it silently ignores the `SYNC` row and every other sysctl. The same run's `report` shows the drift, so the engine sees it; only `harvest` omits it.

## Evidence

```
$ echo 60 > ~/machine/root/proc/sys/vm/swappiness
$ cat ~/machine/root/proc/sys/vm/swappiness
60
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report | grep SYNC
DRIFT SYNC vm.swappiness=60 want 150 findings#writeback
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings harvest | wc -c
0
```

The help:

```
$ genarch-settings harvest --help
Print the intent change that would match live. Writes nothing

Usage: genarch-settings harvest

Options:
  -h, --help  Print help
```

For contrast, the same verb does propose a mirror change when one exists (from a separate run with a modified mirror target):

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings harvest
--- intent: /etc/tmpfiles.d/tmp.conf (live differs)
+ AAA
+ BBB
```

The sysctl drift still exists and is not mentioned.

## What could not be determined

- Whether sysctls are deliberately outside `harvest`'s scope; neither the help nor `CONTRIBUTING.md` says so.
- Whether the same omission applies to the `CEIL` row; CEIL is derived from live state rather than the intent, so it may legitimately have no proposal.
