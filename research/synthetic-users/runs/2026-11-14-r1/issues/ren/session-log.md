# Session log — ren, round 1

## World

- Card: `card.md`; machine: a headless Arch guest (`genarch-settings 0.4.0-1`,
  `ryzenadj 0.19.0-1`, kernel 7.2.11-arch1-1, 2 vCPU). `~/payload` holds the two package
  tarballs and `MANIFEST.txt`.
- `~/machine/README`: `root/` is a rehearsal tree with this machine's DMI, two fake cpufreq
  policies and the usual `/etc` targets. The documented command is
  `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report`.
- The real guest has **no SMU**, **no wireless interface** and **no cpufreq**; the staging tree
  supplies the DMI and two policies. Those absences are recorded below as limitations, not
  issues.
- Installed with `pacman`: `strace 7.3` (for the syscall evidence). `jq` is not installed; JSON
  was read with `python`.
- The package's real `/etc/genarch-settings/profile` ships `profile = "zenbook-s16"` (the retail
  laptop); on this guest `genarch-settings report` (no root override) therefore shows PROFILE
  drift. That is the documented "package ships this host's selection" behaviour and I did not
  change the real `/etc`.

## What I did, in order

1. Read the card and `~/machine/README`; listed the payload and the package file list
   (`pacman -Ql`). Read every shipped document (`AGENTS.md`, `CONTEXT.md`, `DESIGN.md`,
   `SCOPE.md`, `CONTRIBUTING.md`, `install.sh`) and every shipped profile.
2. Baseline: `GENARCH_SETTINGS_ROOT=~/machine/root genarch-settings report` → `PROFILE SKIPPED`,
   4 drift rows (MIRROR, MANAGED, SYNC, TIMER), 6 skipped, `exit=1`.
3. Read the staging DMI (`product_name = Standard PC (i440FX + PIIX, 1996)`,
   `sys_vendor = QEMU`, `product_version = QEMU Virtual Machine`, `bios_version = 1.0`) and the
   two policies (`cpuinfo_max_freq=3500000`, `scaling_max_freq=4000000`).
4. Wrote a minimal operator profile and selected it; PROFILE changed from SKIPPED to OK. Then
   wrote the final profile `qemu-i440fx` (below), applied it, and converged the ceiling.
5. Probed profile matching and load-time validation: wrong product/vendor, a selection that
   names a missing profile, unknown `[requires]` key (`product_version` refused; readable keys
   are product, sys_vendor, board, bios_version), `board` unreadable, `bios_version` mismatch,
   `documented` without `source`, invalid `status`, **no `[requires]` at all**, an empty profile
   directory, an overlay profile with the same name as a shipped one (merges per key), and
   malformed selection files. All fail loudly except the no-`[requires]` cases (issue 02).
6. Applied and converged copies of the tree, verifying mirrors, managed records, sysctl writes,
   pre-images/backups, modes (0755 hooks stay 0755; 0644 backups), the `system-sleep-backups`
   staging directory, the manifest (`owned`/`touched`), and `report`/`apply --dry-run`
   read-only-ness with `strace` (no writes; only live `systemctl is-enabled` reads for TIMER).
7. Exercised the layers:
   - one operator `sysctl.toml` key overriding base (`vm.swappiness` 150 → 60);
   - profile and operator `mirror/` overriding a base file, and revert on removal;
   - operator tombstone disabling a base mirror (live path), profile tombstone disabling a base
     mirror, base tombstone naming a profile/operator mirror (refused);
   - `[units]` union across base, profile and operator;
   - a deselected profile's mirror stays owned (dormant) and is not pruned; deleting the profile
     hands it back and it is pruned;
   - mirror+managed overlap refused ("one path, one owner");
   - a deployable-name violation in a profile mirror fails the intent load;
   - a bare dotted sysctl key fails with the quoting hint; a v2 intent fails.
