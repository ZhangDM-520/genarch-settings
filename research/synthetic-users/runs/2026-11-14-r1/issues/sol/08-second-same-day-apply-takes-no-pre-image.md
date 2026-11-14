# A second apply on the same day takes no pre-image; the only backup still holds the state before the first change

**Severity:** Medium
**Category:** bug
**Environment:** Arch guest, rehearsal root `~/ev3` (a converged copy). The managed record on
`/etc/systemd/journald.conf` is changed by hand twice in one day.

## Intent

`CONTEXT.md`:

> `Pre-image — the dated .bak-YYYYMMDD copy taken before a write. Never deleted.`
> `Every write is preceded by a pre-image and applied atomically.`

`AGENTS.md`:

> `Pre-images before every write, dated, never deleted`

The pre-image is the only rollback path: a bad managed patch or mirror change is restored by
copying the `.bak` back.

## Steps

```sh
R=~/ev3; rm -rf $R; cp -a ~/r14 $R
F=$R/etc/systemd/journald.conf
sed -i 's/^SystemMaxUse=.*/SystemMaxUse=FIRST/' $F
GENARCH_SETTINGS_ROOT=$R genarch-settings apply >/dev/null
grep SystemMaxUse $F.bak-20261114          # FIRST
sed -i 's/^SystemMaxUse=.*/SystemMaxUse=SECOND/' $F
GENARCH_SETTINGS_ROOT=$R genarch-settings apply >/dev/null
grep SystemMaxUse $F.bak-20261114          # expected SECOND, actually FIRST
stat -c '%y' $F.bak-20261114
```

## Expected

Every write has a pre-image of the bytes it replaces. The second apply replaces `SECOND`, so a
dated copy of `SECOND` exists (either a new name, or the date-qualified one is the one *this*
write replaced). The doc's "before every write" cannot be satisfied by a backup that predates
the first write.

## Actual

The second apply reuses the existing `.bak-20261114` and **does not take a new pre-image**; the
backup's mtime does not move and it still contains `FIRST`. The bytes that the second write
overwrote (`SECOND`) are gone. The apply reports success.

## Evidence (verbatim)

```
--- backup after first apply:
SystemMaxUse=FIRST
--- file just before second apply was SECOND; backups now:
/home/arch/ev3/etc/systemd/journald.conf
/home/arch/ev3/etc/systemd/journald.conf.bak-20261114
SystemMaxUse=FIRST
--- mtime of backup:
2026-11-14 14:51:55.332754457 +0000
```

The second apply's summary:

```
SUMMARY planned=1 applied=1 failed=0 exit=0
```

The collision is visible inside one apply too: the Zenbook profile patches three anchors of
`/etc/asusd/asusd.ron`, and the backup list names the same dated path three times:

```
BACKUPS 4
  /etc/udev/rules.d/61-eco-ioschedulers.rules.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
```

(Checked: the file's content after that apply is still the original, so the first writer wins
inside a single apply; only the *next day's* second apply loses the intermediate state.)

## What could not be determined

Which behaviour the author intended: "never deleted" argues for keeping the oldest copy, while
"before every write" argues for a fresh copy per write. A timestamp or a counter in the name
would satisfy both; a date-only name cannot. Whether `.bak-YYYYMMDD` collisions also happen on
the *same* target from two applies on one day (shown) versus two different targets (different
names) was checked only for the same target.
