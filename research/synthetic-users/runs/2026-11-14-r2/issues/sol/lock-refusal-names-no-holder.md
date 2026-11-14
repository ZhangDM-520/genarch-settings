# The lock refusal names no holder: `apply lock held by  (/path/lock)`

**severity:** low
**category:** bug
**environment:** Arch Linux, genarch-settings 0.4.0-1, unprivileged, rehearsal copy `~/c`

## Intent

Find out which process holds the apply lock when a second apply is refused.

## Steps

```sh
cp -a ~/root.pristine ~/c
GENARCH_SETTINGS_ROOT=$HOME/c genarch-settings apply >c1.out 2>&1 & p1=$!
GENARCH_SETTINGS_ROOT=$HOME/c genarch-settings apply >c2.out 2>&1 & p2=$!
wait $p1; echo rc1=$?; wait $p2; echo rc2=$?
tail -3 c2.out
```

## Expected

A refusal that is diagnosable. The shipped material makes the lock part of the apply contract:

> `apply` performs exactly that plan, after a pre-image for every target, under an exclusive lock

The message says "held by", so the field after it should identify the holder (pid, uid, start
time), or the sentence should not promise one.

## Actual

The field is empty. The second apply correctly refuses with exit 2 and writes nothing, but an
operator has no way to find the process short of scanning `/proc`.

## Evidence

```
$ cat ~/ev/c2.out
SUMMARY applied=0 failed=1 exit=2
genarch-settings: apply lock held by  (/home/arch/c/var/lib/genarch-settings/lock)
```

The first apply completed normally (`SUMMARY planned=56 applied=56 failed=0 exit=0`).

## What could not be determined

Whether the lock file ever contains holder data; `cat ~/c/var/lib/genarch-settings/lock`
prints nothing readable, and the code was not available (by design of this review).
