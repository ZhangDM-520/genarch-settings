# session log — ivy, clean-room review, round 1

Machine: Arch guest (QEMU, 2 vCPU, 2 GiB, kernel 7.2.11-arch1-1), user `arch`, passwordless
sudo. `genarch-settings 0.4.0-1` was already installed system-wide; the payload is in
`~/payload`; the rehearsal tree is `~/machine` (README says `root/` is a fake live filesystem
with a QEMU DMI, two cpufreq policies and a `vm-generic` operator profile).

## What I did, in order

1. Read `card.md`, `sysinfo.txt`, `~/machine/README`, `~/payload/MANIFEST.txt`.
2. Read everything the package installs: `pacman -Ql/-Qi`, the `.INSTALL` hook from the pacman
   database, all of `/usr/share/doc/genarch-settings/` (CONTEXT, DESIGN, AGENTS, SCOPE,
   CONTRIBUTING, LICENSE, install.sh), the intent files, the units, the managed records, the
   profiles. `pacman -Qkk` reported 144 files, 0 altered.
3. Ran the real-host report (`sudo` and unprivileged; identical):
   `PROFILE` DRIFT (QEMU vs zenbook-s16), `MIRROR` 41, `MANAGED` unclassifiable (asusd + ppd),
   `SYNC` 13, `CEIL`/`SMU`/`WIFI` SKIPPED, `CO`/`POWER` OK, `TIMER` DRIFT, exit 1.
   `apply --dry-run` refused with exit 3 and wrote nothing.
4. Installed from the payload (`sudo pacman -U ...pkg.tar.zst`) twice — once as a reinstall and
   once after `sudo pacman -R` — to watch the hook on both paths. Both: apply refused, `systemctl
   enable` failed, `pacman` exit 0, nothing on disk. This became issue 01.
5. Explored the rehearsal tree. `report`, `apply --dry-run`, `apply`, re-`apply` (PLAN 0),
   `report --json` (schema 2). Verified the manifest (41 owned, 2 touched), the two pre-images,
   mode preservation on the 0755 system-sleep hook and the `/usr/local/lib` helpers, and that a
   second apply is idempotent.
6. Ran the passes on the staging root: `converge ceil` (fixed both policies), `converge boot`
   (re-asserted a changed sysctl; did not restore a deleted mirror — the unit's second ExecStart
   does that), `converge resume`. Boot/resume exit 0 with 2/2 ryzenadj failures (issue 04).
7. Tested the lock: a pid written into `var/lib/genarch-settings/lock` does not block; an
   exclusive `flock` on the same file does, and `converge ceil` prints
   `Ceil pass skipped: apply lock held by  (...)` and exits 0. So the lock protocol is `flock`,
   not the pid text. No issue.
8. Tested a refused sysfs write in a pass (`chmod 0444 scaling_max_freq`): `converge ceil` exits
   3, correctly. The SMU path is the exception (issue 04).
9. Failure-mode tests on copies of the staging tree:
   - malformed operator `sysctl.toml` (unquoted dotted key) → exit 2, loud;
   - selected profile does not exist → exit 2, loud;
   - `status = "documented"` without `source` → exit 2, loud (rule works);
   - base-only with the PPD unit removed → `apply` refuses everything, exit 3 (issue 02);
   - `[omit] managed = ["ppd-conflict"]` → the same host applies (PLAN 55), so the escape hatch
     works but no shipped example shows its syntax;
   - a managed file with two anchor matches → whole apply refuses, exit 3 (issues 02/03);
   - a read-only directory mid-plan → exit 3 after 8 files landed, no manifest and no summary;
     the retry wrote the full manifest, so ownership is recovered.
10. Profile-mismatch apply with the managed targets present (fabricated `/etc/asusd/asusd.ron`
    and the PPD unit): 62 changes applied, exit 1 for `PROFILE` drift. So the documented
    "apply still runs" path works when nothing is missing; the real-host failure is the absent
    targets.
11. `harvest` on a converged staging root with one changed sysctl: report says
    `DRIFT SYNC vm.swappiness=60 want 150`, harvest prints 0 lines (issue 05). On the real
    install, harvest printed only mirror and journald lines, none of the 13 sysctl diffs.
12. Tested the documented uninstall. `sudo pacman -R` removed the package cleanly when nothing
    had been applied (only `/etc/genarch-settings/profile` existed). The package's live file
    list contains no deployed mirror/unit/manifest path, so a real converged host would keep
    them (issue 07).
