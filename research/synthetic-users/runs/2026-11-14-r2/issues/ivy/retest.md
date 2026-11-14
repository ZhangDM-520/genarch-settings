# Round 2 retest — genarch-settings 0.4.0-1

Box: Arch Linux, QEMU/KVM (`Standard PC (i440FX + PIIX, 1996)`, `sys_vendor=QEMU`),
2 vCPU, headless, passwordless sudo. Package files: `~/payload`. Staging tree: `~/machine/root`
(QEMU DMI, two cpufreq policies, operator profile `vm-generic`). No real SMU and no wireless
interface on this box; rows needing that hardware say SKIPPED.

Verdict summary:

| # | Round-1 title | Verdict |
|---|---|---|
| 1 | pacman install exits 0 and deploys nothing | different |
| 2 | one unclassifiable managed record aborts the whole apply | still |
| 3 | an unusable intent exits 3 (partial) although nothing was written | different |
| 4 | boot and resume exit 0 when every SMU write failed | gone |
| 5 | harvest prints nothing for a sysctl drift report calls DRIFT | gone |
| 6 | shipped docs point at docs/PROFILES.md, the findings ledger and a PKGBUILD that are not installed | different |
| 7 | uninstall leaves the deployed files, units and manifest with no documented restore | different |
| 8 | a never-selected profile mirror is recorded as owned; deleting the profile prunes a file the engine never wrote | gone |

---

## 1. pacman install exits 0 and deploys nothing — **different**

The package payload now deploys, and the install scriptlet now runs `apply` and prints its
failure loudly. What did **not** change: `pacman -U` still exits 0 while nothing on the live
machine was converged (on this box it refuses before writing anything) and the post-install
`systemctl enable` fails. The failure is visible on stdout but not in the exit status.

Reproduction (fresh install from the payload package):

```
$ sudo pacman -R --noconfirm genarch-settings
$ sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
INSTALL_EXIT=0
[...]
installing genarch-settings...
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
genarch-settings: apply did not converge this host (see the message above)
genarch-settings: the package is installed; run 'genarch-settings report' to see why
Failed to enable unit: Unit genarch-settings.service does not exist
:: Running post-transaction hooks...
(1/1) Arming ConditionNeedsUpdate...
```

Package files are present after the install (the old "deploys nothing" symptom is fixed):

```
$ ls -la /etc/genarch-settings/profile /usr/lib/genarch-settings/intent.toml /usr/bin/genarch-settings
-rw-r--r-- 1 root root      24 Nov 14 15:24 /etc/genarch-settings/profile
-rwxr-xr-x 1 root root 3050872 Nov 14 15:24 /usr/bin/genarch-settings
-rw-r--r-- 1 root root     854 Nov 14 15:24 /usr/lib/genarch-settings/intent.toml
$ cat /etc/genarch-settings/profile
profile = "zenbook-s16"
$ sudo ls -la /var/lib/genarch-settings
ls: cannot access '/var/lib/genarch-settings': No such file or directory
$ genarch-settings report | tail -3
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
OK PRUNE 0 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=3 accepted=0 warn=0 drift=5 skipped=3 exit=1
```

Leftover defect filed as issue `02-pacman-install-exits-0-when-apply-failed.md`.

---

## 2. one unclassifiable managed record aborts the whole apply — **still**

Same reproduction and same bad outcome: one absent managed target refuses the entire plan, exit 3,
`nothing was written`. The README now documents this refusal, but it is still the behaviour, and
on a host with a **matching** profile the base layer's `ppd-conflict` record causes it on its own
(filed as issue 01).

```
$ cp -a ~/machine/root.pristine ~/machine/root.item2
$ echo 'profile = "zenbook-s16"' > ~/machine/root.item2/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.item2 genarch-settings apply
APPLY_EXIT=3
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
$ ls ~/machine/root.item2/var/lib/genarch-settings 2>&1
(no output; directory absent)
```

With the matching `vm-generic` profile but the base layer's PPD target removed, the same refusal
happens with no profile record involved:

```
$ cp -a ~/machine/root.pristine ~/machine/root.ppd
$ rm -f ~/machine/root.ppd/usr/lib/systemd/system/power-profiles-daemon.service
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings apply
EXIT=3
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
```

---

## 3. an unusable intent exits 3 (partial) although nothing was written — **different**

A malformed intent now exits **2** (precondition), which is the right class. The residual case is
an intent that parses but cannot be classified (an absent managed target): that still exits 3
("partial") with nothing written — and the same happens under `--dry-run`. The dry-run side is
filed as issue 03.

Malformed TOML:

