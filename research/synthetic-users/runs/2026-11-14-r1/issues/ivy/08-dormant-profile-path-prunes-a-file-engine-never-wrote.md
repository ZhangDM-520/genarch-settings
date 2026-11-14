# A never-selected profile's mirror is recorded as owned, and deleting the profile prunes a file the engine never wrote

**Severity:** high
**Category:** bug

**Environment:** Arch guest; `genarch-settings 0.4.0-1`; a fresh copy of the staging tree
(`~/machine/root.dormant2`, a copy of `root.pristine`, which is a byte copy of the delivered
`~/machine/root` taken before any apply) with the operator profile `vm-generic` selected. The
package ships seven model profiles; only `zenbook-s16` has a mirror
(`/etc/udev/rules.d/98-bt-autosuspend.rules`).

**Intent:** Install on a headless box, select my own profile, and later remove the unused
laptop profile (or upgrade past it). A file the engine never wrote must never be deleted. The
material says so twice:

> `apply` cannot delete a file it does not own; it prunes only what the manifest lists ... That
> is why unmanaging something is safe

> Nothing is deleted on a hunch.

**Steps (reproduced from a pristine staging copy):**

```sh
cp -a ~/machine/root.pristine ~/machine/root.dormant2
export GENARCH_SETTINGS_ROOT=$HOME/machine/root.dormant2

# 1. converge with the vm-generic profile; the zenbook-s16 profile is merely deselected
genarch-settings apply > ~/machine/d2-apply1.out 2>&1

# 2. prove the engine never wrote the 98-bt path: no OWNED line, but it is in the manifest
grep -c 'OWNED.*98-bt' ~/machine/d2-apply1.out
grep -c '98-bt' ~/machine/root.dormant2/var/lib/genarch-settings/manifest.toml

# 3. create a file at that path myself (the engine has never written it)
printf 'USER CREATED FILE\n' > ~/machine/root.dormant2/etc/udev/rules.d/98-bt-autosuspend.rules

# 4. the profile stops shipping it (delete the unused profile directory, as an upgrade could)
rm -rf ~/machine/root.dormant2/usr/lib/genarch-settings/profiles/zenbook-s16

# 5. report and apply
genarch-settings report > ~/machine/d2-report.out 2>&1
genarch-settings apply  > ~/machine/d2-apply2.out 2>&1
grep PRUNE ~/machine/d2-report.out
grep -E 'PRUNE|SUMMARY' ~/machine/d2-apply2.out
ls ~/machine/root.dormant2/etc/udev/rules.d/98-bt-autosuspend.rules
cat ~/machine/root.dormant2/etc/udev/rules.d/98-bt-autosuspend.rules.bak-20261114
```

**Expected:** The CONTEXT definition of the manifest:

> **Manifest** — the engine's ownership record ... It is what makes "one writer per file"
> checkable and what lets a target leave the intent without being forgotten on disk
> (**prune**). ... A path belonging to a profile that is merely not selected stays owned
> (**dormant**), so a lost selection cannot delete a system file.

and AGENTS:

> A **deselected** profile's mirrors stay owned (dormant) and are never pruned ... `Intent::dormant`
> keeps them in `owned_targets`; only a profile that stops shipping a path hands it back.

"Stays owned" presupposes the engine owned it. A profile that was never selected and never
applied should not put its mirror in `owned`, and a path the engine never wrote should not be
prunable.

**Actual:** The first apply records `/etc/udev/rules.d/98-bt-autosuspend.rules` in the
manifest's `owned` list even though it deployed nothing there (the plan has no `OWNED` line for
it). When the profile directory is removed, the next `report` says
`DRIFT PRUNE 1 path(s) to remove at apply`, and the next `apply` prints
`PRUNE /etc/udev/rules.d/98-bt-autosuspend.rules (no longer shipped)` and deletes my file. A
pre-image is taken, so the bytes survive in a `.bak`, but the live file is gone. The run's
summary says `planned=1 applied=0` even though it deleted a file.

**Evidence:**

```
APPLY1=0
OWNED lines mentioning 98:
0
manifest mentions 98:
1
manifest line:
 "/etc/udev/rules.d/98-bt-autosuspend.rules"
REPORT=1
DRIFT PRUNE 1 path(s) to remove at apply: /etc/udev/rules.d/98-bt-autosuspend.rules findings#intent
APPLY2=0
PLAN 1 change(s)
  PRUNE /etc/udev/rules.d/98-bt-autosuspend.rules (no longer shipped)
APPLY-SCOPE profile=vm-generic full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
BACKUPS 1
  /etc/udev/rules.d/98-bt-autosuspend.rules.bak-20261114
PRUNED /etc/udev/rules.d/98-bt-autosuspend.rules
SUMMARY planned=1 applied=0 failed=0 exit=0
live file:
ls: cannot access '/home/arch/machine/root.dormant2/etc/udev/rules.d/98-bt-autosuspend.rules': No such file or directory
bak:
USER CREATED FILE
```

The first apply's plan (same run) contains no line for the 98 rule; the manifest is the only
place the path appears:

```
$ grep -c 'OWNED.*98-bt' ~/machine/d2-apply1.out
0
$ grep -c '98-bt' ~/machine/root.dormant2/var/lib/genarch-settings/manifest.toml
1
```

**Why it matters:** `/etc/udev/rules.d` is exactly where an operator or another package puts
files. The manifest claims ownership of a path the engine never touched because a *deselected*
profile ships it; deleting that profile (an upgrade that drops a model, or housekeeping) turns
the dormant claim into a delete. The backup makes it recoverable, but the file disappears from
its live path and the run reports `applied=0`. This is the "one writer per file" rule inverted:
the engine becomes a second writer by claiming a path it never wrote.

**What I could not determine:** whether the intended fix is to record a dormant path only when
the manifest already lists it (i.e., it was written once during a period when the profile was
selected), or to verify the file's bytes against a known pre-image before pruning. Whether an
upgrade of the package would ever drop a profile directory in practice (the package ships all
seven today). Whether a real host has a file at the 98-bt path (the VM does not; I created one
to make the deletion observable).
