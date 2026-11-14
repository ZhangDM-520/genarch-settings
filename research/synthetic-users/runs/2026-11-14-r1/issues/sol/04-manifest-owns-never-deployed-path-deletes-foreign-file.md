# The manifest claims ownership of mirrors a fresh install never wrote; removing the profile then deletes a foreign file at that path

**Severity:** Medium
**Category:** bug
**Environment:** Arch guest, rehearsal root `~/ev1` (a copy of a converged
`~/machine/root`), profile `vm-generic` selected, all shipped profiles present.
`~/ev1` is rebuilt from scratch below so the sequence is the whole story.

## Intent

`AGENTS.md` states the ownership rule the whole design rests on:

> `apply cannot delete a file it does not own; it prunes only what the manifest lists`

and the dormant rule:

> `A deselected profile's mirrors stay owned (dormant) and are never pruned`

"Dormant" is about a path that *was* owned and whose profile stopped being selected. A path the
engine never wrote is not owned, and a user file at that path is not its to delete.

## Steps

```sh
R=~/ev1; rm -rf $R; cp -a ~/r14 $R; rm -rf $R/var/lib/genarch-settings
export GENARCH_SETTINGS_ROOT=$R
genarch-settings apply >/dev/null
grep -o "/etc/udev/rules.d/98-bt-autosuspend.rules" $R/var/lib/genarch-settings/manifest.toml
ls $R/etc/udev/rules.d/98-bt-autosuspend.rules          # never written
echo "FOREIGN CONTENT" > $R/etc/udev/rules.d/98-bt-autosuspend.rules
rm -rf $R/usr/lib/genarch-settings/profiles/zenbook-s16
genarch-settings report | grep PRUNE
genarch-settings apply | grep -E "PRUNE|SUMMARY"
cat $R/etc/udev/rules.d/98-bt-autosuspend.rules.bak-*
```

## Expected

The first apply records only what it deployed (the base mirrors + sysctl projection). The
`zenbook-s16` profile is installed but not selected, so its `98-bt-autosuspend.rules` is not
owned; a file a user places at that path is theirs.

## Actual

On the first apply the manifest already lists `/etc/udev/rules.d/98-bt-autosuspend.rules`
although that file does not exist: the engine treats **every installed profile's** mirrors as
owned (dormant) from the start. When the profile directory later disappears, the path is
"no longer shipped" and `apply` prunes it — deleting the user's file and leaving only a dated
backup.

## Evidence (verbatim)

```
--- manifest claims this path:
/etc/udev/rules.d/98-bt-autosuspend.rules
--- but the file does not exist:
ls: cannot access '/home/arch/ev1/etc/udev/rules.d/98-bt-autosuspend.rules': No such file or directory
--- user creates their own file there:
--- remove the installed profile, re-apply:
DRIFT PRUNE 1 path(s) to remove at apply: /etc/udev/rules.d/98-bt-autosuspend.rules findings#intent
  PRUNE /etc/udev/rules.d/98-bt-autosuspend.rules (no longer shipped)
PRUNED /etc/udev/rules.d/98-bt-autosuspend.rules
SUMMARY planned=1 applied=0 failed=0 exit=0
--- file after:
-rw-r--r-- 1 arch arch   16 Nov 14 14:51 98-bt-autosuspend.rules.bak-20261114
FOREIGN CONTENT
```

The same manifest entry also appeared on a root where **no** profile was selected and the file
had never been deployed (`~/r2` copy, fresh manifest):

```
owned = [... "/etc/udev/rules.d/98-bt-autosuspend.rules", ...]
```

## What could not be determined

Whether a package upgrade can remove a profile directory. `install.sh` does
`rm -rf "$INTENT_DIR/profiles"` before copying, so a release that drops a profile, or an
operator pruning the tree, reaches this path. The deletion is recoverable from the `.bak`, but
the file was never the engine's to delete.
