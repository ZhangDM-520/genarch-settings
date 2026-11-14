# The package's shipped profile selection is applied on a machine whose DMI does not match

**Severity:** High
**Category:** bug
**Environment:** Arch guest. The package ships `profile = "zenbook-s16"`. Rehearsal roots
`~/r18` and `~/r18b` (copies of a converged `~/machine/root`) with QEMU DMI, the Zenbook profile
selected, and an `/etc/asusd/asusd.ron` present (simulating any Asus host that has asusd). The
apply output below is the `~/r18b` repeat; `~/r18` behaved identically.

## Intent

The shipped `SCOPE.md` promises that a package built for one model does not configure another:

> `Not a distribution — the package is built from this tree and ships the model's profile (not one unit's identity). It selects nothing on a host whose DMI does not match (`PROFILE` drift, exit 1).`

## Steps

```sh
R=~/r18; cp -a ~/r14 $R   # ~/r14 is a converged copy of ~/machine/root; its DMI is QEMU
printf 'profile = "zenbook-s16"\n' > $R/etc/genarch-settings/profile
mkdir -p $R/etc/asusd
cat > $R/etc/asusd/asusd.ron <<'RON'
(
    ac_command: "",
    bat_command: "",
    platform_profile_linked_epp: true,
    change_platform_profile_on_ac: true,
    change_platform_profile_on_battery: true,
)
RON
GENARCH_SETTINGS_ROOT=$R genarch-settings report
GENARCH_SETTINGS_ROOT=$R genarch-settings apply; echo "EXIT=$?"
cat $R/etc/asusd/asusd.ron
ls -l $R/etc/udev/rules.d/98-bt-autosuspend.rules
```

## Expected

From `SCOPE.md`: the selection should be inert on a non-matching host (PROFILE drift, exit 1,
nothing written). From `DESIGN.md` the author knows this is not what happens:

> `apply inherits the drift rule too: a profile that does not match this machine exits 1 after the apply, because the writes succeeded against another machine's facts.`

But `DESIGN.md` is not the promise `SCOPE.md` makes to an operator, and the installer runs
`apply` unattended.

## Actual

`apply` writes the Zenbook profile's facts into the foreign machine's tree and only then exits
1: it installs the MT7925 Bluetooth rule and patches three anchors in `/etc/asusd/asusd.ron`.
The package's `post_install` runs exactly this command and discards the exit code (`... apply || true`),
so a host with asusd/ppd but a different DMI gets another model's configuration silently.

## Evidence (verbatim)

```
DRIFT PROFILE profile 'zenbook-s16' is not for this machine: product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA; sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC. findings#intent
DRIFT MIRROR 2 target(s) differ: /etc/udev/rules.d/61-eco-ioschedulers.rules /etc/udev/rules.d/98-bt-autosuspend.rules findings#intent
DRIFT MANAGED 3 record(s) drifted: asusd-epp asusd-profile-ac asusd-profile-bat findings#managed

=== dry-run ===
  PROFILE-MISMATCH product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA
  PROFILE-MISMATCH sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC.
PROFILE zenbook-s16
PLAN 5 change(s)
  OWNED /etc/udev/rules.d/61-eco-ioschedulers.rules
  OWNED /etc/udev/rules.d/98-bt-autosuspend.rules
  MANAGED /etc/asusd/asusd.ron (asusd-epp)
  MANAGED /etc/asusd/asusd.ron (asusd-profile-ac)
  MANAGED /etc/asusd/asusd.ron (asusd-profile-bat)
APPLY-SCOPE profile=zenbook-s16 full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
=== real apply ===
  PROFILE-MISMATCH product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA
  PROFILE-MISMATCH sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC.
PROFILE zenbook-s16
PLAN 5 change(s)
  OWNED /etc/udev/rules.d/61-eco-ioschedulers.rules
  OWNED /etc/udev/rules.d/98-bt-autosuspend.rules
  MANAGED /etc/asusd/asusd.ron (asusd-epp)
  MANAGED /etc/asusd/asusd.ron (asusd-profile-ac)
  MANAGED /etc/asusd/asusd.ron (asusd-profile-bat)
APPLY-SCOPE profile=zenbook-s16 full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
BACKUPS 4
  /etc/udev/rules.d/61-eco-ioschedulers.rules.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
  /etc/asusd/asusd.ron.bak-20261114
RELOAD udevadm control --reload
SUMMARY planned=5 applied=5 failed=0 exit=1
EXIT=1
=== asusd.ron now ===
(
    ac_command: "",
    bat_command: "",
    platform_profile_linked_epp: false,
    change_platform_profile_on_ac: false,
    change_platform_profile_on_battery: false,
)
=== bt rule ===
-rw-r--r-- 1 arch arch 175 Nov 14 14:45 /home/arch/r18/etc/udev/rules.d/98-bt-autosuspend.rules
```

The shipped selection is a package file:

```
$ cat /etc/genarch-settings/profile
profile = "zenbook-s16"
```

The same wrong-profile writes happen from the **scheduled pass**, which prints no mismatch at
all (`~/r18b`, QEMU DMI, Zenbook selected):

```
$ GENARCH_SETTINGS_ROOT=~/r18b genarch-settings converge boot
genarch-settings: CEIL DRIFT policy0 3000000 -> 4700000
genarch-settings: CEIL DRIFT policy1 3000000 -> 4700000
...
genarch-settings: smu: 12 of 12 ryzenadj call(s) failed — firmware limits may still stand
EXIT=0
--- ceiling written? ---
4700000
```

`apply` at least prints `PROFILE-MISMATCH`; the boot/resume hook does not, and it is the unit
that runs unattended on every boot and resume.

## What could not be determined

Whether the package is intended only for the vendor laptop. The contradiction inside the shipped
material (`SCOPE.md` says "selects nothing"; `DESIGN.md` and the engine say "apply still runs")
means an operator cannot know which promise holds. The payload cannot demonstrate the write on
the real `/etc` (the real box lacks asusd), so the reproduction uses a rehearsal root that has it.
