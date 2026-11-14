# round 2 — retest of the eight round-1 items

Box: QEMU VM, Arch Linux 7.2.11-arch1-1, `genarch-settings 0.4.0-1` fresh from `~/payload`,
`ryzenadj 0.19.0-1`. Rehearsal tree `~/machine/root` (rebuilt with `~/machine.sh ~/machine`
between tests). Nothing under the real `/etc` was written by hand.

---

## 1. pacman -U exits 0 while the engine applies nothing and no unit is installed

**Verdict: still**

Reproduction:

```
cd ~/payload && sudo pacman -U --noconfirm genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
```

Output (verbatim, package lines only):

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
:: Running post-transaction hooks...
(1/1) Arming ConditionNeedsUpdate...
PACMAN EXIT:0
```

```
$ find /etc/systemd/system /usr/lib/systemd/system -maxdepth 1 -name "genarch*" | wc -l
0
```

The outcome is the item's claim unchanged: pacman exits 0, nothing is applied, no unit exists.
What did change is that the package now *says* so (the three `genarch-settings:` lines are new to
me, and `Failed to enable unit` is printed instead of being hidden). The transaction still ends
success — a script that checks `pacman -U`'s status cannot tell this host from a converged one.

---

## 2. apply refuses every change when one managed target is missing

**Verdict: still**

Reproduction:

```
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
rm $HOME/machine/root/usr/lib/systemd/system/power-profiles-daemon.service
genarch-settings apply
```

Output:

```
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT:3
```

One absent foreign file still costs all 55 other changes; the staging root stays as it was. The
README now calls this deliberate ("the whole apply is refused with 'nothing was written' and exit
3. The drift to fix is the selection"), so I can predict it — but the workflow is still blocked:
on this box the file it wants is `asusd.ron`'s sibling, and no command removes a record from the
plan without editing the intent.

Two details while retesting:

- The refusal prints **no plan**, while the README says "The plan is still computed and printed".
  On the refusal path the only output is the error block above.
- `apply --dry-run` with the same tree does not behave as the README says a preview behaves; see
  `issues/01-apply-dry-run-exits-3.md`.

---

## 3. the documentation the product points at is not on the machine

**Verdict: different**

The documents now ship. `pacman -Ql` and the directory:

```
$ ls /usr/share/doc/genarch-settings/
AGENTS.md  CONTEXT.md  CONTRIBUTING.md  DESIGN.md  LICENSE  PROFILES.md  README.md  SCOPE.md  install.sh
```

What is still broken:

```
$ pacman -Ql genarch-settings | grep -ci cutover
0
$ grep -n "CUTOVER" /usr/share/doc/genarch-settings/README.md
143:- `docs/CUTOVER.md` — the runbook that moved this host off the shell engine, including rollback
$ ls /usr/share/doc/genarch-settings/docs
ls: cannot access '/usr/share/doc/genarch-settings/docs': No such file or directory
```

The one document that is missing is the one this persona needs most — the runbook with the
rollback procedure — and the README still lists it under "Documentation" as if it were installed.
The remaining `docs/` references resolve to the flat files, and the README's last paragraph says
so ("where `docs/PROFILES.md` becomes `PROFILES.md`"), so those are merely untidy. I checked the
anchors it quotes: "When a machine cannot be covered" exists in `PROFILES.md` (line 91).

---

## 4. a failed write makes apply exit 3 with no plan, no summary and no account of what was applied

**Verdict: still**

Reproduction:

```
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
chmod 0444 $HOME/machine/root/proc/sys/vm/swappiness
genarch-settings apply
```

Output, in full:

```
SUMMARY applied=0 failed=1 exit=3
genarch-settings: write /home/arch/machine/root/proc/sys/vm/swappiness: Permission denied (os error 13)
EXIT:3
```

That is the whole output. No plan, no list of completed steps, and `applied=0` is wrong — by the
time the sysctl writer runs, the 40 mirror files and both managed records have already been
written:

```
$ ls $HOME/machine/root/etc/udev/rules.d/
61-eco-ioschedulers.rules
70-eco-audio-pm.rules
81-wifi-powersave.rules
82-genarch-settings-ceil.rules
$ ls $HOME/machine/root/etc/systemd/*.bak* $HOME/machine/root/usr/lib/systemd/system/*.bak*
/home/arch/machine/root/etc/systemd/journald.conf.bak-20261114
/home/arch/machine/root/usr/lib/systemd/system/power-profiles-daemon.service.bak-20261114
```

So the summary says zero applied while the host is half converged; the operator gets no account of
which half. (For completeness: a second `apply` after `chmod 0644` completes, `exit 0`, and the
report then shows only CEIL and TIMER. Recovery works; only the reporting of the failure does not.)

---

## 5. harvest prints nothing for a drifted sysctl

**Verdict: gone**

Reproduction:

```
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
echo 60 > $HOME/machine/root/proc/sys/vm/swappiness
genarch-settings harvest
```

Output:

```
--- sysctl vm.swappiness (live 60)
- 150
+ 60
--- ceil policy0 (live 4000000)
- 3000000
+ 4000000
--- ceil policy1 (live 4000000)
- 3000000
+ 4000000
EXIT:0
```

It names the key, the live value, and the diff. (On a host with nothing to propose, `harvest`
prints nothing at all and exits 0 — silent success, but that matches the "writes nothing"
description.)

---

## 6. harvest managed-record diff shows live to intent, opposite of its mirror output

**Verdict: gone**

Reproduction:

```
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
sed -i 's/^SystemMaxUse=256M/SystemMaxUse=64M/' $HOME/machine/root/etc/systemd/journald.conf
genarch-settings harvest
```

Output:

```
--- managed journald in /etc/systemd/journald.conf
- SystemMaxUse=256M
+ SystemMaxUse=64M
```

`-` is the declared value and `+` the live one — the same direction as the sysctl and ceil blocks
above. One observation while comparing: the mirror block prints the whole live file as `+` lines
with no `-` lines (`--- intent: /etc/security/limits.d/20-audio.conf (live differs)` followed by
every line of the file, including a one-line local edit). It is the right direction, but not a
diff I can read at a glance.

---

## 7. uninstall leaves everything the engine wrote

**Verdict: different**

Reproduction:

```
sudo pacman -R --noconfirm genarch-settings
```

Output:

```
removing genarch-settings...
genarch-settings: deployed files listed in /var/lib/genarch-settings/manifest.toml were left in place
genarch-settings: pre-images (*.bak-YYYYMMDD) sit beside every file the engine replaced
(1/1) Arming ConditionNeedsUpdate...
EXIT:0
```

What is different: the package now tells you at removal time what it is leaving, and the README's
**Removal** section is a real procedure ("Read the manifest before removing; if the removal is
permanent, delete those paths yourself. A reinstall adopts the same manifest."). What is not
different: there is still no command that removes what the engine wrote, and a user with a
fuller machine than this one deletes dozens of paths by hand.

I could not observe leftovers themselves on this box: `apply` never succeeds here, so
`/var/lib/genarch-settings/manifest.toml` never exists —

```
$ ls -la /var/lib/genarch-settings
ls: cannot access '/var/lib/genarch-settings': No such file or directory
```

— and the uninstall removed everything it had installed (`/etc/genarch-settings`,
`/usr/lib/genarch-settings`, `/usr/bin/genarch-settings` all gone, `systemctl list-unit-files |
grep genarch` empty).

---

## 8. GENARCH_SETTINGS_ROOT does not isolate external commands

**Verdict: gone**

Reproduction (a shim for every external tool the engine could call, ahead on `PATH`):

```
mkdir -p ~/machine/shim
for t in ryzenadj systemctl udevadm modprobe iw sysctl; do
  printf '#!/bin/sh\necho "CALLED: %s $*" >> $HOME/machine/shim.log\nexit 0\n' $t > ~/machine/shim/$t
  chmod +x ~/machine/shim/$t
done
rm -f ~/machine/shim.log
PATH=$HOME/machine/shim:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply
PATH=$HOME/machine/shim:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot
PATH=$HOME/machine/shim:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge resume
PATH=$HOME/machine/shim:$PATH GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge ceil
cat ~/machine/shim.log
```

Output of the passes:

```
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (-1): rehearsal root: not executing ryzenadj
genarch-settings: ryzenadj failed: ryzenadj --power-saving (-1): rehearsal root: not executing ryzenadj
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
genarch-settings: smu: all 2 ryzenadj call(s) failed — no SMU setting was applied
EXIT:3
```

and the shim log:

```
cat: /home/arch/machine/shim.log: No such file or directory
```

No external command ran: `apply` and `converge ceil` exit 0 silently, `boot`/`resume` refuse the
SMU step by name instead of calling it. The README states exactly this ("a rehearsal never writes
this machine's SMU and never reloads its systemd or udev"). The exit 3 of a rehearsal SMU pass is
consistent with "nothing was applied", not a regression.
