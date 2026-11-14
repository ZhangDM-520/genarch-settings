#!/bin/bash
# Fixed check battery for the differential round: one line per check, same text on both builds.
set +e
say() { printf 'CHECK %s: %s\n' "$1" "$2"; }

say docs "$(ls /usr/share/doc/genarch-settings 2>/dev/null | tr '\n' ',' )"
genarch-settings report >/dev/null 2>&1; say report-exit "$?"
genarch-settings apply --dry-run >/tmp/dr 2>&1
say dryrun-exit "$? plan=$(grep -c '^PLAN' /tmp/dr) anomaly=$(grep -c ANOMALY /tmp/dr)"
bash -c 'genarch-settings report 2>/dev/null | true; echo ${PIPESTATUS[0]}' >/tmp/sp
say sigpipe "$(cat /tmp/sp)"
say report-last-byte "$(genarch-settings report 2>/dev/null | tail -c 1 | od -An -c | tr -d ' ')"

rm -rf ~/d; cp -a ~/machine/root ~/d
GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings apply >/tmp/a 2>&1
say rehearsal-apply "$? $(grep -o 'SUMMARY.*' /tmp/a)"
say rehearsal-replan "$(GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings apply 2>&1 | grep -c '^PLAN 0')"
say rehearsal-timer "$(GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings report 2>/dev/null | awk '$2=="TIMER"{print $1}')"

printf '150\n' > ~/d/proc/sys/vm/swappiness
say harvest-sysctl "$(GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings harvest 2>&1 | grep -c 'sysctl vm.swappiness')"
GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings converge ceil >/tmp/c 2>&1
say ceil "$? max=$(cat ~/d/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq 2>/dev/null)"
GENARCH_SETTINGS_ROOT=$HOME/d genarch-settings converge boot >/tmp/b 2>&1
say boot-smu-fail "$?"

rm -rf ~/p; cp -a ~/machine/root ~/p; mkdir -p ~/p/etc/tlp.d; chmod a-w ~/p/etc/tlp.d
GENARCH_SETTINGS_ROOT=$HOME/p genarch-settings apply >/tmp/p 2>&1
say partial "$? $(grep -o 'SUMMARY.*' /tmp/p)"

rm -rf ~/l; cp -a ~/machine/root ~/l; rm -f ~/l/usr/lib/genarch-settings/managed/ppd-conflict.toml
mkdir -p ~/l/var/lib/genarch-settings
( flock -x ~/l/var/lib/genarch-settings/lock -c 'sleep 4' & )
sleep 1
say lock "$(GENARCH_SETTINGS_ROOT=$HOME/l genarch-settings apply 2>&1 | grep -o 'apply lock held by.*' | head -1)"
