# Session log — ren, round 2

Box: `r2-ren`, Arch VM (`QEMU`, product `Standard PC (i440FX + PIIX, 1996)`), `genarch-settings
0.4.0-1` from `~/payload`, staging tree `~/machine` with no profile in `root/`.
All work on remote copies (`~/work`, `~/t2`, `~/t4`…`~/t8`); the real root was only read, except
two temporary `systemctl enable` runs that were reverted (see item 12).

1. Read the card, `sysinfo.txt`, `machine`, `~/payload/MANIFEST.txt`, `~/machine/README`, the
   staging tree (`find ~/machine/root -type f`) and the installed docs (`README.md`,
   `PROFILES.md`, `CONTEXT.md`, `DESIGN.md`, `SCOPE.md`, `install.sh` grep). `PROFILES.md` was in
   the package this time.
2. Baseline report against the untouched staging tree:
   `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report` → `SKIPPED PROFILE no
   profile selected (base intent only)`, 40 mirror targets differ, exit 1.
3. Read the staging DMI (`QEMU` / `Standard PC (i440FX + PIIX, 1996)` / `QEMU Virtual Machine`),
   both cpufreq policies (2 vCPU, `cpuinfo_max_freq` 3 500 000, `scaling_max_freq` 4 000 000) and
   copied the root to `~/work/root`.
4. Wrote the port: a profile under the operator layer,
   `~/work/root/etc/genarch-settings/profiles/qemu-i440fx/{intent.toml,ceil.toml}` with
   `[requires] product/sys_vendor` and one band class at 3 500 000 kHz, and the one selection line
   `profile = "qemu-i440fx"` in `~/work/root/etc/genarch-settings/profile`. Report then says
   `OK PROFILE profile 'qemu-i440fx' matches product=Standard PC (i440FX + PIIX, 1996)
   sys_vendor=QEMU`; `CEIL` reports `4000000 want 3500000` on both policies.
5. `apply --dry-run` (56 changes, `APPLY-SCOPE … full=MIRROR,MANAGED,SYNC,PRUNE file-only=WIFI
   intent=CO,POWER none=PROFILE,CEIL,SMU,TIMER`, exits 0), then installed `strace`
   (`pacman -Sy --noconfirm --needed strace`) and ran real `apply` under it: exit 0, 54 applied,
   only `execve("/usr/bin/genarch-settings", …)` recorded.
6. Layer exercise: (a) removed one shipped file without forking it, with an operator
   `intent.toml` `[omit] mirror = ["/etc/tlp.d/01-eco.conf"]` (first guess used the relative
   path and was refused — issue 02); `MIRROR` drops 40 → 39. (b) overrode one sysctl value with
   `~/work/root/etc/genarch-settings/sysctl.toml` `[values] "vm.swappiness" = "60"`; the key
   leaves the drift list and the plan while the other 15 base keys remain. Both layers win, and
   nothing in the output says which layer won (retest item 7).
7. Rehearsal passes: `converge ceil` capped both policies in the copy (3 500 000) and then
   reported `OK CEIL all 2 policies capped`. `converge boot`/`resume` with a declared
   `ryzenadj.toml` refused to execute ryzenadj under the rehearsal root and exited 3 (issue 06).
   Writing `ryzenadj.toml` needed two attempts: `[values]` is refused, only `[limits]` works
   (issue 03, found from the shipped `zenbook-s16` profile).
8. Retest 4: a profile mirror at `/etc/sysctl.d/90-genarch-settings.conf` is refused at load,
   exit 2.
9. Retest 5: `zenbook-s16` selected on this box → `report PROFILE DRIFT`; `apply` refuses with
   exit 3 / "nothing was written"; a documented profile (`framework-13-amd`) on the same box
   writes and exits 1, so the documented exit-1 path exists but not for the shipped measured
   profile.
10. Retest 6: `harvest` prints `-`/`+` sysctl suggestions and `--- ceil <policy> (live …)` lines.
11. Retest 2, 7, 8 on fresh copies (`~/t2`, `~/t8`): a profile with no `[requires]` is `OK`;
    `--json` now carries the profile object but no per-value layer; a converged no-profile tree
    is green except `TIMER`, `SUMMARY … exit=1`.
12. Retest 8 / issue 07: enabled the package units on the real host temporarily
    (`systemctl enable genarch-settings.service genarch-settings-ceil.timer`), saw `OK TIMER`
    with the real root and `DRIFT TIMER not enabled` with `GENARCH_SETTINGS_ROOT=$HOME/t8` on the
    same machine, then disabled and removed both units and both `.wants` links; confirmed
    `systemctl is-enabled … → not-found`. Also removed a temporary
    `~/t8/usr/bin/systemctl` shim used to rule out a shell-out.

13. Checked the package install script (`.INSTALL` in the payload): `post_install` runs
    `genarch-settings apply` and then `systemctl enable $(genarch-settings units)`. The pacman log
    for this box shows that apply refused (`intent cannot be classified`) and the enable failed
    because the units were never deployed — the package is present but nothing is converged, and
    the ASUS profile is selected. Folded into issue 01.

## What I could not do

- Produce a fully green *no-profile* host: a rehearsal root always makes `TIMER` `DRIFT` (issue
  07), and converging the real root was out of bounds. Item 8 is therefore a partial
  reproduction.
- Determine how the real-root `TIMER` check reads enablement (no `systemctl` exec, no D-Bus).
- Exercise `WIFI`, `SMU` or a real curve offset: this host has no wireless interface and no SMU.
  Recorded as a limitation, not an issue.
- Run the profile gate (`cargo test --test profiles`): no `cargo`, no source on the box (issue
  04).

## Handoff: what I would give the next porter (third model)

1. Read `PROFILES.md`, then copy the *shape* of the nearest shipped profile from
   `/usr/lib/genarch-settings/profiles/<ref>/`. The docs name most of the contract; the reference
   profile is where the table names actually live.
2. Put the new profile under `/etc/genarch-settings/profiles/<slug>/` (the operator layer is
   read as a profile root; it survives package upgrades and keeps one-unit facts out of package
   data). `intent.toml` needs `[requires] product` and `sys_vendor` exactly as
   `/sys/class/dmi/id` prints them; add `status = "documented"` plus `source = [...]` only if
   every value is published.
3. `ceil.toml`: one band per documented clock (`khz`, no `cpus`) for a documented profile;
   explicit `cpus` only for a measured one.
4. `ryzenadj.toml`: `[limits]` (not `[values]`), `stapm`/`fast`/`slow` in mW; `power_saving` is a
   boolean under the same table.
5. Select with one line, `profile = "<slug>"`, in `/etc/genarch-settings/profile`.
6. Judge against a copy, never the machine:
   `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report`, then `apply --dry-run`.
   Expect `TIMER` to stay `DRIFT` in the copy and read only the rows you can affect.
7. Layer overrides: `/etc/genarch-settings/sysctl.toml` `[values]` merges per key;
   `/etc/genarch-settings/intent.toml` `[omit] mirror = ["/etc/<live path>"]` disables a shipped
   file. The report will not tell you which layer supplied a value.
8. Do not trust `PROFILE OK` alone: a profile with no `[requires]` is `OK` by declaration. Keep
   the DMI strings exact and re-read them from the target machine.