13. Tested dormant ownership. A first apply with `vm-generic` selected records the deselected
    `zenbook-s16` profile's mirror `/etc/udev/rules.d/98-bt-autosuspend.rules` in the manifest
    even though it deployed nothing there (no `OWNED` line in the plan). I then created a file
    at that path, deleted the unused `zenbook-s16` profile directory, and ran `apply`: it
    printed `PRUNE ... (no longer shipped)`, deleted the file, and left the bytes in a `.bak`.
    The summary said `planned=1 applied=0`. This became issue 08.
14. Wrote the eight issue files. Everything is read-only on the real host except the install/
    removal cycle and the engine's own `/var/lib` lock during one `sudo converge ceil` test
    (removed afterwards); all writes for apply/ceil/boot tests went to `~/machine/root*`.

## What I could not do

- **No matching hardware.** No ASUS Zenbook S 16, no AMD SMU, no wireless interface, no real
  cpufreq. `ryzenadj` cannot see an SMU; `iw` finds no interface; the real `/sys` has no
  policies. So I could not verify a matching-DMI install end to end, a real SMU write, a real
  ceiling write, or the `WIFI` row beyond SKIPPED.
- **I did not apply for real.** The card says not to use the real `/etc` as scratch, so the
  uninstall residue and the `TIMER` row after enabling units are inferred from the staging
  apply and the package file list, not measured on the real host.
- **I could not reach `report` exit 0.** In the rehearsal the units are not installed/enabled
  (the staging root's unit files are not visible to the host's systemd), so `TIMER` stays DRIFT
  after apply + ceil: report exit 1. Enabling the real units would require writing the real
  `/etc`, which the rules forbid. The report's drift=0 path is therefore untested.
- **No source, no internet research.** The installed documentation was the only description of
  intent. `install.sh` from source was not used: it requires `cargo` and a GitHub download, and
  the PKGBUILD it calls the supported path is not shipped.
- **No Ansible run.** I reasoned about exit codes as Ansible would consume them; I did not
  install/run Ansible (not necessary; `pacman`'s own exit status was the point).
- **`import` was not exercised.** It is migration-only and no legacy `state/` tree was provided.
  `completions` was read from `--help` only; I did not diff the three generated scripts.

## Observations I did not file (to avoid padding)

- The report table prints `OK CO` and `OK POWER` on a box where no apply ever ran and the SMU
  row is `SKIPPED`; the `reach` distinction (`intent` = "declared", not "measured", per CONTEXT)
  appears only in `--json` and in `apply`'s `APPLY-SCOPE` line. A first-time operator cannot
  tell a declared green from a measured one. Low; the JSON carries the information and the row
  detail is honest ("intent declares ...").
- The shipped `zenbook-s16` profile has no `status`/`source` while all seven other shipped
  profiles declare `status = "documented"` and a `source` list. CONTEXT says a profile states
  how it came to be. Low; a data/doc inconsistency, not a workflow blocker.
- A second managed write on the same day does not overwrite the existing `.bak-YYYYMMDD`; the
  original pre-engine copy is preserved. That is the safer behaviour, but "every write is
  preceded by a pre-image" is literally untrue for the second write.
- A partial apply prints only the failing line: no plan, no list of what landed, no SUMMARY.
  `report`/`apply --dry-run` afterwards show the remainder, so it is recoverable but not a
  record of the failed run.
- A pass that cannot create `/var/lib/genarch-settings` prints
  `Boot pass skipped: mkdir /var/lib/genarch-settings: Permission denied` and exits 0 (same
  silent-success class as issue 04, different path). On a host with a read-only `/var` every
  scheduled pass would be skipped forever with a clean status.
- `report` and `apply` classify the same unclassifiable intent differently (`1` vs `3`) — that
  is issue 03, not a separate one.
- The manifest's `PRUNE` count includes the deselected `zenbook-s16` profile's dormant mirror,
  so `PRUNE` says 41 owned while `MIRROR` says 39 deployed targets; that is issue 08, not a
  separate count bug.
- There is no operator-facing README/quickstart; the installed docs are developer-facing
  (AGENTS/DESIGN/CONTRIBUTING). The install.sh usage block is the closest thing to a quickstart.
- The rehearsal's `TIMER` DRIFT is expected (units are not enabled in the staging tree), not a
  package defect; the staging README does not mention it.

## Files

- `issues/01-package-install-exits-0-and-deploys-nothing.md`
- `issues/02-unclassifiable-managed-record-aborts-all-of-apply.md`
- `issues/03-unclassifiable-intent-exits-3-not-2.md`
- `issues/04-boot-resume-exit-0-when-smu-writes-fail.md`
- `issues/05-harvest-omits-sysctl-and-ceil-drift.md`
- `issues/06-shipped-docs-point-at-absent-profiles-contract-and-ledger.md`
- `issues/07-uninstall-leaves-deployed-files-units-and-manifest.md`
- `issues/08-dormant-profile-path-prunes-a-file-engine-never-wrote.md`
