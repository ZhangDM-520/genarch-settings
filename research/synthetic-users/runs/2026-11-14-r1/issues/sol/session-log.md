# Session log — sol, round 1

Guest: Arch Linux, 2 vCPU, 2 GiB, kernel 7.2.11-arch1-1, headless, passwordless sudo.
Package: `genarch-settings 0.4.0-1` (installed from `~/payload`), `ryzenadj 0.19.0-1`.
Installed in the guest: `strace`, `jq` (`pacman -S --noconfirm --needed strace jq`) — both used
throughout for syscall and JSON evidence.

I read the whole shipped documentation set (`/usr/share/doc/genarch-settings/{CONTEXT,DESIGN,
AGENTS,CONTRIBUTING,SCOPE}.md`, `install.sh`, `--help`, the intent and all profiles). The binary
was not disassembled beyond `strings` to identify the live paths it uses (`/proc/acpi/wakeup`,
`/sys/.../cpuidle/current_governor`, the lock path). No source was read; by the brief there is
none.

Writes were confined to copies of `~/machine/root` (`~/r2`…`~/r44`, `~/ev1`…`~/ev6`). The real
`/etc` was only read, except for the package manager install/remove/reinstall cycle required by
the brief. The real `/proc` and `/sys` were never written.

## Chronology

1. Read the card, `~/machine/README`, `~/payload/MANIFEST.txt`, `pacman -Ql`, the package's
   `.INSTALL` and `.PKGINFO`, and every shipped document.
2. Naive pass on the real root: `report`, `report --json`, `apply --dry-run`. The stock box is
   a PROFILE mismatch (QEMU DMI, package selects `zenbook-s16`); `apply --dry-run` refuses with
   exit 3 because `asusd.ron` and the ppd unit are absent. `/var/log/pacman.log` shows the same
   refusal during the original package install, then `systemctl enable` failing. Nothing was
   deployed to `/etc`.
3. Naive pass on the rehearsal root as the README describes: `report` (exit 1), `apply
   --dry-run` (56 changes, byte-for-byte no tree change), `apply` (56 applied), second `apply`
   (0). Verified with a full `find -printf` + `sha256sum` snapshot before/after the dry run.
4. `strace` on `report` as user and as root: no write/open-for-write/unlink/rename/mkdir except
   `/dev/tty`. `report` is read-only. `harvest` and `units` produced no writes.
5. Inspected the manifest, pre-images, deployed modes/owners; tested mode preservation (600),
   ownership preservation (`nobody:nobody` target after a root apply), a new target inheriting
   the directory owner, and a symlinked target (replaced; the link target untouched).
6. Hostile inputs: unknown key, truncated TOML (mid-string), schema 2, unquoted dotted key,
   en-dash CPU list, selection naming an absent profile, empty profile directory, malformed
   selection, unreadable intent, backup-looking mirror name, stale `[omit]`, mirror/managed
   overlap, base tombstone naming an operator file. All refused with exit 2 except the empty
   profile (legal) — messages are specific and no panic occurred.
7. Concurrency and locking: an externally held `flock` makes `apply` exit 2 and both scheduled
   passes log "pass skipped" and exit 0. Two simultaneous applies: the second exits 2 with
   `apply lock held by pid=…`; the first completes.
8. Crash behaviour: `SIGKILL` during the rename (delay injected with `strace`) leaves
   `.99-wifi-powersave.conf.6445.tmp` in `/etc/NetworkManager/conf.d/` and no manifest. A
   partial apply (one unwritable directory) leaves a landed mirror with no manifest. A later
   apply heals the ownership; the temp is never cleaned.
9. Passes: `converge boot/resume/ceil` on a root with the Zenbook profile and fake DMI/wake/
   cpuidle files. `settle_secs=5` measured (5030 ms boot, 13 ms resume). Wake devices already
   `*disabled` are not rewritten. A failing `ryzenadj` shim makes all 12 SMU calls fail and the
   pass exits 0; an unwritable sysctl in the same pass exits 3. `converge ceil` only touches
   `scaling_max_freq`.
10. Lifecycle: reinstall and `pacman -R` + reinstall. Reinstall exits 0 while deploying nothing;
    removal drops `/etc/genarch-settings` and the binary; reinstall restores
    `profile = "zenbook-s16"` and the same silent refusal.
11. Wrote the eight issues, ordered by blast radius.

## What I could not do or measure

- **No internet.** `install.sh --dry-run` dies at `curl` (TLS EOF, exit 35) because there is no
  source tree and it tries to fetch the release tarball. The installer could not be exercised.
- **No real hardware.** No AMD SMU, no wireless interface, no `power_supply` device. SMU/WIFI
  rows are SKIPPED; `converge` SMU writes were observed only through `ryzenadj`'s failure and a
  shim. The `/proc/acpi/wakeup` toggle behaviour of a real kernel was not exercised (only the
  rehearsal copy).
- **No clean report.** The `TIMER` row checks the host's systemd, not the rehearsal root (adding
  `.wants` symlinks under `GENARCH_SETTINGS_ROOT` leaves it DRIFT), and enabling the units would
  require writing the real `/etc`, so a green `report`/exit 0 was not reachable. `apply` exit 0
  on a converged root was verified.
- **`import`** was not exercised: there is no legacy `system-tuning/state` tree on this box.
- **Package removal with deployed files** could not be tested without deploying to the real
  `/etc`; `pre_remove` only disables units (static reading).
- **Real-machine concurrency/timings** are not meaningful on 2 vCPU; I measured syscall and
  file effects, not speed.

## Observations not filed as issues

- A rehearsal `apply` executes the **host's** `systemctl daemon-reload` and `udevadm control
  --reload` (`strace -f -e trace=execve` shows both), even though every live path is redirected.
  Unprivileged, both fail silently and `apply` still prints `RELOAD` and exits 0. The README
  presents `GENARCH_SETTINGS_ROOT` as total isolation; it is not.
- `[omit]` has two syntaxes with no documentation: `managed = ["journald"]` (record name) and
  `mirror = ["/etc/tlp.d/01-eco.conf"]` (absolute live path). The natural guesses
  (`etc/tlp.d/…`, `mirror/etc/…`) all fail with "which no layer below it ships" even though the
  base does ship it.
- An unreadable intent tree is reported as `no intent at … — is the package installed?`
  (EACCES presented as absence).
- A truncated manifest makes `report` print `check 'MIRROR' failed to run` (three rows) and
  `apply` exit 2 with the parse error; there is no self-healing path.
- `SUMMARY …` has no trailing newline (`exit=1EXIT=1` when concatenated).
- A prune prints `SUMMARY planned=1 applied=0` although the file was removed.
- `report` as root still says the SMU table is absent because it "needs root, or ryzenadj is
  missing" when run as root.
