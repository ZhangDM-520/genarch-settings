# A selected profile that has no `[requires]` is reported OK and its machine facts are applied to this box

- **severity:** high
- **category:** bug
- **environment:** Arch Linux guest (QEMU, 2 vCPU, kernel 7.2.11-arch1-1), `genarch-settings 0.4.0-1`. `GENARCH_SETTINGS_ROOT` points at a copy of the staging tree; the two cpufreq policies admit 3.5 GHz (`cpuinfo_max_freq`).

## Intent

Write the profile for the staging machine, select it, and have the engine either match the box or
refuse the profile. The profile below has a ceiling (a machine fact) but no machine requirement —
the shape a first-time porter produces when the `[requires]` block is omitted or left empty.

## Steps

```sh
cp -a ~/machine/root ~/ev1
mkdir -p ~/ev1/etc/genarch-settings/profiles/forgot-requires
printf '[[class]]\nname = "forgot"\nkhz = 3000000\n' \
  > ~/ev1/etc/genarch-settings/profiles/forgot-requires/ceil.toml
printf 'profile = "forgot-requires"\n' > ~/ev1/etc/genarch-settings/profile
GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings report | head -1
GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings converge ceil
cat ~/ev1/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings report | grep -E "PROFILE|CEIL"
ls ~/ev1/etc/genarch-settings/profiles/forgot-requires/
```

## Expected

`DESIGN.md`: "A profile declares `[requires]` (DMI product/vendor); a mismatch is `PROFILE`
drift (exit 1), not a refusal to load."

`install.sh` usage: "Profiles live in <intent-dir>/profiles/NAME and declare the machine they are
for ([requires])."

`AGENTS.md`: "A profile's `[requires]` mismatch is **PROFILE drift (exit 1)**, not a load error."

A profile that carries machine facts must therefore be tied to a machine: either refuse it at
load or report PROFILE drift. Reporting a green PROFILE row is neither.

## Actual

The profile has only `ceil.toml` (no `intent.toml`, no `[requires]`). The report says

`OK PROFILE profile 'forgot-requires' declares no machine requirement`

and `converge ceil` applies the ceiling declared by that unconstrained profile to this box
(3.5 GHz → 3.0 GHz on both policies). `apply` is happy (exit 0). The same result follows from an
*empty* profile directory (`OK PROFILE profile 'empty-one' declares no machine requirement`), which
I also reproduced while probing the layers.

The detail text does say "declares no machine requirement", so it is not silent; but the class is
`OK`, `SUMMARY` counts no drift for it, and the machine facts are applied. A porter who forgets
the `[requires]` block — or names an empty directory, e.g. by typo — gets a green profile row on
every machine.

## Evidence

```
COMMAND: GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings report | head -1
OK PROFILE profile 'forgot-requires' declares no machine requirement findings#intent

COMMAND: GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings converge ceil
genarch-settings: CEIL DRIFT policy0 3500000 -> 3000000
genarch-settings: CEIL DRIFT policy1 3500000 -> 3000000
exit=0

COMMAND: cat ~/ev1/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
3000000

COMMAND: GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings report | grep -E "PROFILE|CEIL"
OK PROFILE profile 'forgot-requires' declares no machine requirement findings#intent
OK CEIL all 2 policies capped forgot 3000000 findings#ceil

COMMAND: ls ~/ev1/etc/genarch-settings/profiles/forgot-requires/
ceil.toml
```

Empty directory variant (same copy, an empty `profiles/empty-one/`):

```
COMMAND: mkdir -p ~/ev1/etc/genarch-settings/profiles/empty-one
COMMAND: printf 'profile = "empty-one"\n' > ~/ev1/etc/genarch-settings/profile
COMMAND: GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings report | head -1
OK PROFILE profile 'empty-one' declares no machine requirement findings#intent
COMMAND: GENARCH_SETTINGS_ROOT=~/ev1 genarch-settings apply | tail -2
APPLY-SCOPE profile=empty-one full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER
SUMMARY planned=0 applied=0 failed=0 exit=0
```

## What could not be determined

- Whether an unconstrained profile was intended as a supported "generic" profile. The shipped
  docs describe a profile as the machine layer and say it declares `[requires]`; no shipped file
  describes a profile that deliberately matches every machine.
- Whether the shipped-profile test (`cargo test --test profiles`) would reject such a directory;
  that test is a source-tree gate, not part of an installed host, so an operator-authored profile
  never meets it.
