#!/bin/bash
# Adversarial battery: junk, empty, half-written, wrong-identity, interrupted, removal.
set +e
R=~/machine/root

echo "== A1 empty base intent =="
rm -rf ~/x1; mkdir -p ~/x1/usr/lib/genarch-settings
GENARCH_SETTINGS_ROOT=$HOME/x1 genarch-settings report 2>&1 | tail -2; echo "EXIT=${PIPESTATUS[0]}"

echo "== A2 half-written sysctl (unquoted dotted key) =="
rm -rf ~/x2; cp -a $R ~/x2
printf 'kernel.printk = "3 3 3 3"\n' > ~/x2/etc/genarch-settings/profiles/vm-generic/sysctl.toml
GENARCH_SETTINGS_ROOT=$HOME/x2 genarch-settings report 2>&1 | tail -2; echo "EXIT=${PIPESTATUS[0]}"

echo "== A3 unknown key in a layer intent.toml =="
rm -rf ~/x3; cp -a $R ~/x3
printf 'schema = 3\nnonsense = 1\n' > ~/x3/etc/genarch-settings/intent.toml
GENARCH_SETTINGS_ROOT=$HOME/x3 genarch-settings report 2>&1 | tail -2; echo "EXIT=${PIPESTATUS[0]}"

echo "== A4 selected profile does not exist =="
rm -rf ~/x4; cp -a $R ~/x4
printf 'profile = "nope"\n' > ~/x4/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=$HOME/x4 genarch-settings report 2>&1 | tail -2; echo "EXIT=${PIPESTATUS[0]}"

echo "== A5 unprivileged apply on the live root =="
genarch-settings apply 2>&1 | tail -2; echo "EXIT=${PIPESTATUS[0]}"

echo "== A6 SIGKILL mid-apply =="
rm -rf ~/x6; cp -a $R ~/x6; mkdir -p ~/x6/usr/local/lib
GENARCH_SETTINGS_ROOT=$HOME/x6 timeout -s KILL 0.10 genarch-settings apply >/dev/null 2>&1; echo "KILLED_EXIT=$?"
echo "files=$(find ~/x6/etc ~/x6/usr/local ~/x6/usr/lib/systemd -type f 2>/dev/null | wc -l) manifest=$(test -f ~/x6/var/lib/genarch-settings/manifest.toml && echo yes || echo no)"
GENARCH_SETTINGS_ROOT=$HOME/x6 genarch-settings report 2>/dev/null | awk '$1=="DRIFT"{print $2}' | tr '\n' ' '; echo
GENARCH_SETTINGS_ROOT=$HOME/x6 genarch-settings apply >/tmp/r6 2>&1; echo "REAPPLY_EXIT=$? $(grep -o 'SUMMARY.*' /tmp/r6)"
echo "manifest_after=$(test -f ~/x6/var/lib/genarch-settings/manifest.toml && echo yes || echo no)"

echo "== A7 install -> remove -> reinstall =="
sudo pacman -R --noconfirm genarch-settings 2>&1 | grep -E "genarch-settings:|removed|error" | head -4
echo "unit_left=$(test -f /usr/lib/systemd/system/genarch-settings.service && echo yes || echo no) doc_left=$(test -f /usr/share/doc/genarch-settings/README.md && echo yes || echo no)"
sudo pacman -U --noconfirm ~/payload/genarch-settings-*.pkg.tar.zst >/tmp/re 2>&1; echo "REINSTALL_EXIT=$?"
grep -E "genarch-settings:|SUMMARY" /tmp/re | head -3

echo "== A8 stale pid text, no lock held =="
rm -rf ~/x8; cp -a $R ~/x8; rm -f ~/x8/usr/lib/genarch-settings/managed/ppd-conflict.toml
mkdir -p ~/x8/var/lib/genarch-settings
printf 'pid=999999 via=genarch-settings\n' > ~/x8/var/lib/genarch-settings/lock
GENARCH_SETTINGS_ROOT=$HOME/x8 genarch-settings apply >/tmp/r8 2>&1; echo "EXIT=$? $(grep -o 'SUMMARY.*' /tmp/r8)"

echo "== A9 renamed binary =="
cp /usr/bin/genarch-settings /tmp/gs-renamed; /tmp/gs-renamed report >/dev/null 2>&1; echo "REPORT_EXIT=$?"
/tmp/gs-renamed bogus 2>&1 | head -2; rm -f /tmp/gs-renamed
