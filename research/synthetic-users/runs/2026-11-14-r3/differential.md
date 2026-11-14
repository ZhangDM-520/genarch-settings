# round 3 — differential

Same fixed battery (`harness/diff-checks.sh`) on two installed payloads, same VM shape:

- prior: `04a1f9ef4f7b3990ff8e48e65db6743e1c744763` (v0.4.0 release payload)
- now: `0371256` (round-2 fixes)

Raw output: `prior.txt`, `now.txt`.

| check | prior | now | verdict |
|---|---|---|---|
| installed docs | AGENTS, CONTEXT, CONTRIBUTING, DESIGN, LICENSE, SCOPE, install.sh | + README, PROFILES, CUTOVER | **better** |
| `report` exit | 1 | 1 | same |
| `apply --dry-run` on an unclassifiable intent | exit 3, no plan | exit 0, `PLAN`, 6 ANOMALY lines | **better** |
| `report \| true` | 134 (SIGABRT) | 141 (SIGPIPE) | **better** |
| last byte of `report` | `1` | `\n` | **better** |
| rehearsal apply | 56/56, exit 0 | 56/56, exit 0 | same |
| re-plan is empty | 1 | 1 | same |
| TIMER under a rehearsal root | DRIFT | SKIPPED | **better** (no false drift) |
| `harvest` on a drifted `vm.vfs_cache_pressure` | silent | `--- sysctl … - 50 + 77` | **better** |
| `converge ceil` on the staging tree | exit 0, 3000000 | exit 0, 3000000 | same |
| `converge boot` with every SMU write failing | exit 0 | exit 3 | **better** |
| partial apply summary | exit 3, no summary | exit 3, `SUMMARY applied=27 failed=1` | **better** |
| lock held without a pid | `held by  ()` | `held by an unidentified process (no pid recorded)` | **better** |

**Worse: none.** Every row is same or better; the nine "better" rows are the round-1 and round-2
fixes observed from the outside, not from the test suite.
