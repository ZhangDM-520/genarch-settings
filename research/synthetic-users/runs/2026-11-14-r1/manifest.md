# round 1 — cold discovery

- run: 2026-11-14-r1
- started: 2026-11-14T22:30+08:00
- under test: commit `8a02f4683ea0609898b45ccff08764f2c7c52c73` (engine `master`, 6 commits after `v0.4.0`)
- payload: `genarch-settings-0.4.0-1-x86_64.pkg.tar.zst` sha256
  `7bf5f83f3948a9570f4d0911d204f8560e708e52306fd960773415ce197af7b4`
- environment provider: `ryzenadj-0.19.0-1-x86_64.pkg.tar.zst` sha256
  `57533cc90ca0483e314714206fb2d2256258cf0990f5a45760d921356e3bd883`
  (a stub: the real dependency is AUR-only and there is no SMU in a VM)
- baseline `cargo test` at the pinned commit: 26 lib + 4 cli + 4 contract + 34 converge +
  15 layers + 5 profiles, all pass
- environment: 4 × QEMU/KVM, Arch cloud image, guest kernel `7.2.11-arch1-1`,
  2 vCPU / 2048 MiB / no display, one per persona, destroyed after the round
- isolation: each persona received only a card, `sysinfo.txt`, an ssh wrapper (`machine`), the
  installed package, the package files in `~/payload`, and a staging tree at `~/machine`
- staging tree: a copy of a filesystem with the guest's own DMI, two synthetic cpufreq policies
  (`scaling_max_freq` 4.0 GHz, `cpuinfo_max_freq` 3.5 GHz), the sysctl surface the base intent
  asserts, a `journald.conf` and a `power-profiles-daemon.service` for the managed records, and an
  operator profile `vm-generic` (except ren, who got the bare tree) — so plan/apply/prune can be
  exercised without the retail laptop's hardware
- assignment (fixed): ivy→`r1-ivy`, kaz→`r1-kaz`, ren→`r1-ren`, sol→`r1-sol`

## Known deviation, recorded before the round

Subagents inherit the parent's project instructions and cwd, so an "outside" agent could in
principle see this repository. The run directory is outside the tree
(`~/.workspace/PERMANENT/su-genarch/rounds/r1/<persona>/`) and the leak audit measures whether
contamination reached a transcript; it cannot remove the instruction context. This round is
therefore **outside-user with a recorded instruction-leak limitation**; the audit result is
reported per persona rather than assumed.

Round-1 question: can a competent stranger install this, understand what it did, and trust it with
a machine; and does the engine's general logic survive on a machine it was not written for?

Cards as handed over: `persona-cards/<id>.md`. Issues land in `rounds/r1/<persona>/issues/`.
