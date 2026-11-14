# Selecting a profile for another machine is a hard exit-3 refusal ("nothing was written"), not the documented "apply still runs and exits 1"

- **severity:** medium
- **category:** bug
- **environment:** Arch Linux guest (QEMU, 2 vCPU), `genarch-settings 0.4.0-1`; staging tree with `profile = "zenbook-s16"` selected. The staging tree has no `/etc/asusd/asusd.ron`, exactly as a non-ASUS laptop would not.

## Intent

Follow the documented behaviour for a profile that belongs to another machine: see the PROFILE
drift, let `apply` run, and read the exit code the operator scripts branch on.

## Steps

```sh
cp -a ~/machine/root ~/ev4
printf 'profile = "zenbook-s16"\n' > ~/ev4/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings report | head -2
GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings apply; echo "exit=$?"
GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings apply --dry-run; echo "exit=$?"

# prove nothing was written
cp -a ~/ev4 ~/ev5
rm -f ~/ev5/var/lib/genarch-settings/manifest.toml
find ~/ev5 -name '*.bak-*' -delete
find ~/ev5 -name '*.bak-*' | wc -l
GENARCH_SETTINGS_ROOT=~/ev5 genarch-settings apply; echo "exit=$?"
find ~/ev5 -name '*.bak-*' | wc -l
ls -la ~/ev5/var/lib/genarch-settings
```

## Expected

`install.sh` usage: "Selecting one for another laptop is not an error: `genarch-settings report`
says PROFILE DRIFT and exits 1, and `apply` still runs, because the operator has to be able to see
it and fix it."

`AGENTS.md`: "A profile's `[requires]` mismatch is **PROFILE drift (exit 1)**, not a load error."

`DESIGN.md`: "`apply` inherits the drift rule too: a profile that does not match this machine exits
1 after the apply, because the writes succeeded against another machine's facts."

Exit codes (`--help`, `DESIGN.md`): "0 clean / 1 drift / 2 precondition / 3 partial"; 3 means
"apply step failed (partial)".

## Actual

`apply` refuses before doing anything:

```
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
exit=3
```

`apply --dry-run` also exits 3. No pre-images and no manifest are created (the count of
`*.bak-*` is 0 before and after; `var/lib/genarch-settings` holds only `lock`).

`apply` gets "partial failure" (3) for a run in which nothing was even attempted, and the
documented "apply still runs" path is unreachable for any profile whose managed targets are not
installed on this machine — which is exactly the cross-machine selection the sentence describes. A
self-contained mismatched profile does follow the documented path (it runs and exits 1; the
contrast below is on an already-converged copy, and the first run on a pre-converged copy planned
56 writes and also exited 1), so the contradiction is triggered by the missing managed target.

## Evidence

```
COMMAND: GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings report | head -2
DRIFT PROFILE profile 'zenbook-s16' is not for this machine: product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA; sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC. findings#intent
DRIFT MIRROR 2 target(s) differ: /etc/udev/rules.d/61-eco-ioschedulers.rules /etc/udev/rules.d/98-bt-autosuspend.rules findings#intent

COMMAND: GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings apply; echo "exit=$?"
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
exit=3

COMMAND: GENARCH_SETTINGS_ROOT=~/ev4 genarch-settings apply --dry-run; echo "exit=$?"
exit=3

COMMAND: find ~/ev5 -name '*.bak-*' | wc -l
0
COMMAND: GENARCH_SETTINGS_ROOT=~/ev5 genarch-settings apply; echo "exit=$?"
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
exit=3
COMMAND: find ~/ev5 -name '*.bak-*' | wc -l
0
COMMAND: ls -la ~/ev5/var/lib/genarch-settings
total 4
drwxr-xr-x 1 arch arch  8 Nov 14 14:50 .
drwxr-xr-x 1 arch arch 32 Nov 14 14:35 ..
-rw-r--r-- 1 arch arch 30 Nov 14 14:49 lock
```

For contrast, the documented mismatch path with a profile that has no missing managed target:

```
COMMAND: cp -a ~/machine/root ~/ev4b
COMMAND: printf '[requires]\nproduct = "SOME OTHER LAPTOP"\nsys_vendor = "SOMEONE ELSE"\n' \
           > ~/ev4b/etc/genarch-settings/profiles/other/intent.toml
COMMAND: printf 'profile = "other"\n' > ~/ev4b/etc/genarch-settings/profile
COMMAND: GENARCH_SETTINGS_ROOT=~/ev4b genarch-settings apply
PROFILE other
  PROFILE-MISMATCH product is "Standard PC (i440FX + PIIX, 1996)", profile requires SOME OTHER LAPTOP
  PROFILE-MISMATCH sys_vendor is "QEMU", profile requires SOMEONE ELSE
PLAN 0 change(s)
APPLY-SCOPE profile=other full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
SUMMARY planned=0 applied=0 failed=0 exit=1
exit=1
```

## What could not be determined

- Whether an absent managed target is meant to be "unclassifiable" (the shipped rule speaks of
  zero or several *anchor matches*, not of a missing file). Refusing may be deliberate; the
  observable bug is that the result is 3 (partial) with nothing written and that the documented
  cross-machine behaviour does not happen.
- Whether the engine should skip a managed record whose target package is not installed; no
  shipped sentence covers that case.
