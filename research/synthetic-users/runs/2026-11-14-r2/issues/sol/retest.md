# Round 2 retest

Environment: Arch Linux, 2 vCPU QEMU (`Standard PC (i440FX + PIIX, 1996)`, `QEMU`),
genarch-settings 0.4.0-1 freshly installed from `~/payload`, ryzenadj 0.19.0-1.
Writes were made in copies of `~/machine/root` (`~/root.pristine` is the untouched copy);
`~/t3`, `~/t4`, `~/h`, `~/k2`, `~/k3`, `~/c` are the scratch trees named below.

---

## 1. pacman -U reports success while post_install apply deployed nothing — **different**

Reproduction:

```sh
sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst; echo "EXIT=$?"
```

Verbatim output (tail):

```
reinstalling genarch-settings...
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
PACMAN_EXIT=0
```

Changed, but not fixed: pacman still exits 0 and leaves a host with no engine-deployed unit,
but the install now prints the refusal verbatim and tells the operator to run `report`.
`/var/lib/genarch-settings/manifest.toml` is absent afterwards, so nothing deployed is
owned. A provisioning script that chains `pacman -U … && systemctl start genarch-settings.service`
still fails at the second command for a reason unrelated to the engine's diagnosis.
The shipped `.INSTALL` now states the install is deliberately non-fatal, so the exit code is
documented behaviour rather than an accident.

---

## 2. the shipped profile selection is applied on a machine whose DMI does not match — **still**

Reproduction:

```sh
cat /etc/genarch-settings/profile
genarch-settings report | head -1
```

Verbatim output:

```
profile = "zenbook-s16"
DRIFT PROFILE profile 'zenbook-s16' is not for this machine: product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA; sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC. findings#intent
```

