# phase 6 — verification on the installed payload

- verified on: `r1-verify`, Arch cloud image, guest kernel 7.2.11-arch1-1
- payload: the worktree build of the round-1 fixes (`genarch-settings 0.4.0-1`), installed with
  `pacman -U`; the `ryzenadj` provider is the same environment stub
- reference: the round-1 package (`8a02f46`) on four VMs

| defect | before (installed 8a02f46) | after (installed fix build) |
|---|---|---|
| closed stdout | `report \| true` → panic, exit **134** | exit **141** (SIGPIPE), no panic |
| `harvest` omits sysctl | 0 lines for a drifted sysctl | `--- sysctl vm.swappiness (live 60)` / `- 150` / `+ 60` |
| `harvest` managed direction | `- live` / `+ intent` | `- intent` / `+ live` (matches the mirror convention) |
| rehearsal executes tools | 2 real `ryzenadj` runs from `GENARCH_SETTINGS_ROOT=… converge boot` | 0 real runs; pass exits **3** naming the refusal |
| pass with every SMU write failed | `converge boot` exit **0**, 24 failures logged | exit **3** (`all 24 ryzenadj call(s) failed`) |
| manifest owns dormant paths | `98-bt-autosuspend.rules` (never written) in `owned` | not recorded; a deselected profile's written path stays owned and present |
| mirror at the projection path | accepted; every apply `drift persisted`, exit 1 | refused at load: exit **2**, `a mirror cannot also own it` |
| install hook silent no-op | `pacman -U` exit 0, nothing converged, no message | same exit 0, but `apply did not converge this host` on stdout |
| package docs | no README, no PROFILES.md | both installed under `/usr/share/doc/genarch-settings/` |
| refused apply accounting | error message only | `SUMMARY applied=0 failed=1 exit=3` + `nothing was written` |

Not fixable without a contract change (recorded as decisions): the all-or-nothing refusal of an
unclassifiable intent (exit 3), the once-per-day pre-image, and `report`'s exit 0 for a host with
no profile selected.

Regression check: the full gate after the fixes — 93 tests pass (26 lib, 4 cli, 4 contract,
38 converge, 16 layers, 5 profiles). The four new behaviour tests fail on the pre-fix source and
pass after it.
