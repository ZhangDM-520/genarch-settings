# Profiles

A **profile** is the machine layer: a directory under `intent/profiles/<slug>/` that carries
everything true of one laptop and nothing true of another. `/etc/genarch-settings/profile` selects it
(`profile = "<slug>"`), and `/sys/class/dmi/id` decides whether it is the right machine
(`[requires]`, reported as the `PROFILE` row).

This page is the contract for writing one. It exists because the alternative — a profile that
inherits this project's own laptop — is exactly what made 0.1.x non-portable.

## Naming

`<vendor>-<model>-<generation-or-sku>`, lowercase, hyphenated: `framework-13-amd-7040`,
`thinkpad-z13-gen2`, `zephyrus-g14-2024`, `elitebook-845-g11`, `zenbook-s16`.

The directory name **is** the profile name, and it is what the operator writes in
`/etc/genarch-settings/profile`. Rename the directory and you have renamed the profile.

## Provenance: the rule that decides what may be in a profile

Every value falls into one of three classes. The class decides whether it may ship.

| Class | What it is | Examples | In a **documented** profile? |
|---|---|---|---|
| **D** documented | A publication states it | DMI strings, CPU model, core count, boost clock, platform TDP envelope, vendor tooling | yes, with a citation |
| **C** community | Someone measured or reported it | a safe sustained PPT, a wake-device list, a suspend quirk | yes, marked, never promoted to D |
| **L** live-only | Only obtainable on the machine in hand | `/etc/fstab` UUIDs, `/home/<user>`, `/dev/nvme0n1p3`, curve offsets from a silicon sweep | **never** |

A profile declares how it was made:

```toml
# intent/profiles/<slug>/intent.toml
status = "documented"          # or "measured" (the default)
source = ["https://…"]         # required for status = "documented"

[requires]
product = "<exact /sys/class/dmi/id/product_name>"
sys_vendor = "<exact /sys/class/dmi/id/sys_vendor>"
```

`status`/`source`/`[requires]` may appear **only** in a profile; the base and the operator layer
refuse them, because "this package is for laptop X" is a property of the selected layer, not of
the package.

## What a documented profile carries

- `[requires]` — exact DMI strings. Get them from a hardware probe (`linux-hardware.org`) or the
  project's own device page, **never** from marketing copy. A near-miss string is a profile that
  never selects; a wrong one is a profile that selects the wrong machine.
- `ceil.toml` — one **band** class per documented clock:

  ```toml
  [[class]]
  name = "7840hs"
  khz = 5100000        # the documented maximum boost clock
  [[class]]
  name = "7940hs"
  khz = 5200000
  ```

  A class without `cpus` applies by frequency: the highest declared ceiling a policy's
  `cpuinfo_max_freq` admits. This is the only honest way to cap a machine whose CPU numbering we
  have never observed, and it is what makes **one profile per DMI string** correct even when
  that string covers SKUs at several clocks — each unit takes the band it can reach, so nothing
  is capped needlessly and no ceiling above `cpuinfo_max_freq` is ever attempted. Declare every
  SKU clock the product string covers; two bands at the same clock are refused as ambiguous. A
  class with `cpus` (CPU-number based) is what a *measured* profile uses.
- `ryzenadj.toml` — only if the envelope is published **in ryzenadj terms** (`stapm`/`fast`/
  `slow`, mW). A vendor "TDP" is not one: leave it out and let `SMU` report `SKIPPED`.
- `passes.toml` — only genuinely per-machine entries. `wifi_interface` and `settle_secs` are
  fine; `wake_disarm` and `usb_autosuspend` are live observations and belong in a measured
  profile.

It carries **no** `mirror/` and **no** `managed/`: a file deployed to a machine we have never
booted is unverifiable by construction, and the base already ships everything generic.

## What a measured profile adds

Everything above, plus: `[co]` offsets from an actual sweep, `wake_disarm` from
`/proc/acpi/wakeup`, `usb_autosuspend` from `lsusb`, `ceil.toml` classes with explicit CPU sets,
and `mirror/`/`managed/` entries for this machine's own files. That is what
`intent/profiles/zenbook-s16/` is.

A measured profile is still **package data**: it ships in `/usr/lib/genarch-settings` and is read
by whoever installs the package. So "this machine's own files" means the *model's* files — a
device rule, a vendor config — never a value unique to one unit or one person. Disk UUIDs, an
`/etc/fstab` naming them, a partition layout, a path under `/home/`: those are `L` (live-only) for
a measured profile too, and they belong in the **operator layer** (`/etc/genarch-settings`),
where the engine reads them as an ordinary layer and they never leave the machine.

## When a machine cannot be covered

Some vendors do not publish a stable machine identity. Lenovo is the worked example: on a
ThinkPad Z13 Gen 2, `product_name` **is the MTM** (`21JVCTO1WW`, `21JV0017MX`, …), and PSREF lists
**1,276** MTMs across two machine types, with two different CPUs (7840U at 5.1 GHz, 7540U at
4.9 GHz). Exact matching would therefore ship a profile for two or three arbitrary MTMs and claim
coverage of a thousand — the worst of both worlds.

So the Z13 Gen 2 is **not covered**, deliberately. Shipping it needs one of:

- per-MTM profiles (correct, and only for the MTMs you actually own), or
- a prefix rule in `[requires]` (e.g. machine type `21JV`), which is a matching-mode change and
  a different design question — see `docs/DESIGN.md` “Open”.

Recording the blocker is the honest result. A profile that never selects is not coverage.

## The gate

`cargo test --test profiles` loads **every** directory under `intent/profiles/` as if it were
selected and refuses:

- a name that disagrees with the directory, or `[requires]` that is empty;
- any profile other than the measured reference that does **not** declare
  `status = "documented"` — `measured` is the default, and a profile that merely omits the field
  would silently skip every rule below;
- `status = "documented"` with no `source`, with a `mirror/`, a `managed/` record, a `[co]`
  offset, a wake list, a USB list, or a `ceil.toml` class that names CPUs (a documented profile
  declares clocks, not a stranger's core layout — a wrong core map caps the wrong cores on
  another SKU that shares the product string);
- any path under `/home/`, `/dev/`, `/sys/`, `/proc/`, `/var/lib/` or a `UUID=` in **any**
  shipped profile — file names and, for a managed record, its `target`. A measured profile may
  own the machine's `/etc` files; a user's home is the operator layer's business, because the
  package is read by every host that installs it;
- a ceiling outside 500 MHz–10 GHz (the `470000` / `4700000` unit mistake), an empty class map,
  or two bands at the same clock.

Adding a profile is therefore: write the directory, run `cargo test --test profiles`, fix what it
says. It fails on a *plausible* profile as well as a broken one, which is the point.

## Selecting it

```sh
printf 'profile = "framework-13-amd-7040"\n' | sudo tee /etc/genarch-settings/profile
sudo genarch-settings apply --dry-run    # read the plan
sudo genarch-settings apply
genarch-settings report                  # PROFILE must be OK; the rest says what was judged
```

On a machine whose DMI does not match, `PROFILE` reports DRIFT and `apply` exits 1 — the writes
still happen (you have to be able to see the plan), but nothing pretends the profile fits. There
is no `--force`.

## Rows a documented profile leaves unjudged

`CO`, `POWER` and `SMU` report `SKIPPED` unless the profile declares an offset, the power-saving
flag or an envelope. `MIRROR` counts zero because a documented profile ships no file. That is the
honest reading of "documented": `CEIL` and `PROFILE` are green because they were verified against
a publication, and everything measured on real hardware is left to the operator who owns it.