8. Traced a rehearsal with `strace`: `GENARCH_SETTINGS_ROOT` does **not** contain
   `ryzenadj`, `iw`, `systemctl` or `udevadm` (issue 03).
9. Timed the passes: `converge boot` with `settle_secs = 5` took 5013 ms (the documented settle
   delay), `converge resume` took 5 ms (no sleep, as documented).
10. Found the projection collision (issue 04), the wrong-machine refusal (issue 05), the
    `harvest` gap (issue 06), the missing provenance (issue 07) and the no-profile exit code
    (issue 08). Wrote `issues/01…08`.
11. Left the authored profile selected in `~/machine/root`; all other rehearsal copies removed.

## The port: profile, selection, verification (recipe)

Written in the operator overlay so a package upgrade cannot overwrite it:
`~/machine/root/etc/genarch-settings/profiles/qemu-i440fx/` (a directory under `profiles/` plus
one line in `/etc/genarch-settings/profile`, as `SCOPE.md` puts it).

`intent.toml`:

```toml
# QEMU standard PC (i440FX + PIIX) — the staging box this profile was written for.

# Measured on the machine (DMI read 2026-11-14):
#   product_name    = Standard PC (i440FX + PIIX, 1996)
#   sys_vendor      = QEMU
#   product_version = QEMU Virtual Machine
#   bios_version    = 1.0
# product + sys_vendor identify the model; product_version is not a key the loader
# reads (it names product, sys_vendor, board, bios_version), and bios_version is
# not stable across firmware updates.
status = "measured"

[requires]
product = "Standard PC (i440FX + PIIX, 1996)"
sys_vendor = "QEMU"
```

`ceil.toml`:

```toml
# One band: both policies admit 3.5 GHz (cpuinfo_max_freq), so that is the ceiling.
# A band is used rather than CPU numbers because this profile names no CPU layout.
[[class]]
name = "qemu-vcpu-3.5"
khz = 3500000
```

Selection: `printf 'profile = "qemu-i440fx"\n' > ~/machine/root/etc/genarch-settings/profile`

Verification:

```sh
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
genarch-settings report            # PROFILE OK, CEIL DRIFT (band not applied yet), timer/etc.
genarch-settings apply             # 56 changes; the copy converges
genarch-settings converge ceil     # CEIL DRIFT policy0/1 4000000 -> 3500000
genarch-settings report            # see below
```

Final report:

```
OK PROFILE profile 'qemu-i440fx' matches product=Standard PC (i440FX + PIIX, 1996) sys_vendor=QEMU findings#intent
OK MIRROR 39 mirror target(s) byte-equal findings#intent
OK MANAGED 2 record(s) hold their line findings#managed
OK SYNC 16 runtime sysctl value(s) match intent findings#writeback
OK CEIL all 2 policies capped qemu-vcpu-3.5 3500000 findings#ceil
SKIPPED CO no curve-optimizer offset declared (the silicon is the profile's to describe) findings#co
SKIPPED SMU no SMU envelope declared (the silicon is the profile's to describe) findings#ppt
SKIPPED POWER no --power-saving declaration (the silicon is the profile's to describe) findings#eco
SKIPPED WIFI profile declares no wifi_interface findings#wifi
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
OK PRUNE 41 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=6 accepted=0 warn=0 drift=1 skipped=4 exit=1
```

The remaining TIMER drift is structural in a rehearsal: TIMER asks the live systemd whether the
package's units are enabled, and a rehearsal root cannot enable units in the live systemd (and I
did not write the real `/etc`). On a host installed by `install.sh` the units are enabled.

## Could the layer exercises be done from the shipped material alone?

- **Change one value without copying the whole file** — yes. `CONTEXT.md` states the merge rule
  and the operator path, and the shipped `sysctl.toml` shows the file shape. Adding
  `[values]` with one key under `/etc/genarch-settings/sysctl.toml` changed only that key.
