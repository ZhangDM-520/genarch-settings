# Synthetic-user review — 2026-11-14

Four personas who never saw the source installed the package on disposable Arch VMs and used it.
They filed what they hit first; the engine was fixed; the same strangers retested the fixes without
being told what changed. Round 3 attacked the result and diffed it against the previous build.

- under test: `8a02f4683ea0609898b45ccff08764f2c7c52c73` (v0.4.0 + 6 commits)
- fixes: `71543d8` (round 1), `0371256` (round 2), plus the documentation clarifications in the
  commit that adds this report
- gate at each fix: `cargo test` (98 tests after round 2) and `cargo clippy --all-targets`, clean
- harness, per-round manifests, verdicts and raw transcripts: `research/synthetic-users/`

## Method

- Four sealed persona cards (`research/synthetic-users/runs/2026-11-14-r1/persona-cards/`): ivy, a
  headless homelab admin; kaz, a solo developer; ren, a porter adding a second machine; sol, a
  sceptic who tests failure paths. 40 minutes each, enforced from outside the agent.
- One QEMU/KVM Arch VM per persona (2 vCPU, 2 GiB), destroyed after each round. The payload is
  exactly the two package files and `MANIFEST.txt`: no source tree, no internet research, no repo
  instructions in the review directory.
- A staging tree (`~/machine`) carries the VM's own DMI, two synthetic cpufreq policies, the
  sysctl surface and a `vm-generic` operator profile, so plan/apply/prune/passes can be exercised
  without the retail laptop's hardware. ren got the bare tree (no operator profile).
- Round 1 cold. Round 2 blind: each persona was given the titles it filed and asked
  still / gone / different, with evidence; never what changed. Round 3: an adversarial battery and
  a differential of the prior release payload against the current one through one fixed script.
- Leak audit over every outside-track transcript and issue: 0 hard leaks on all four personas,
  falsified both ways (a planted leak scores 1, an empty directory exits non-zero).

## Numbers

Computed from the session ledger (`su_issues`), not narrated:

| metric | value |
|---|---|
| rows filed | 52 (34 in round 1, 18 in round 2) |
| unique findings | 35 (17 cross-persona duplicates) |
| by category | 35 bug · 12 doc · 5 enhancement |
| closed | 25 (12 `fixed`, 13 `retested-gone`) |
| decisions (deliberate, not fixed) | 9 |
| still live after round 2 | 15 |
| unreproduced / debris | 0 / 0 |

Round-2 verdicts on the 34 round-1 rows: **14 gone, 8 different (partly fixed), 10 still**.

## Round 1 — what a stranger hit first

The install story failed first, then the accounting. `pacman -U` exited 0 while `post_install`
converged nothing and no unit existed; one absent managed target refused every change; a failed
apply printed no summary; `harvest` was silent on sysctl drift and had its managed diff backwards;
`GENARCH_SETTINGS_ROOT` still executed live tools; a pass whose every SMU write failed exited 0; the
manifest claimed mirrors a fresh install never wrote; `docs/PROFILES.md` and the rollback runbook
were not installed.

Fixed in `71543d8`: loud install hook, `SUMMARY` on a failed apply, harvest sysctl/ceil proposals and
the managed direction, manifest ownership (`shipped_targets` vs `owned_targets`), refusal to execute
external tools under a rehearsal root, exit 3 when every SMU write fails, the projection-path refusal,
and the packaging/documentation holes.

## Round 2 — does the fix survive a stranger?

The personas were told only their old titles. What they found:

- **Gone (14):** the all-SMU-failed pass, silent harvest (sysctl, ceil, managed direction), the
  rehearsal executing tools, the projection-path mirror, manifest ownership of never-written files,
  the missing profile/layer contract, the pre-image-per-day gap.
- **Different, still wrong in part (8):** the install now says it failed (still exit 0); the docs are
  mostly installed (the runbook was not); uninstall now explains itself (still no restore command);
  provenance now names the profile (still not the winning layer).
- **Still (10):** the base `ppd-conflict` record refuses every change on a host without
  power-profiles-daemon; `pacman -U` exits 0 on a refusal; a wrong-machine profile is an exit-3
  refusal rather than the documented exit 1; a profile without `[requires]` is reported OK; a host
  with no profile exits 0 when green.

Round 2 also found new defects in the fixes themselves, fixed in `0371256`:

