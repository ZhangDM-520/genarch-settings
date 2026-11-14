# session log — kaz, round 2

Box: QEMU VM, Arch Linux, kernel 7.2.11-arch1-1, 2 vCPU, genarch-settings 0.4.0-1 (fresh
install from `~/payload`), ryzenadj 0.19.0-1. Nothing below touched the real `/etc` except the
package operations (`pacman -U`, `pacman -R`) and read-only commands.

| time | what I did | result |
|---|---|---|
| 0 | read `card.md`, `sysinfo.txt`, `~/payload/MANIFEST.txt`, `~/machine/README`, `~/machine.sh` | the staging tree is a rehearsal root with a `vm-generic` operator profile; `machine.sh` rebuilds it, which is my reset button |
| 2 | `pacman -Ql genarch-settings`, `--version`, `--help` | 0.4.0, eleven rows promised, docs under `/usr/share/doc/genarch-settings/`; no unit under `/usr/lib/systemd/system` |
| 4 | `sudo pacman -U` the payload (reinstall) | **pacman EXIT:0**, `SUMMARY applied=0 failed=1 exit=3`, `nothing was written`, `Failed to enable unit: Unit genarch-settings.service does not exist`, 0 genarch unit files |
| 7 | `genarch-settings report` on the real box | PROFILE DRIFT (zenbook-s16 vs QEMU), 41 mirrors differ, managed unclassifiable, 13 sysctls drift, CEIL SKIPPED, TIMER DRIFT, exit 1 |
| 9 | `GENARCH_SETTINGS_ROOT=~/machine/root report` / `--json` | JSON schema 2, 11 checks, `reach` per row, stable across two runs (`diff` clean) |
| 12 | `apply --dry-run`, then `apply` in the staging root | 56 changes, `BACKUPS 2`, `SUMMARY planned=56 applied=56 failed=0 exit=0` |
| 13 | report after that apply | `DRIFT CEIL`, `DRIFT TIMER`, `SUMMARY … exit=1` — apply exited 0 on a host that is still red |
| 15 | `converge ceil` in the staging root | fixes CEIL (`policy0 4000000 -> 3000000`), `converge resume`/`boot` report `rehearsal root: not executing ryzenadj` and exit 3 |
| 17 | retest 2: delete the `ppd-conflict` target, `apply` | `nothing was written`, exit 3 — all 55 other changes blocked |
| 19 | retest 4: `chmod 0444` on `proc/sys/vm/swappiness`, `apply` | `SUMMARY applied=0 failed=1 exit=3` + one error line, but 40 mirrors and 2 managed files had landed |
| 21 | retest 5/6: drift a sysctl, a managed line and a mirror; `harvest` | sysctl and managed diffs now print (intent → live), mirror prints the live file as `+` lines |
| 23 | retest 8: shim `ryzenadj systemctl udevadm modprobe iw sysctl` on PATH, rehearsal `apply`/`converge boot|resume|ceil` | shim log never created: no external command runs under a rehearsal root |
| 27 | retest 7: `sudo pacman -R genarch-settings`, then reinstall | removal prints the contract ("deployed files … left in place", "pre-images … sit beside"); nothing was deployed on this box, so no leftovers to inspect |
| 30 | second same-day apply over a drifted managed line | the `*.bak-20261114` pre-image is **not** overwritten — good |
| 32 | doc sweep: every `docs/…` reference in the shipped docs vs `pacman -Ql` | `docs/CUTOVER.md` (the rollback runbook) is named in README:143 and not installed |
| 35 | `report | tail -c 40 | od -c` | SUMMARY line ends without a newline; `apply`'s does not |
| 38 | wrote `retest.md` and four issue files | — |

## Could not do

- Reproduce "uninstall leaves what the engine wrote": on this box `apply` never succeeds, so no
  manifest and no deployed file ever exists to leave behind. I judged the removal *message* and the
  README's Removal section instead.
- Reach a clean (exit 0) report on the real box at all: the package ships the retail laptop's
  profile selection, so every install here starts with PROFILE DRIFT and an unclassifiable plan.
- Exercise a real SMU, cpufreq policy or wireless interface on the real box (hardware absent; the
  staging root stands in). Recorded as a limitation, not an issue.
- Watch `pacman -U` enable a unit: the units only appear after an apply, and the apply fails here.