- **Disable one shipped file without forking it** — the *concept* is documented, the *syntax* is
  not. `[omit]` is mentioned only in prose; the key is the live path for `mirror` and the record
  name for `managed`, and the two differ. I found them by trial and error against the error
  messages (issue 01). A copy of the base tree was not needed.
- **Show which layer wins** — the winning *value* is visible; the *layer* is not. Precedence is
  documented and behaves as documented (base 150 → profile 120 → operator 60), but no command or
  JSON field names the source (issue 07). I had to remove layers one at a time.
- **A profile in the overlay with the same name as a shipped one** — yes; it merges per key. The
  docs name the overlay path (`{base,overlay}/profiles/<name>`), though not the merge example.
- **Units union, dormant ownership, tombstone direction, deployable names** — all documented and
  observed.

## What I could not do / limitations

- No SMU: `sudo ryzenadj --info` → `Error: Unable to get SMU object (no AMD SMU on this machine)`.
  SMU/CO/POWER rows are therefore declared-only or SKIPPED; the boot/resume passes invoke
  `ryzenadj` and fail. Hardware absence, not an issue.
- No wireless interface: `WIFI` is SKIPPED (`iw unavailable or interface absent`; `iw` is
  installed, the interface is not).
- No cpufreq on the real guest; the staging tree's two policies stand in.
- Could not enable the package's units on the live guest without writing the real `/etc`; TIMER
  cannot be made green for a rehearsal root. For the no-profile demo I stubbed `systemctl
  is-enabled` with a script on `PATH` and said so in issue 08.
- Could not test a real-laptop SMU write, a resume with a real wireless device, or the
  `import` verb (no legacy `state/` tree here).
- Could not determine whether `docs/PROFILES.md` exists in the source repository (no source on
  this machine); only that it is not installed and the installer does not install it.
- The package's real selection (zenbook-s16) and real report are drift, as expected for a
  non-ASUS host; I left the real `/etc` untouched.

## Observed but not filed (8-issue cap, or hardware-absence caveat)

- `genarch-settings report | true` panics: "failed printing to stdout: Broken pipe (os error
  32)", **exit 134**, outside the documented 0/1/2/3 contract. Reproduced with `report | true`
  and `report --json | true`. A porter piping to `head`/`grep -m1` can hit it. Real, but not
  porting-specific enough to displace the filed issues.
- A boot/resume pass exits **0** when every `ryzenadj` call fails (`smu: 9 of 9 ryzenadj
  call(s) failed — firmware limits may still stand`), while a refused `scaling_max_freq` write
  in the same pass exits 3. `AGENTS.md` says "A refused write in a pass is a failure (exit 3),
  never `.is_ok()`". The trigger here is absent hardware, which the brief says to record as a
  limitation; I am recording the exit-code observation, not filing it.
- `apply` prints `SUMMARY planned=2 applied=1` when the plan contains a prune (the prune is not
  counted as applied). Cosmetic.
- The SMU row's SKIPPED detail says "needs root, or ryzenadj is missing" even when `ryzenadj` is
  installed and the real reason is a missing SMU; it conflates two causes.
- Every report row's `doc` column is a `findings#…` anchor; there is no `findings.md` on the
  machine (folded into issue 01).

## Issues filed (in the order a porter meets them)

1. `01-profile-contract-doc-absent.md` — **high / doc** — `docs/PROFILES.md` is cited everywhere
   and installed nowhere; `docs/DESIGN.md` is a dead path; every `findings#…` anchor is
   unresolvable; the `[omit]` key is undocumented and differs between `mirror` and `managed`.
2. `02-profile-without-requires-matches-any-machine.md` — **high / bug** — a selected profile
   with no `[requires]` (or an empty directory) is `OK` and its machine facts are applied.
3. `03-rehearsal-root-executes-live-tools.md` — **high / bug** — `GENARCH_SETTINGS_ROOT` still
   runs the live `ryzenadj`, `iw`, `systemctl daemon-reload` and `udevadm control --reload`.