```
$ cp -a ~/machine/root.pristine ~/machine/intent-broken
$ printf '\nthis is not toml\n' >> ~/machine/intent-broken/usr/lib/genarch-settings/sysctl.toml
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine \
    GENARCH_SETTINGS_INTENT=$HOME/machine/intent-broken/usr/lib/genarch-settings \
    genarch-settings apply --dry-run
EXIT=2
SUMMARY applied=0 failed=1 exit=2
genarch-settings: parse /home/arch/machine/intent-broken/usr/lib/genarch-settings/sysctl.toml: TOML parse error at line 21, column 6
   |
21 | this is not toml
   |      ^
expected `.`, `=`
```

Still exit 3, nothing written, when the intent parses but cannot be classified:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.item2 genarch-settings apply --dry-run
DRY_EXIT=3
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  [...]  (the five asusd record lines quoted in item 2)
nothing was written
```

---

## 4. boot and resume exit 0 when every SMU write failed — **gone**

Both passes now exit 3 and say every ryzenadj call failed. The staging tree's rehearsal stub makes
each call fail, which is exactly the condition under test.

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine genarch-settings converge boot
BOOT_EXIT=3
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (-1): rehearsal root: not executing ryzenadj
genarch-settings: ryzenadj failed: ryzenadj --power-saving (-1): rehearsal root: not executing ryzenadj
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: all 2 ryzenadj call(s) failed — no SMU setting was applied
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine genarch-settings converge resume
RESUME_EXIT=3
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (-1): rehearsal root: not executing ryzenadj
genarch-settings: ryzenadj failed: ryzenadj --power-saving (-1): rehearsal root: not executing ryzenadj
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: all 2 ryzenadj call(s) failed — no SMU setting was applied
```

---

## 5. harvest prints nothing for a sysctl drift report calls DRIFT — **gone**

