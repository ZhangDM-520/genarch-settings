# `harvest`'s managed-record diff shows live→intent, the opposite direction of its mirror output and of the verb's contract

- **Severity**: medium
- **Category**: bug
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest; rehearsal tree `~/machine/root`, 2026-11-14.

## Intent

`harvest` exists to propose the intent change that would make the intent match a live machine:

> Print the intent change that would match live. Writes nothing

Its output should therefore be read as "put this into the intent". The mirror block behaves that way: it prints the **live** bytes with `+`, i.e. the content to adopt.

## Steps

1. In `~/machine/root/etc/systemd/journald.conf`, set the managed line to `SystemMaxUse=999M` (the intent record says `SystemMaxUse=256M`).
2. Make a mirror target differ (e.g. replace `~/machine/root/etc/tmpfiles.d/tmp.conf` with `AAA`/`BBB`).
3. `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings harvest`
4. Compare the direction of the mirror block with the managed block.
5. `genarch-settings harvest --help`

## Expected

For the managed record, the proposal should be the live value — something equivalent to changing the record's `line` from `SystemMaxUse=256M` to `SystemMaxUse=999M` — consistent with the mirror block printing the live content to adopt.

## Actual

The managed block prints `- SystemMaxUse=999M` / `+ SystemMaxUse=256M`: it shows the change `apply` would make (live→intent), not the intent change that would match live. A user following the output would edit the intent to the value it already has (or conclude there is nothing to change), while the live line stays `999M`. The two blocks in one run therefore tell opposite directions.

## Evidence

```
$ grep SystemMaxUse ~/machine/root/etc/systemd/journald.conf
SystemMaxUse=999M
$ grep '^line' ~/machine/root/usr/lib/genarch-settings/managed/journald.toml
line = "SystemMaxUse=256M"

$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings harvest
--- intent: /etc/tmpfiles.d/tmp.conf (live differs)
+ AAA
+ BBB
--- managed journald in /etc/systemd/journald.conf
- SystemMaxUse=999M
+ SystemMaxUse=256M
$ echo $?
0
```

The mirror block proposes the live bytes (`AAA`, `BBB`); the managed block proposes the intent's existing value (`256M`), the reverse of "match live".

## What could not be determined

- Whether the managed diff is intended to be a "what apply will change" preview rather than a harvest proposal; the verb's help and the mirror block say otherwise.