4. `04-mirror-can-shadow-the-sysctl-projection.md` — **high / bug** — a profile mirror at
   `/etc/sysctl.d/90-genarch-settings.conf` is accepted; apply writes the projection over it and
   every later apply exits 1 with "drift persisted".
5. `05-wrong-machine-profile-refused-exit-3.md` — **medium / bug** — selecting a profile for
   another machine is a hard exit-3 refusal ("nothing was written"), not the documented "apply
   still runs and exits 1".
6. `06-harvest-silent-on-sysctl-and-ceiling-drift.md` — **medium / bug** — `harvest` prints
   nothing and exits 0 for SYNC/CEIL drift, indistinguishable from a converged host.
7. `07-no-layer-or-provenance-in-output.md` — **medium / enhancement** — the report shows the
   winning value but never the layer, and the JSON omits the profile's `status`/`source`.
8. `08-no-profile-selected-exits-zero.md` — **low / enhancement** — a host with no profile
   selected exits 0 once the other rows are green; a fleet audit must parse verdict text.

## Handover: what I would give the next person porting a third model

1. **Where the profile goes.** Create `/etc/genarch-settings/profiles/<slug>/` on the installed
   host (the overlay), never inside `/usr/lib/genarch-settings` (package data, replaced on
   upgrade). Select it with one line in `/etc/genarch-settings/profile`.
2. **`[requires]` is mandatory in practice.** Use `product` + `sys_vendor` exactly as DMI
   reports them; `board` and `bios_version` are readable keys, `product_version` is not. Do not
   trust a green PROFILE row if you left `[requires]` out: issue 02 means the row is green and
   facts are applied to any box. Verify with `report --json` (`profile.matched`) and re-read
   your file.
3. **Ceilings as bands.** Declare `[[class]]` with `khz` and no `cpus`; the highest band the
   unit admits applies by `cpuinfo_max_freq`. A band below 500 MHz or above 10 GHz is refused;
   a machine admitting less than every band reports "this profile does not describe this
   machine". Use `converge ceil` (or the timer) to apply; `apply` does not touch ceilings
   (`APPLY-SCOPE … none=CEIL`).
4. **`sysctl` via `sysctl.toml`, not a mirror.** A mirror at
   `/etc/sysctl.d/90-genarch-settings.conf` is accepted and then fights the engine's projection
   forever (issue 04). Use `[values]` with quoted dotted keys.
5. **Rehearse read-only.** `GENARCH_SETTINGS_ROOT` is safe for `report` and `apply --dry-run`,
   but `apply`/`converge` still run the live `systemctl`, `udevadm`, `ryzenadj` and `iw` (issue
   03). On a real machine, do not run `converge boot/resume` against a rehearsal root while the
   profile declares `[co]`/`[limits]`/`wifi_interface`, and expect the reloads to be live.
6. **Layers.** One value: `${GENARCH_SETTINGS_ROOT}/etc/genarch-settings/<file>.toml` with only
   the keys you change (scalar maps merge per key; lists replace). One file off: `[omit]` in the
   operator `intent.toml`, keyed by **live path** for `mirror` and by **record name** for
   `managed` (issue 01). A profile may omit a base file; a base layer may not omit a profile or
   operator file. Units are the one union.
7. **`harvest` is not a value harvester.** It proposes mirror/managed changes only; a sysctl or
   ceiling drift produces no output (issue 06). Do not use its empty result as "converged";
   read `report`.
8. **TIMER in a rehearsal.** It is always DRIFT for a rehearsal root because it asks live
   systemd; that is not a fault in your profile. On the installed host enable the units
   `genarch-settings units` prints (or use `install.sh`).
9. **Audit trail.** The verdict anchors point at a ledger that is not installed, and the report
   does not say which layer won. Keep the layer files in your own change control; the output
   alone is not enough for an auditor (issues 01, 07).