- `apply --dry-run` exited 3 on an unclassifiable intent although the README promises a preview
  always exits 0. It now prints the plan (`ANOMALY` lines included), writes nothing, exits 0.
- A failed apply reported `applied=0` after 34 files had landed. `Failure` carries the count now
  (`SUMMARY applied=27 failed=1 exit=3` on the same reproduction).
- `report` did not terminate its `SUMMARY` line.
- The lock refusal rendered an empty holder; it now says "an unidentified process".
- TIMER was judged with `systemctl is-enabled` under a rehearsal root — a false DRIFT for a copy. It
  is SKIPPED there, which is what unverified means.
- Documentation: `CUTOVER.md` ships, the flat `docs/…` mapping is stated, the `[omit]` shape and the
  `ryzenadj.toml` tables are written down, and an installed host without cargo is told which profile
  check it has.

Every fix above was proven red-first (guard removed → its test fails) and verified again on the
installed payload (`research/synthetic-users/runs/2026-11-14-r2/verify.md`).

## Round 3 — adversarial and differential

Adversarial (`runs/2026-11-14-r3/adversarial.md`): empty base, half-written TOML, unknown key,
missing profile, unprivileged apply, `SIGKILL` mid-apply, remove → reinstall, stale pid text, renamed
binary. Every case either refused loudly with exit 2/3 or recovered; `SIGKILL` leaves landed files
outside the manifest until the next apply adopts them, which the accepted decision predicts and which
the re-apply confirmed.

Differential (`runs/2026-11-14-r3/differential.md`), prior v0.4.0 vs current, same fixed battery:
**9 better, 7 same, 0 worse** — better on the dry-run contract, SIGPIPE, the report terminator, TIMER
under a rehearsal, harvest, the all-SMU-failed pass, partial-apply accounting, the lock message and
the installed documentation.

## Decisions (deliberate, digest)

1. An unclassifiable intent is refused whole, exit 3, nothing written (pinned by
   `managed_zero_match_refuses_and_writes_nothing`; the message now names the `[omit]` escape hatch).
2. A package install stays non-fatal (exit 0) even when `apply` refused: a boot chain must not break.
   Gate automation on `report`, which is the contract that never lies.
3. A partial or killed apply leaves its landed files outside the manifest until the next apply; a
   write-ahead manifest was rejected as the larger change.
4. Pre-images are one per target per day; a second write the same day reuses the first.
5. `apply`'s 0 means the plan `apply` can reach was applied; `CEIL` and `TIMER` are reach `none`, so a
   green apply never implies a green report.
6. A profile without `[requires]` is reported OK: only a declared requirement can mismatch.
7. With no profile selected the intent is the base and a green host exits 0 — never a guessed default.
8. A pass whose writes are refused exits 3, including a refusal under a rehearsal root.
9. The package ships this model's profile and its selection; on another machine `PROFILE` drift is the
   signal to write the operator's selection line.

## Still open after round 2

- The install's exit status (4 rows, one root cause) and the base `ppd-conflict` record (2 rows,
  decision 1) — the two a deployment script trips on first.
- A wrong-machine profile exits 3 rather than the documented 1 when a record is unclassifiable; the
  documentation now says both outcomes instead of pretending one.
- A failed apply still does not print the plan it was executing; `apply --dry-run` is the preview.
- Uninstall documents what it leaves but has no restore command.
- Provenance is per profile, not per winning layer (enhancement).
- A no-profile host exits 0 when green (enhancement; the review's own reading is that this is right).

## Would they keep it installed?

Judged, not computed, from the retest verdicts: yes for three of four. The plan/apply engine, the
manifest, the passes and the rehearsal isolation survived a stranger's misuse; what still costs trust
is the package-manager exit code and the all-or-nothing refusal on a base record the host may not
have. The two are decisions with a documented cost, and the differential shows the fixes made nothing
worse.

## Limitations

- A VM has no AMD SMU, no wireless interface and no real cpufreq policy: those rows are SKIPPED and
  were reviewed by the rehearsal tree, not by hardware. Absolute performance was not judged.
- Subagents inherit the parent's project instructions and working directory. Each review directory
  lives outside the repository and the leak audit measures contamination in the transcript; it cannot
  remove the instruction context. This is an outside-user round with a recorded instruction-leak
  limitation, not a pristine clean room.
- One round per persona per build: the result is "what a competent stranger hit first", not a
  coverage proof.