The package still installs a selection for another machine's DMI, and every fresh install on a
non-Zenbook host starts in drift. The README now says so ("The package ships the `zenbook-s16`
profile **and** its selection"), and `install.sh` selects none, so this is expected-of-the-
package rather than unexpected — but the outcome is the same bad one: nothing warns at install
time except the refused apply whose reason is a different missing file (item 3).

---

## 3. one absent managed target refuses the entire plan — **still**

Reproduction (profile selection for a machine without the ASUS tooling):

```sh
rm -rf ~/t3; cp -a ~/root.pristine ~/t3
printf 'profile = "zenbook-s16"\n' > ~/t3/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=$HOME/t3 genarch-settings apply; echo "exit=$?"
ls ~/t3/var/lib/genarch-settings
```

Verbatim output (tail):

```
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
nothing was written
exit=3
ls: cannot access '/home/arch/t3/var/lib/genarch-settings': No such file or directory
```

Five optional `asusd.ron` anchors withhold the other ~50 changes — 41 mirrors, the sysctl
projection, two managed records. The README now documents this outcome explicitly, so it is
not a contradiction of the material; it is still the wrong shape for a converge tool on a host
that never had asusd: it converges nothing and leaves no ownership record, and the operator
must hand-edit the intent layer to get a single systemd drop-in deployed. `report` shows the
same refusal as DRIFT, so the host cannot even be brought to base intent with `apply`.

---

## 4. the manifest claims ownership of mirrors a fresh install never wrote — **gone**

Reproduction:

```sh
sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
ls /var/lib/genarch-settings
# and, for a successful apply, in a rehearsal copy:
rm -rf ~/t4; cp -a ~/root.pristine ~/t4
GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings apply
python3 - <<'EOF'
import tomllib, os
m = tomllib.load(open(os.path.expanduser("~/t4/var/lib/genarch-settings/manifest.toml"), "rb"))
print("owned", len(m["owned"]), "missing", [p for p in m["owned"] if not os.path.exists(os.path.expanduser("~/t4") + p)])
print("touched", m.get("touched"))
EOF
```

Verbatim output:

```
$ ls -la /var/lib/genarch-settings
-rw-r--r-- 1 root root  30 Nov 14 15:30 lock
$ cat /var/lib/genarch-settings/manifest.toml
cat: /var/lib/genarch-settings/manifest.toml: No such file or directory
$ ...python...
owned 40 missing []
touched ['/etc/systemd/journald.conf', '/usr/lib/systemd/system/power-profiles-daemon.service']
```

No manifest is written when the apply is refused (the directory holds only the lock), and a
successful apply's manifest names
exactly the files that exist, plus the two patched foreign files in `touched`. No phantom
ownership.

---

## 5. a partial or killed apply leaves deployed files outside the manifest and a stray temp — **still**

Reproduction:

```sh
rm -rf ~/k3; cp -a ~/root.pristine ~/k3
timeout -s KILL 0.12 env GENARCH_SETTINGS_ROOT=$HOME/k3 genarch-settings apply
find ~/k3/etc ~/k3/usr/local ~/k3/usr/lib/systemd/system-sleep -type f | wc -l
ls ~/k3/var/lib/genarch-settings
GENARCH_SETTINGS_ROOT=$HOME/k3 genarch-settings report | grep -E "^(DRIFT MIRROR|OK PRUNE)"
```

Verbatim output:

```
Killed
pristine=6 killed=40 manifest=lock
$ ls ~/k3/var/lib/genarch-settings
lock
$ GENARCH_SETTINGS_ROOT=$HOME/k3 genarch-settings report
DRIFT MIRROR 39 target(s) differ: /etc/NetworkManager/conf.d/99-wifi-powersave.conf ...
OK PRUNE 0 owned path(s) in the manifest, none to remove findings#intent
```

34 written files, no `manifest.toml`; `report` says the engine owns nothing even while it
reports that one mirror already matches. The stray temp is also still there after a successful
recovery apply:

```
$ find ~/k -name ".*.tmp"
./etc/systemd/journald.conf.d/.99-eco-size.conf.4544.tmp
$ GENARCH_SETTINGS_ROOT=$HOME/k genarch-settings apply
SUMMARY planned=49 applied=49 failed=0 exit=0
$ find ~/k -name ".*.tmp"
./etc/systemd/journald.conf.d/.99-eco-size.conf.4544.tmp
```

One improvement since round 1: a later successful apply rewrites the manifest covering the
files the killed run left behind, so `PRUNE 40 owned path(s)` is reachable without manual
cleanup. The killed state itself is still unowned and the temp is never removed.

---

## 6. harvest prints nothing for sysctl drift — **gone**

Reproduction:

```sh
GENARCH_SETTINGS_ROOT=$HOME/k2 genarch-settings harvest; echo "rc=$?"
```

Verbatim output (tail):

```
--- sysctl vm.vfs_cache_pressure (live 100)
- 50
+ 100
--- ceil policy0 (live 4000000)
- 3000000
+ 4000000
--- ceil policy1 (live 4000000)
- 3000000
+ 4000000
harvest rc=0
```

Sysctl drift is now printed, along with mirror and ceiling drift.

---

## 7. converge boot exits 0 when every ryzenadj write failed — **gone**

Reproduction:

```sh
sudo ryzenadj --set-coall=0x00000000; echo "direct rc=$?"
sudo genarch-settings converge boot; echo "rc=$?"
```

Verbatim output (tail):

```
Error: Unable to get SMU object (no AMD SMU on this machine)
set rc=1
...
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFB (1): Error: Unable to get SMU object (no AMD SMU on this machine)
...
genarch-settings: smu: 12 of 12 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: all 12 ryzenadj call(s) failed — no SMU setting was applied
rc=3
```

A pass where every SMU write failed now exits 3. The hardware absence (no AMD SMU) is a
limitation of this box, not of the package.

---

## 8. a second apply on the same day takes no pre-image — **gone**

Reproduction:

```sh
# ~/t4 was already applied once; it created /etc/tlp.d/01-eco.conf (no pre-image, new file)
cd ~/t4
printf "# operator edit\n" >> etc/tlp.d/01-eco.conf
GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings apply
ls -l etc/tlp.d/
printf "# second operator edit\n" >> etc/tlp.d/01-eco.conf
md5sum etc/tlp.d/01-eco.conf.bak-20261114
GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings apply
md5sum etc/tlp.d/01-eco.conf.bak-20261114
```

Verbatim output:

```
$ GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings apply
PLAN 1 change(s)
  OWNED /etc/tlp.d/01-eco.conf
BACKUPS 1
  /etc/tlp.d/01-eco.conf.bak-20261114
$ tail -2 etc/tlp.d/01-eco.conf.bak-20261114
MAX_LOST_WORK_SECS_ON_BAT=0
# operator edit
$ md5sum etc/tlp.d/01-eco.conf.bak-20261114
8e450732adb93ef20f75007ceab4ab23  etc/tlp.d/01-eco.conf.bak-20261114
$ GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings apply
BACKUPS 1
  /etc/tlp.d/01-eco.conf.bak-20261114
$ md5sum etc/tlp.d/01-eco.conf.bak-20261114
8e450732adb93ef20f75007ceab4ab23  etc/tlp.d/01-eco.conf.bak-20261114
```

The replacement takes a dated pre-image, and a later same-day replacement neither loses nor
overwrites the first one. The bytes replaced by the *second* replacement are not kept, which is
consistent with the pre-image being the original restoration point, not a history.
Mode is preserved on deployment: `755` for `system-sleep/genarch-settings` and `audio-pm-ctl`,
`644` for `/etc/tlp.d/01-eco.conf`.
