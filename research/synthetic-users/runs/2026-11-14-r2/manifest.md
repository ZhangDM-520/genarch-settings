# round 2 — blind retest of the round-1 issues

- run: 2026-11-14-r2
- started: 2026-11-14T23:35+08:00
- under test: commit `71543d855e00e3125bdf565438c1fc5f6dad81bf` (round-1 fixes on `master`)
- payload: `genarch-settings-0.4.0-1-x86_64.pkg.tar.zst` sha256
  `ca0b6b8484a5f8a419727a5362fefdc32cbc39cb812e77ad422e2fd59a84f294`
- environment provider: `ryzenadj-0.19.0-1-x86_64.pkg.tar.zst` sha256
  `74f1cb41bf015e8ae02dd2f97bb303f6b5494d893528fd425d31bb4cc25f1216` (stub, as in round 1)
- gate at the payload commit: 26 lib + 4 cli + 4 contract + 38 converge + 16 layers + 5 profiles,
  all pass
- environment: 4 × QEMU/KVM, Arch cloud image, guest kernel `7.2.11-arch1-1`,
  2 vCPU / 2048 MiB, one per persona, destroyed after the round
- isolation: card + `sysinfo.txt` + ssh wrapper `machine` + installed package + `~/payload` +
  staging tree `~/machine` (ren: bare, no operator profile — same as round 1). The known
  instruction-leak limitation from round 1 is unchanged.
- assignment (fixed): ivy→`r2-ivy` 22644, kaz→`r2-kaz` 22643, ren→`r2-ren` 22795, sol→`r2-sol` 22829
- round-2 question: for each issue the persona filed in round 1, is it still, gone, or different —
  judged by the same stranger, who is told nothing about what changed.

Verdicts land in `rounds/r2/<persona>/retest.md`. New issues land in `rounds/r2/<persona>/issues/`.
