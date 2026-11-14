# A failed apply writes 28 files, then reports `applied=0`

**severity:** high
**category:** bug
**environment:** Arch Linux, genarch-settings 0.4.0-1, unprivileged, rehearsal tree `~/root.pristine` copied to `~/h` (QEMU DMI, profile `vm-generic`)

## Intent

Make the outcome of a failed convergence legible, so the operator knows whether anything
landed and what has to be rolled back.

## Steps

```sh
rm -rf ~/h; cp -a ~/root.pristine ~/h
mkdir -p ~/h/etc/tlp.d && chmod a-w ~/h/etc/tlp.d   # one late target unwritable
GENARCH_SETTINGS_ROOT=$HOME/h genarch-settings apply
find ~/h/etc ~/h/usr/local ~/h/usr/lib/systemd/system-sleep -type f | wc -l
ls ~/h/var/lib/genarch-settings
```

## Expected

The shipped material says what the exit codes mean and that a partial apply is a partial
apply:

> Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3`
> partial.

> `apply` writes a dated pre-image beside every file it replaces, keeps the source mode
> and owner, and refuses on any target it cannot classify.

A partial apply (exit 3) should say how many changes landed, and the ownership record should
name the files that did land.

## Actual

`SUMMARY applied=0 failed=1 exit=3` while 34 files are on disk (6 before). The ownership
record does not exist, so those files are also outside the manifest.

## Evidence

```
$ GENARCH_SETTINGS_ROOT=$HOME/h genarch-settings apply
...
SUMMARY applied=0 failed=1 exit=3
genarch-settings: create /home/arch/h/etc/tlp.d/.01-eco.conf.4726.tmp: Permission denied (os error 13)
$ find ~/h/etc ~/h/usr/local ~/h/usr/lib/systemd/system-sleep -type f | wc -l
34
$ find ~/root.pristine/etc ~/root.pristine/usr/local ~/root.pristine/usr/lib/systemd/system-sleep -type f | wc -l
6
$ ls ~/h/var/lib/genarch-settings
$            # empty: no manifest.toml
```

The same accounting bug appears after a SIGKILL: 40 files present, `report` says
`OK PRUNE 0 owned path(s) in the manifest, none to remove` and `MIRROR 39 target(s) differ`.

## What could not be determined

Whether `applied=` is intended to count operations attempted after the first failure rather
than files written; either way the number is read as "nothing was written". Whether a
retry repairs the count (the next successful apply does write a manifest covering the files).
