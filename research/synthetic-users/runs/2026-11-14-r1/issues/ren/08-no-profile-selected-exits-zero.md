# A host with no profile selected is reported `exit 0` once the other rows are green

- **severity:** low
- **category:** enhancement
- **environment:** Arch Linux guest (QEMU, 2 vCPU), `genarch-settings 0.4.0-1`; converged copy of the staging tree with the selection removed (`GENARCH_SETTINGS_ROOT=~/ev8`). The TIMER row was made green with a stub `systemctl` on `PATH` that answers `is-enabled` with `enabled`; a rehearsal root cannot enable units in the live systemd, and a host installed by `install.sh` has them enabled.

## Intent

Ask the engine whether this host is actually declared: a fleet/audit check that runs
`genarch-settings report` and branches on its exit code.

## Steps

```sh
cp -a ~/machine/root ~/ev8
rm -f ~/ev8/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings apply        # base only, exit 0
mkdir -p ~/fakebin
printf '#!/bin/sh\necho enabled\n' > ~/fakebin/systemctl; chmod 755 ~/fakebin/systemctl
PATH=~/fakebin:$PATH GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings report; echo "exit=$?"
PATH=~/fakebin:$PATH GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings report --json \
  | python -c 'import json,sys; d=json.load(sys.stdin); print(d["profile"], d["summary"])'
```

## Expected

`CONTEXT.md`: "`SKIPPED` means unverified, not fine. It counts as green only because the
alternative is training an operator to ignore the exit code."

`install.sh` (header comment): "a fresh host runs the base intent and reports PROFILE SKIPPED until an
operator writes that line."

A host with no profile has not been ported; the engine should make that distinguishable by exit
status (or by a summary field a script can read), not only by the word SKIPPED in one of eleven
rows.

## Actual

A converged base with no profile selected reports `SUMMARY … drift=0 skipped=6 exit=0`; the only
signal that this host is not declared is the word SKIPPED in the PROFILE row (and `skipped=6`).
`apply` on the same tree exits 0. The JSON says `profile: null`, with the same `exit: 0` in
`summary`. A script that branches on the exit code sees a clean host, which is the failure mode
the brief calls out ("reports 'no profile' as success").

The behaviour is consistent with the design note quoted above, so this is filed as an enhancement,
not a contradiction: the workflow it blocks is a fleet audit that must not parse verdict text.

## Evidence

```
COMMAND: PATH=~/fakebin:$PATH GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings report; echo "exit=$?"
SKIPPED PROFILE no profile selected (base intent only) findings#intent
OK MIRROR 39 mirror target(s) byte-equal findings#intent
OK MANAGED 2 record(s) hold their line findings#managed
OK SYNC 16 runtime sysctl value(s) match intent findings#writeback
SKIPPED CEIL no ceiling classes declared (ceil.toml) findings#ceil
SKIPPED CO no curve-optimizer offset declared (the silicon is the profile's to describe) findings#co
SKIPPED SMU no SMU envelope declared (the silicon is the profile's to describe) findings#ppt
SKIPPED POWER no --power-saving declaration (the silicon is the profile's to describe) findings#eco
SKIPPED WIFI profile declares no wifi_interface findings#wifi
OK TIMER ["genarch-settings.service", "genarch-settings-ceil.timer"] enabled findings#timer
OK PRUNE 41 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=5 accepted=0 warn=0 drift=0 skipped=6 exit=0
exit=0

COMMAND: PATH=~/fakebin:$PATH GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings report --json | python -c "import json,sys; d=json.load(sys.stdin); print(d['profile'], d['summary'])"
None {'ok': 5, 'accepted': 0, 'warn': 0, 'drift': 0, 'skipped': 6, 'exit': 0}

COMMAND: GENARCH_SETTINGS_ROOT=~/ev8 genarch-settings apply > /dev/null 2>&1; echo "apply (no profile) exit=$?"
apply (no profile) exit=0
```

## What could not be determined

- Whether a non-zero exit for an unselected profile was deliberately rejected (the docs say
  SKIPPED counts as green to protect the exit code's usefulness; that is an argument, not a
  statement that an unported host is clean).
- Whether a real installed host's TIMER row is green (the package's units are enabled by
  `install.sh`; I could not enable them on the live guest without writing the real `/etc`).
