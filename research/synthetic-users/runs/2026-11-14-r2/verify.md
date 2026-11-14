# phase 6 (round 2) — verification on the installed payload

- verified on: `r2-verify`, Arch cloud image, guest kernel 7.2.11-arch1-1
- payload: commit `0371256`, `genarch-settings-0.4.0-1` sha256
  `fe78ce3d3855a4f7b18e59bf07c948a5a00da58a4a391ec2eb5bbea202a6fceb`, installed with `pacman -U`
- reference: the round-2 build (`71543d8`) on the four persona VMs

| round-2 defect | before (installed 71543d8) | after (installed 0371256) |
|---|---|---|
| `apply --dry-run` on an unclassifiable intent | exit **3**, no plan | exit **0**, `PLAN`, 6 `ANOMALY` lines, `DRY-RUN nothing written` |
| a failed apply's accounting | `SUMMARY applied=0 failed=1 exit=3` after 34 files landed | `SUMMARY applied=27 failed=1 exit=3` |
| `report` last line | no trailing newline (`exit=1` then EOF) | ends `exit=1\n` (`od -c`) |
| lock held without a recorded pid | `apply lock held by  ()` | `apply lock held by an unidentified process (no pid recorded) (…)` |
| TIMER under a rehearsal root | DRIFT (the live machine's answer for a copy) | `SKIPPED TIMER rehearsal root: unit enablement is the live machine's, not the tree's` |
| installed documentation | no `CUTOVER.md`, dead `docs/` links, `install.sh` cites an absent PKGBUILD | `CUTOVER.md` installed; README explains the flat mapping; install.sh points at the source-tree PKGBUILD |

Regression check: the full gate after the fixes — 28 lib + 4 cli + 4 contract + 41 converge +
16 layers + 5 profiles = 98 tests, all pass; `cargo clippy --all-targets` clean. Every new guard
was falsified (guard removed → its test red, restored → green).
