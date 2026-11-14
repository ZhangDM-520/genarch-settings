# The report shows the winning value but never the layer it came from or the profile's documented/measured provenance

- **severity:** medium
- **category:** enhancement
- **environment:** Arch Linux guest (QEMU, 2 vCPU), `genarch-settings 0.4.0-1`; copy of the staging tree (`GENARCH_SETTINGS_ROOT=~/ev7`). Base `vm.swappiness=150`, profile layer `120`, operator layer `60`.

## Intent

Answer an auditor's question from the engine's own output: "which layer supplied this value, and
is the selected profile documented or measured?" — the brief's test that precedence is obvious
from the output, not from reading the source.

## Steps

```sh
cp -a ~/machine/root ~/ev7
O=~/ev7/etc/genarch-settings
P=$O/profiles/qemu-i440fx
printf '[values]\n"vm.swappiness" = "120"\n' > $P/sysctl.toml
printf '[values]\n"vm.swappiness" = "60"\n'  > $O/sysctl.toml
printf '150' > ~/ev7/proc/sys/vm/swappiness
GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report --json
GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report --help
rm $O/sysctl.toml; GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
rm $P/sysctl.toml; GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
```

The profile also carries `status = "documented"` and a `source` (the JSON probe below).

## Expected

`CONTEXT.md`: "Precedence is base → profile → operator; the operator layer always wins."

`CONTEXT.md`: "A profile also states how it came to be: **documented** (every value published and
cited) or **measured** (observed on the machine). The distinction is load-bearing, not cosmetic."

`DESIGN.md`: "the JSON document (`schema: 2`, with a top-level `profile` object) is the machine seam
that a documentation gate reads instead of parsing prose."

An operator table or JSON that an auditor can use should name the layer that supplied each
effective value, and include the profile provenance.

## Actual

The report prints only the effective value (`want 60`, then `want 120`, then the OK row), with no
layer. `report --help` offers only `--json`. The JSON `profile` object carries `name`, `matched`
and `requires`, and nothing about `status`/`source`; the SYNC check carries `detail`/`doc`/`reach`,
no layer. There is no verb or flag that reports where a merged value came from. I had to remove
the layers one at a time to establish which one won.

## Evidence

```
COMMAND: GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
DRIFT SYNC vm.swappiness=150 want 60 findings#writeback

COMMAND: GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report --json | python -c "import json,sys;d=json.load(sys.stdin);print(json.dumps(d[\"profile\"],indent=1));print([c for c in d[\"checks\"] if c[\"id\"]==\"SYNC\"])"
{
 "name": "qemu-i440fx",
 "matched": true,
 "requires": {
  "product": "Standard PC (i440FX + PIIX, 1996)",
  "sys_vendor": "QEMU"
 }
}
[{'id': 'SYNC', 'class': 'DRIFT', 'detail': 'vm.swappiness=150 want 60', 'doc': 'findings#writeback', 'reach': 'full'}]

COMMAND: GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report --help
Judge the live system against intent. Read-only; runs unprivileged

Usage: genarch-settings report [OPTIONS]

Options:
      --json  Emit the stable machine-readable schema instead of the operator table
  -h, --help  Print help

COMMAND: rm ~/ev7/etc/genarch-settings/sysctl.toml
COMMAND: GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
DRIFT SYNC vm.swappiness=150 want 120 findings#writeback

COMMAND: rm ~/ev7/etc/genarch-settings/profiles/qemu-i440fx/sysctl.toml
COMMAND: GENARCH_SETTINGS_ROOT=~/ev7 genarch-settings report | grep SYNC
OK SYNC 16 runtime sysctl value(s) match intent findings#writeback
```

The profile's provenance (status/source) that the JSON omits:

```
COMMAND: cat ~/ev7/etc/genarch-settings/profiles/qemu-i440fx/intent.toml
status = "documented"
source = ["https://example.invalid/staging-box"]
[requires]
product = "Standard PC (i440FX + PIIX, 1996)"
sys_vendor = "QEMU"
```

## What could not be determined

- Whether the JSON schema documents any provenance field beyond `schema`, `profile`, `checks`,
  `summary`; `DESIGN.md` only says "top-level `profile` object".
- Whether a diagnostic flag exists outside the CLI (no source on the machine; `--help` shows
  only `--json`).
