# Session log — sol, round 2

Box: Arch, 2 vCPU QEMU, passwordless sudo, `genarch-settings 0.4.0-1` installed from
`~/payload` (sha matches MANIFEST.txt on unpack). All writes were made in copies of
`~/machine/root`; `~/root.pristine` was taken first and is unmodified.

1. Read `~/machine/README`, `~/payload/MANIFEST.txt`, the package file list
   (`pacman -Ql`), the installed `README.md`, and the package `.INSTALL` script.
2. Snapshotted the real `/etc`, `/var/lib/genarch-settings`, `/usr/local/lib` and
   `system-sleep` (1133 paths) before touching the package.
3. Read-only rehearsal: `GENARCH_SETTINGS_ROOT=~/machine/root genarch-settings report`
   with a before/after `find -printf` manifest of the tree: no file changed (`NOCHANGE`).
   `apply --dry-run` on the same tree wrote nothing.
4. Retest 1 — reinstalled the package from `~/payload`; captured pacman's exit code,
   the post_install refusal, and the absent manifest. Verdict: different.
5. Retest 2 — read `/etc/genarch-settings/profile` and the PROFILE row on this QEMU box.
   Verdict: still.
6. Retest 3 — fresh copy `~/t3` with `profile = "zenbook-s16"`, apply refused on the five
   absent `asusd.ron` anchors, exit 3, no manifest. Verdict: still.
7. Retest 4 — successful apply in `~/t4`; parsed the manifest with python and checked every
   owned path exists; checked `touched`. Verdict: gone.
8. Retest 5 — SIGKILL at 0.02–0.15 s over five fresh copies; found leftover
   `.<name>.<pid>.tmp` files, no manifest, then a recovery apply and a second kill test.
   Verdict: still.
9. Retest 6 — `harvest` on the drifted tree prints sysctl and ceil proposals, exit 0.
   Verdict: gone.
10. Retest 7 — `sudo ryzenadj --set-coall` fails (no SMU); `sudo genarch-settings converge boot`
    now exits 3 after 12 failed calls. Verdict: gone.
11. Retest 8 — same-day pre-image behaviour: drift a deployed file, apply, drift again, apply;
    md5 of the pre-image unchanged. Verdict: gone.
12. Hostile pass, all in fresh copies:
    - selection names an absent profile, selection empty, selection half-written, selection
      with an unknown key → exit 2 with a precise message each;
    - profile `intent.toml` with an unknown key → exit 2; a truncated (comment-only)
      `intent.toml` loads as "declares no machine requirement" (valid TOML; not filed);
    - an empty profile *directory* selected → PROFILE OK and base intent applies (not filed:
      the row says the profile declares nothing);
    - operator `sysctl.toml` with a bare dotted key → exit 2 with the pointing error;
    - unreadable intent file → exit 2; non-writable target directory → exit 3 naming the temp;
    - unprivileged apply on a root-owned copy → exit 2 "open …/lock: Permission denied";
    - two applies at once → one success, one exit 2; the refusal's holder field is empty
      (filed);
    - `pacman -R` then reinstall: removal leaves `/var/lib/genarch-settings/lock`, removes
      `/etc/genarch-settings` including the selection; reinstall recreates it and refuses the
      apply again.
13. Found the partial-apply accounting bug (34 files written, `applied=0`) and the absent
    `CUTOVER.md` while checking the recovery path; verified both by re-running.

Not filed / could not determine:
- Whether the empty-profile-directory fallback is intended; it is consistent with "no
  profile means base only" but indistinguishable from a mis-built profile.
- Whether the missing `docs/CUTOVER.md` was left out of the package deliberately.
- Real SMU, wireless and cpufreq behaviour: this VM has none (`ryzenadj --info` fails,
  `iw` absent), so SMU/WIFI rows are limitations, not findings.
- Owner preservation on deployment needs root and a user-owned target; not measured.
