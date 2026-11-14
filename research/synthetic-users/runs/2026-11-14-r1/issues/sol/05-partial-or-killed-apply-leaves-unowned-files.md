# A partial or killed apply leaves deployed files out of the ownership record, plus a stray temp file

**Severity:** Medium
**Category:** bug
**Environment:** Arch guest, rehearsal roots `~/ev2` and `~/ev6` (copies of a converged
`~/machine/root` with the manifest removed and the mirrors deleted), unprivileged apply.

## Intent

`CONTEXT.md`:

> `Manifest — the engine's ownership record (…). It is what makes "one writer per file" checkable and what lets a target leave the intent without being forgotten on disk (prune).`

`DESIGN.md`:

> `apply performs exactly that plan, after a pre-image for every target, under an exclusive lock, then re-plans and exits 1 if anything survived.`

A write that landed must be in the record, or the next apply cannot own, prune or report it; a
failure must not leave debris in a system directory.

## Steps

Partial apply (one unwritable directory after a writable mirror):

```sh
R=~/ev2; rm -rf $R; cp -a ~/r13 $R; rm -rf $R/var/lib/genarch-settings
rm -f $R/etc/NetworkManager/conf.d/99-eco-dns.conf $R/etc/modprobe.d/99-acp-power-gating.conf
chmod 555 $R/etc/modprobe.d
GENARCH_SETTINGS_ROOT=$R genarch-settings apply; echo "EXIT=$?"
ls -l $R/etc/NetworkManager/conf.d/99-eco-dns.conf
ls -l $R/var/lib/genarch-settings/
```

Kill mid-apply (rename delayed with `strace`, child killed):

```sh
R=~/ev6; rm -rf $R; cp -a ~/r13 $R   # manifest removed, mirrors deleted as above
(strace -f -e trace=rename -e inject=rename:delay_enter=2000000 \
   -o /home/arch/ev6.strace genarch-settings apply >/dev/null 2>&1 & echo $! >/home/arch/ev6.pid)
sleep 3; kill -9 $(pgrep -P $(cat /home/arch/ev6.pid) | head -1)
find $R -name "*.tmp*" -o -name ".*tmp*"
ls -l $R/var/lib/genarch-settings/
```

## Expected

The files that landed are in `owned` (or the state file is written before/atomically with the
writes), and a killed run leaves no temp file in `/etc`. `AGENTS.md` promises the temp is
`create_new` + `O_NOFOLLOW` + fsync + rename; a kill between create and rename necessarily
leaves the temp, but a later run should clean it.

## Actual

`apply` writes mirrors first and the manifest last. A failed or killed run leaves:

- a deployed mirror (`99-eco-dns.conf`) on disk with **no manifest at all** (`~/ev2`), and
- a hidden temp file `.99-wifi-powersave.conf.6445.tmp` in `/etc/NetworkManager/conf.d/`
  with no manifest (`~/ev6`).

A later successful apply re-plans and heals the ownership of the same intent, but the stray
temp is never removed — it survives a completed apply and a clean report. If the intent changes
between the failure and the next apply, the landed file is an orphan the engine neither reports
nor prunes.

## Evidence (verbatim)

```
genarch-settings: create /home/arch/ev2/etc/modprobe.d/.99-acp-power-gating.conf.6290.tmp: Permission denied (os error 13)
EXIT=3
--- landed but unowned:
-rw-r--r-- 1 arch arch 198 Nov 14 14:51 /home/arch/ev2/etc/NetworkManager/conf.d/99-eco-dns.conf
--- state dir:
total 4
-rw-r--r-- 1 arch arch 30 Nov 14 14:51 lock
```

```
strace=6442 child=6445
--- stray temp:
/home/arch/ev6/etc/NetworkManager/conf.d/.99-wifi-powersave.conf.6445.tmp
--- ownership record:
total 4
-rw-r--r-- 1 arch arch 30 Nov 14 14:53 lock
--- files landed before the kill:
2
```

After a completed apply, the temp is still present (checked in the earlier `~/r14` recovery):

```
/home/arch/r14/etc/NetworkManager/conf.d/.99-wifi-powersave.conf.5035.tmp
SUMMARY planned=0 applied=0 failed=0 exit=0
```

## What could not be determined

Whether the manifest write itself is atomic and whether a kill *during* that write truncates it
(simulating a truncated manifest makes `report` print `check 'MIRROR' failed to run` for three
rows and `apply` exit 2 with the parse error). Whether an operator can recover without manually
deleting the manifest was not tested beyond that.