`harvest` now prints per-knob proposals for the same SYNC row the report calls DRIFT.

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine genarch-settings report | grep SYNC
DRIFT SYNC fs.file-max=9223372036854775807 want 2097152 kernel.hung_task_panic=0 want 1 [...] vm.swappiness=60 want 150 [...]
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine genarch-settings harvest
EXIT=0
--- sysctl fs.file-max (live 9223372036854775807)
- 2097152
+ 9223372036854775807
--- sysctl kernel.hung_task_panic (live 0)
- 1
+ 0
--- sysctl vm.swappiness (live 60)
- 150
+ 60
[...]
--- ceil policy0 (live 4000000)
- 3000000
+ 4000000
```

(The full output also lists every missing mirror, the two managed records, all 14 sysctls and
both ceil policies.)

---

## 6. shipped docs point at docs/PROFILES.md, the findings ledger and a PKGBUILD that are not installed — **different**

Two of the three are addressed: `PROFILES.md` is installed flat and the README now says the ledger
is not shipped, so its anchors name sections rather than local files. Still broken as installed:

- the README's two links are literally `docs/PROFILES.md` and there is no `docs/` directory;
- the README's Documentation list promises `docs/CUTOVER.md` ("the runbook ... including
  rollback") and that file is not installed at all;
- `install.sh` says "the PKGBUILD beside this file is the supported path" and no PKGBUILD is
  installed beside it.

```
$ ls /usr/share/doc/genarch-settings/docs
ls: cannot access '/usr/share/doc/genarch-settings/docs': No such file or directory
$ ls /usr/share/doc/genarch-settings
AGENTS.md  CONTEXT.md  CONTRIBUTING.md  DESIGN.md  LICENSE  PROFILES.md  README.md  SCOPE.md  install.sh
$ grep -n '](docs/' /usr/share/doc/genarch-settings/README.md
9:instead, is [`docs/PROFILES.md`](docs/PROFILES.md).
82:belong instead, is [`docs/PROFILES.md`](docs/PROFILES.md). Without pacman — from this
$ grep -n 'docs/CUTOVER.md' /usr/share/doc/genarch-settings/README.md
143:- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
$ grep -n PKGBUILD /usr/share/doc/genarch-settings/install.sh
6:# binary outside the package manager; on Arch the PKGBUILD beside this file is the supported
```

Filed as issue `04-installed-docs-reference-absent-cutover-and-pkgbuild.md`.

---

## 7. uninstall leaves the deployed files, units and manifest with no documented restore — **different**

The README now has a Removal section that says what stays, and `pre_remove` prints a reminder.
There is still no restore procedure (only "delete those paths yourself" / pre-images named), and
the portable installer's own uninstall text omits deployed files entirely. On this box no apply
ever succeeded, so removal leaves no deployed files to inspect and no units were ever enabled (the
disable step could not be observed); the message still claims a manifest is present.

```
$ sudo pacman -R --noconfirm genarch-settings
EXIT=0
genarch-settings: deployed files listed in /var/lib/genarch-settings/manifest.toml were left in place
genarch-settings: pre-images (*.bak-YYYYMMDD) sit beside every file the engine replaced
$ ls /usr/lib/genarch-settings /usr/share/doc/genarch-settings /etc/genarch-settings/profile
ls: cannot access '/usr/lib/genarch-settings': No such file or directory
ls: cannot access '/usr/share/doc/genarch-settings': No such file or directory
ls: cannot access '/etc/genarch-settings/profile': No such file or directory
$ find /etc/genarch-settings /var/lib/genarch-settings -type f 2>/dev/null
(no output)
```

README Removal (installed copy):

```
`pacman -R genarch-settings` disables the units and removes the binary, the intent and the
documents. It does **not** remove what the engine deployed: every file listed in
`/var/lib/genarch-settings/manifest.toml` stays on disk, with a `<file>.bak-YYYYMMDD` pre-image
beside anything it replaced, and a managed record leaves the line it patched in place. Read the
manifest before removing; if the removal is permanent, delete those paths yourself.
```

`install.sh --help` still says only:

```
Uninstall: `systemctl disable` the units `genarch-settings units` prints, remove
the binary, the intent directory and the documentation, then run `systemctl daemon-reload`.
```

---

## 8. a never-selected profile mirror is recorded as owned; deleting the profile prunes a file the engine never wrote — **gone**

Both halves now behave correctly.

A profile that is present but **not selected** contributes no owned target: the base-only apply
never records the zenbook-only `98-bt-autosuspend.rules`.

```
$ cp -a ~/machine/root.pristine ~/machine/root.base
$ rm -f ~/machine/root.base/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.base genarch-settings apply
APPLY=0
SUMMARY planned=56 applied=56 failed=0 exit=0
$ grep -c '98-bt-autosuspend' ~/machine/root.base/var/lib/genarch-settings/manifest.toml
0
```

A file the engine never wrote is not pruned when a profile directory disappears:

```
$ cp -a ~/machine/root.base ~/machine/root.never
$ P=~/machine/root.never/etc/genarch-settings/profiles/never
$ mkdir -p $P/mirror/etc
$ printf 'status = "measured"\n[requires]\nproduct = "Standard PC (i440FX + PIIX, 1996)"\nsys_vendor = "QEMU"\n' > $P/intent.toml
$ echo operator-owned > $P/mirror/etc/zz-never.conf
$ echo operator-owned > ~/machine/root.never/etc/zz-never.conf
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.never genarch-settings apply >/dev/null 2>&1
APPLY1=0
$ grep -c zz-never ~/machine/root.never/var/lib/genarch-settings/manifest.toml
0
$ rm -rf $P
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.never genarch-settings apply
APPLY2=0
$ cat ~/machine/root.never/etc/zz-never.conf
operator-owned
```

Deselecting a profile leaves its deployed mirror owned (dormant), and deleting the profile after
that prunes only the file the engine itself wrote:

```
$ cp -a ~/machine/root.pristine ~/machine/root.item8
$ P=~/machine/root.item8/etc/genarch-settings/profiles/mirrortest
$ mkdir -p $P/mirror/etc
$ cp ~/machine/root.pristine/etc/genarch-settings/profiles/vm-generic/{ceil.toml,passes.toml,ryzenadj.toml} $P/
$ printf 'status = "measured"\n[requires]\nproduct = "Standard PC (i440FX + PIIX, 1996)"\nsys_vendor = "QEMU"\n' > $P/intent.toml
$ echo x=1 > $P/mirror/etc/zz-test-only.conf
$ echo 'profile = "mirrortest"' > ~/machine/root.item8/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.item8 genarch-settings apply | tail -1
SUMMARY planned=57 applied=57 failed=0 exit=0
$ ls -l ~/machine/root.item8/etc/zz-test-only.conf
-rw-r--r-- 1 arch arch 4 Nov 14 15:30 /home/arch/machine/root.item8/etc/zz-test-only.conf
$ grep -o 'zz-test-only[^"]*' ~/machine/root.item8/var/lib/genarch-settings/manifest.toml
zz-test-only.conf
$ rm ~/machine/root.item8/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.item8 genarch-settings apply
APPLY2=0
PROFILE none (base intent only)
PLAN 0 change(s)
$ ls -l ~/machine/root.item8/etc/zz-test-only.conf
-rw-r--r-- 1 arch arch 4 Nov 14 15:30 /home/arch/machine/root.item8/etc/zz-test-only.conf
$ rm -rf ~/machine/root.item8/etc/genarch-settings/profiles/mirrortest
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.item8 genarch-settings apply
APPLY3=0
BACKUPS 1
  /etc/zz-test-only.conf.bak-20261114
PRUNED /etc/zz-test-only.conf
```
