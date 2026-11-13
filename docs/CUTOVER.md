# CUTOVER — flipping this host from the shell `eco` stack to `genarch`

> **Historical record (2026-11-10).** Written when the binary was named `genarch`. The current
> name, path and units are `genarch-settings` (renamed in 0.4.0); the commands below are kept
> verbatim so the record stays true to what was run. For the current procedure see `README.md`
> and `install.sh`.

Nothing in this file is optional, and the order is the safety property. Two engines must never
converge the same host, and the shell engine re-creates its own files from
`system-tuning/state/` on every boot — so "install the package" alone leaves a double writer.

## 0. Pre-flight (read-only)

```sh
cd ~/Projects/system-tuning && ./eco report && ./eco apply --dry-run   # shell engine green, PLAN 0
cd ~/Projects/genarch-settings && cargo test                          # engine gate green
GENARCH_INTENT=$PWD/intent ./target/release/genarch report           # only MIRROR/TIMER/PRUNE red
GENARCH_INTENT=$PWD/intent ./target/release/genarch apply --dry-run  # the cutover plan
```

Review the plan line by line. It must list **only**:

- the genarch units, the sleep hook and the udev rule (deploy),
- `/etc/sysctl.d/90-genarch.conf` (the projected sysctl table),
- the 13 shell paths in `intent.toml`'s `prune` list (remove).

Anything else means the intent does not describe live — stop and reconcile with `genarch harvest`.

## 1. Pre-images

`genarch apply` writes a dated `.bak-YYYYMMDD` next to every file it replaces and every file it
removes. It cannot pre-image a file it did not exist for, so take the two anchors it does not own:

```sh
sudo cp -p /etc/systemd/system/multi-user.target.wants/eco-power.service \
           /root/eco-power.service.wants.bak-$(date +%Y%m%d) 2>/dev/null || true
systemctl list-unit-files 'eco-*' > /root/eco-units.before.txt
```

## 2. Freeze the shell engine first

```sh
sudo systemctl disable --now eco-power.service eco-ceil.timer
sudo systemctl disable --now eco-ceil.service 2>/dev/null || true
```

Disabling **before** the new engine writes is what prevents a shell converge between the flip and
the package install. It also means: if anything below fails, the host is running on live values
with no engine at all — which is why step 3 runs immediately.

## 3. Install and apply

```sh
cd ~/Projects/genarch-settings
makepkg -f
sudo pacman -U --noconfirm genarch-0.1.0-1-x86_64.pkg.tar.zst
```

`post_install` runs `genarch apply` (deploys the mirrors, the projection and the prunes), then
`daemon-reload`, `udevadm control --reload`, and enables `genarch.service` + `genarch-ceil.timer`.

## 4. Verify before rebooting

```sh
sudo genarch report                       # expect: only SMU may be SKIPPED; TIMER must be OK
sudo genarch apply --dry-run              # expect: PLAN 0
systemctl is-enabled genarch.service genarch-ceil.timer
ls /usr/local/sbin/eco-power.sh ~/.reserve/hw-tune.fish 2>&1   # both gone
systemctl cat genarch.service | grep Exec
```

Rollback point: every removed path has a dated pre-image beside it (`<path>.bak-<day>`), and
`system-tuning` is tagged at the last shell commit.

## 5. Retire the shell engine in `system-tuning` (same session)

Leaving `state/` in place is not harmless: `eco-power.service`'s `ExecStart` points at
`$ECO_HOME/eco apply`, and any `eco apply` (a human, or a stale timer) restores every eco-* file
and re-arms the double writer. The retirement is therefore a **marker plus a guard**, not a
deletion: the shell tree stays as the rollback artifact and parity reference, and `eco` refuses a
real apply while the marker exists.

```sh
cd ~/Projects/system-tuning
git tag genarch-cutover-shell-retired            # points at the last pre-retirement commit
# state/RETIRED.md is the marker (landed 2026-11-10 with `eco`'s `refuse_retired_apply`, which
# reads it through ECO_STATE); it must be committed for the guard to survive a fresh checkout.
./tests/run                                      # gate case30 pins the refusal; all green
```

## 6. Reboot and a suspend/resume cycle

```sh
systemctl reboot
# after boot:
genarch report                     # drift=0; SMU judged as root
journalctl -b -u genarch.service --no-pager | tail -20
systemctl list-timers genarch-ceil.timer
# then one suspend/resume:
journalctl -b -g 'genarch' --no-pager | tail -20   # resume pass must appear exactly once
```

The resume pass is the one that historically wedged the thaw. Verify the "System returned from
sleep" → "Successfully thawed" delta stays short (single pass, no sleep).

## 7. Rollback

```sh
cd ~/Projects/system-tuning && git checkout genarch-cutover-shell-retired -- eco lib state tests
rm -f state/RETIRED.md                     # the marker must go, or `eco apply` refuses (exit 2)
sudo systemctl disable --now genarch.service genarch-ceil.timer
sudo systemctl enable eco-power.service eco-ceil.timer
sudo rm -f /usr/lib/systemd/system-sleep/genarch
sudo ./eco apply
```

`genarch` never deletes a file it does not own, so every file it wrote is still on disk and the
shell engine converges it back. That is the whole reason the rollback is cheap.

## Known gaps at the cutover (accepted)

- `audio-pm-ctl` and `iw-set-regdomain` stay as mirrored helpers invoked by udev/units; folding
  them into `genarch` subcommands is a follow-up, not a blocker.
- `niri-autostart` is a retired record (niri is gone); it stays in `managed/` until its target
  file is cleaned up, and `MANAGED` will report it as missing-target if the niri config is removed.
- `hw-tune.fish` is pruned; the engine's `ryzenadj.toml` carries the same knobs, so the
  compositor autostart line that spawns it must be removed too (it would fail harmlessly, but a
  stale spawn is a stale fact).
