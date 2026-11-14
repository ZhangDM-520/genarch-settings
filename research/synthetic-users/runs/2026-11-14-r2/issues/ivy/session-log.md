# Session log — round 2, ivy

Box: Arch Linux 7.2.11, QEMU/KVM, 2 vCPU, headless, `sudo` without password.
Package under test: `genarch-settings 0.4.0-1` (plus `ryzenadj 0.19.0-1`) from `~/payload`.
No source was read; the package's own material is the only description used. No internet research.

1. Read `card.md`, `sysinfo.txt`, `~/machine/README`, `~/payload/MANIFEST.txt`.
2. Listed the installed package's files (`pacman -Ql genarch-settings`). Confirmed base intent,
   eight profiles, mirrors, managed records, docs and three completions ship.
3. Read the installed README, PROFILES.md, install.sh and the package `.INSTALL` scriptlet
   (`bsdtar -xOf ... .INSTALL`) to learn the install/remove contract.
4. Ran `genarch-settings report` on the real box: PROFILE drift (`zenbook-s16` vs QEMU DMI),
   MIRROR 41, MANAGED 6 unclassifiable, SYNC 14, CEIL/SMU/WIFI SKIPPED, TIMER drift,
   `SUMMARY ok=3 ... exit=1`.
5. Exercised the staging tree, never the real `/etc`, for every write:
   - copied `~/machine/root` to `~/machine/root.pristine` before any test;
   - `report` on staging: `vm-generic` matches, `SUMMARY ok=4 drift=5 exit=1`;
   - `apply --dry-run`: 60 lines, `PLAN 56 change(s)`, exit 0, `DRY-RUN nothing written`;
   - `apply`: `SUMMARY planned=56 applied=56 failed=0 exit=0`, manifest with 40 owned + 2 touched;
   - re-ran `apply`: `PLAN 0 change(s)`, exit 0 (idempotent);
   - `report` after apply: only CEIL and TIMER drift (both outside apply scope), exit 1.
6. Item 1 (install): `sudo pacman -R --noconfirm genarch-settings`, then
   `sudo pacman -U --noconfirm ~/payload/genarch-settings-...pkg.tar.zst`. Exit 0; package files
   deployed; post-install `apply` refused with exit 3 (`asusd.ron`, `ppd-conflict`) and
   `systemctl enable` failed. No manifest. Verdict: different; residual filed as issue 02.
7. Item 2 (one unclassifiable record): staging copy `root.item2` selecting `zenbook-s16` on the
   QEMU box; `apply` and `apply --dry-run` both exit 3, `nothing was written`. Verdict: still.
8. Item 2b: staging copy `root.ppd` with the matching `vm-generic` profile and only
   `/usr/lib/systemd/system/power-profiles-daemon.service` removed. `apply` exits 3 with the same
   refusal; the real box's package lacks PPD and does not depend on it. Filed as issue 01.
9. Item 3 (unusable intent): staging copy `intent-broken` with a TOML syntax error;
   `apply --dry-run` and `report` exit 2. The unclassifiable case still exits 3. Verdict:
   different. The dry-run contradiction is issue 03.
10. Item 4 (SMU failures): `converge boot` and `converge resume` against the staging root both
    exit 3 and log `all 2 ryzenadj call(s) failed` (the rehearsal stubs every ryzenadj call as
    failed). Verdict: gone.
11. Item 5 (harvest): `harvest` against the staging root printed every sysctl diff the SYNC row
    calls DRIFT plus mirrors, managed records and ceil policies. Verdict: gone.
12. Item 6 (docs): read the installed docs and checked every referenced path. `PROFILES.md` is
    installed flat and the ledger's absence is now explained; `docs/CUTOVER.md` is not installed,
    the README links still say `docs/PROFILES.md`, `install.sh` cites an absent PKGBUILD. Verdict:
    different; filed as issue 04.
13. Item 7 (uninstall): removed the package again
    (`sudo pacman -R --noconfirm genarch-settings`), captured `pre_remove` output, confirmed the
    payload files are gone and no deployed files exist (no apply ever succeeded on this box), then
    reinstalled from the payload. The README now has a Removal section; `install.sh --help` still
    omits deployed files. Verdict: different.
14. Item 8 (dormant/never-owned mirrors), three staging copies:
    - `root.base`, no selection: apply owned 40 paths and did not own the zenbook-only
      `98-bt-autosuspend.rules` (`grep -c` = 0);
    - `root.never`, an unselected profile mirror target pre-created by the operator: never listed
      in the manifest, still untouched after the profile directory was deleted;
    - `root.item8`, a selected mirror profile: deselecting kept the file and its ownership
      (`PLAN 0 change(s)`); deleting the profile then pruned only the file the engine had written,
      with a `.bak-20261114` pre-image.
    Verdict: gone.
15. Extra checks while retesting: `report --json` is schema 2 with 11 checks and a `profile`
    object; `apply` on a mismatched documented profile (`framework-13-amd`) wrote 56 changes and
    exited 1 as documented; `units` prints the two unit names; `completions bash` prints a script;
    a classifiable dry run printed the plan and exited 0.

## What I could not do

- No real SMU on this box (QEMU, no AMD SMU) and no wireless interface, so SMU/WIFI rows could
  only be exercised through the staging rehearsal; I did not run `converge` or `apply` against the
  real machine because that would write live kernel attributes and `/etc`.
- `install.sh` could not be exercised: only the installed copy exists (no source tree), and
  running it would download a release; that is outside this review.
- No real cpufreq on the host, so CEIL was judged against the staging tree's two fake policies.
- The "restore" half of uninstall could not be exercised end to end because no apply ever
  succeeded on the real box; the judgement for item 7 is the shipped procedure plus the observed
  removal output.
