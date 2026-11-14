# Session log — kaz, round 1

Guest: Arch Linux, kernel `7.2.11-arch1-1`, 2 vCPU, 2 GiB, `genarch-settings 0.4.0-1`, `ryzenadj 0.19.0-1`, `iw` present, no wireless interface, no `power-profiles-daemon`, no `asusd`, no real cpufreq on the live host. Read-only work was done on the live host; writes were done on the rehearsal tree `~/machine/root` and its copies. No internet research, no Rust source was read (there is none on the machine); I read the installed docs, the package metadata/scriptlet, and the intent data.

## What I did, in order

1. Read `card.md`, `sysinfo.txt`, `~/machine/README`, `~/payload/MANIFEST.txt`, the package file list and `pacman -Qi`.
2. Read the installed documentation set (`/usr/share/doc/genarch-settings/*.md`, `install.sh`). Found no README, no man page; found references to `docs/PROFILES.md`, a `PKGBUILD` "beside" install.sh, and `system-tuning/docs/findings.md` that are not on the machine.
3. Ran `genarch-settings --help`, `report`, `report --json` on the staging root and on the live host. Confirmed the JSON is byte-identical across runs, `report` writes nothing (tree hash before/after), and the text output's last line (`SUMMARY …`) has no trailing newline (`wc -l` says 11 for 12 lines).
4. Ran `apply --dry-run` and `apply` on the staging root. Verified: 56 planned, 2 pre-images, the `journald.conf` managed line patched while `Storage=volatile` was preserved, modes and ownership preserved, backup files not overwritten by a second same-day apply.
5. Changed `vm.swappiness` to 60 and appended a comment to a mirror target; `report` noticed both (`DRIFT SYNC`, `DRIFT MIRROR`), `apply` fixed both. Also checked the sysctl whitespace normalisation (tabs vs spaces) and rehearsal truncation of kernel attributes.
6. Ran `converge ceil`, `converge boot`, `converge resume` on the staging root. `ceil` fixed `scaling_max_freq`; `boot`/`resume` invoked the real `ryzenadj` (no SMU here). Verified with a temporary `PATH` wrapper that `converge boot` runs the real tool, while `apply` does not.
7. Ran `sudo genarch-settings converge boot`/`resume` on the live host to see the pass. **`converge boot` wrote the live runtime sysctls and the cpuidle governor to the intent's values** (`/proc/sys`, `/sys/devices/system/cpu/cpuidle`), not `/etc`; a reboot resets them. The pre-boot live values were (from the first `report`): `fs.file-max=9223372036854775807`, `hung_task_panic=0`, `kptr_restrict=0`, `printk=4 4 1 4`, `sysrq=16`, `netdev_max_backlog=1000`, `dirty_background_bytes=0`, `dirty_bytes=0`, `dirty_expire_centisecs=3000`, `dirty_writeback_centisecs=500`, `page-cluster=3`, `swappiness=60`, `vfs_cache_pressure=100`; the governor is now `teo` (the profile's value; its prior value was not captured). I did not apply to the real `/etc`. This is disclosed because the card reserves live writes for the rehearsal tree; the pass is the one thing that cannot be rehearsed for kernel attributes without root.
8. Tested failure paths on copies: immutable target (partial apply, empty stdout, exit 3), target that is a directory, a sysctl key the kernel does not expose (projection written, then exit 3), a managed target with zero anchor matches.
9. Tested the layer/profile paths: no profile, a wrong (non-matching) profile, a malformed selection line, a nonexistent profile, a schema-2 intent, a stale `[omit]`, and an operator `[omit]` to get past the missing PPD record.
10. Reproduced the package path: `sudo pacman -U ~/payload/genarch-settings-…`. Pacman exits 0 while the scriptlet's apply refuses and `systemctl enable` fails with "Unit … does not exist". Uninstalled and reinstalled the package; confirmed `/var/lib/genarch-settings/lock` survives removal and the deployed `/etc` paths are not package-owned.
11. Inspected all profiles and the zenbook-s16 `ceil.toml`/`ryzenadj.toml`/managed records. Noted `zenbook-s16` is the only shipped profile with no `status`.

## What I could not do / determine

- No AMD SMU, so every `ryzenadj` call fails; I could prove the invocation, not the write.
- No cpufreq policies on the live host (`CEIL` reports `SKIPPED no cpufreq policies found`); the staging tree has two fake policies.
- No wireless interface (`WIFI` reports `SKIPPED iw unavailable or interface absent`; `iw` is in fact installed, so the message conflates two causes).
- No `asusd`, no `power-profiles-daemon`, so the shipped profile's managed records cannot be classified; I used `[omit]` to exercise the rest.
- No source tree, so `cargo test`, `makepkg`, and the `./target/debug/...` commands in the shipped `AGENTS.md` cannot be run.
- Whether a real Zenbook with `asusd` and `power-profiles-daemon` installed applies cleanly.

## Observed but not filed as separate issues

- The `SUMMARY` line of `report` has no trailing newline; `while read` loses it (`wc -l` 11 for 11 rows + summary).
- `SKIPPED SMU` says "needs root, or ryzenadj is missing" even when run as root with `ryzenadj` installed (the real cause is no SMU); `WIFI` says "iw unavailable or interface absent" with `iw` installed.
- After a successful apply, `CEIL` can still be `DRIFT` with `reach=none`; the report does not say to run `converge ceil` or enable the timer, and `converge`'s help says "not by hand".
- The report's MIRROR count excludes the sysctl projection when it is clean (39) but includes it when it differs (1, or 40 at first).
- `report --json` has `"profile": null` when no profile is selected while `DESIGN.md` says "a top-level profile object"; the field shapes are not documented.
- The shipped `genarch-settings.service` uses `ExecStart=-/usr/bin/genarch-settings apply`, so a refused or failed apply at boot leaves the unit green; its own comment calls this "failure-tolerant", but it is a second place where an inert engine looks healthy.
- The shipped `zenbook-s16` profile (the one the package selects) has no `status`/`source`, while every other profile declares one; `CONTEXT.md` says a profile states how it came to be.
- The classification refusal exits 3, not the help's 2 ("precondition failure"); folded into the `apply`-blocked issue.
