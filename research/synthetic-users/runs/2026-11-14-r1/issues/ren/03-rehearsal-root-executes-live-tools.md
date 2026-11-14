# A rehearsal with `GENARCH_SETTINGS_ROOT` still executes the live `ryzenadj`, `iw`, `systemctl` and `udevadm`

- **severity:** high
- **category:** bug
- **environment:** Arch Linux guest (QEMU, 2 vCPU), `genarch-settings 0.4.0-1`, `ryzenadj 0.19.0-1`, `iw` and `systemd` present (pid 1 is systemd). `strace 7.3` installed with `pacman -S strace`. The rehearsal root is a copy of the staging tree.

## Intent

Rehearse a profile for a second machine without touching the machine, as the shipped documentation
describes, and confirm from the syscall trace that every effect lands in the copy.

## Steps

```sh
cp -a ~/machine/root ~/ev2
P=~/ev2/etc/genarch-settings/profiles/qemu-i440fx
printf '[limits]\nstapm = 31000\nfast = 31000\nslow = 31000\npower_saving = true\n' > $P/ryzenadj.toml
printf 'wifi_interface = "wlan0"\n' > $P/passes.toml
# make the plan non-empty for the interesting classes
printf '# drift\n' >> ~/ev2/etc/udev/rules.d/81-wifi-powersave.rules
printf '# drift\n' >> ~/ev2/etc/systemd/system/genarch-settings.service

GENARCH_SETTINGS_ROOT=~/ev2 strace -f -e trace=execve -o ~/ev2-boot.trace genarch-settings converge boot
grep -E 'execve\("/usr/bin/(ryzenadj|iw|genarch-settings)' ~/ev2-boot.trace | grep '= 0$'

GENARCH_SETTINGS_ROOT=~/ev2 strace -f -e trace=execve -o ~/ev2-apply.trace genarch-settings apply
grep -E 'execve\("/usr/bin/(systemctl|udevadm)' ~/ev2-apply.trace | grep '= 0$'
```

## Expected

`AGENTS.md`: "Rehearsal without touching the machine: `GENARCH_SETTINGS_ROOT=/path/to/copy` aims
every live path at a copy of the tree; `GENARCH_SETTINGS_INTENT=/path/to/intent` swaps the intent."

`DESIGN.md`: "`GENARCH_SETTINGS_ROOT` ... is the only way to rehearse a plan against a copy of the live tree, which the cutover depends on."

A rehearsal may write only under the copy.

## Actual

With `GENARCH_SETTINGS_ROOT` set, `converge boot` execs the live `/usr/bin/ryzenadj` (two calls in
this profile) and the live `/usr/bin/iw dev wlan0 set power_save on`; `apply` execs the live
`/usr/bin/systemctl daemon-reload` and `/usr/bin/udevadm control --reload`. The engine prints
`RELOAD systemctl daemon-reload` / `RELOAD udevadm control --reload` as if they succeeded.

On this VM the SMU and Wi-Fi are absent, so the calls fail harmlessly. On a real second laptop —
the exact machine a port is rehearsed for — a profile that declares `[co]`/`[limits]` and a
`wifi_interface` writes the real SMU and the real wireless interface while the operator believes
they are rehearsing into a copy. The reloads are system-wide actions with no `--root` argument, so
under the documented `sudo … apply` they act on the live manager.

## Evidence

```
COMMAND: GENARCH_SETTINGS_ROOT=~/ev2 strace -f -e trace=execve -o ~/ev2-boot.trace genarch-settings converge boot
exit=0

COMMAND: grep -E 'execve\("/usr/bin/(ryzenadj|iw|genarch-settings)' ~/ev2-boot.trace | grep '= 0$'
6195  execve("/usr/bin/genarch-settings", ["genarch-settings", "converge", "boot"], 0x7ffdfac220d8 /* 18 vars */) = 0
6196  execve("/usr/bin/ryzenadj", ["ryzenadj", "--stapm-limit=31000", "--fast-limit=31000", "--slow-limit=31000"], 0x7ffc32543218 /* 18 vars */) = 0
6197  execve("/usr/bin/ryzenadj", ["ryzenadj", "--power-saving"], 0x7ffc32543218 /* 18 vars */) = 0
6198  execve("/usr/bin/iw", ["iw", "dev", "wlan0", "set", "power_save", "on"], 0x7ffc32543218 /* 18 vars */) = 0

COMMAND: genarch-settings converge boot (stderr, same run)
genarch-settings: ryzenadj failed: ryzenadj --stapm-limit=31000 --fast-limit=31000 --slow-limit=31000 (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: ryzenadj failed: ryzenadj --power-saving (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand

COMMAND: GENARCH_SETTINGS_ROOT=~/ev2 strace -f -e trace=execve -o ~/ev2-apply.trace genarch-settings apply
exit=0

COMMAND: grep -E 'execve\("/usr/bin/(systemctl|udevadm)' ~/ev2-apply.trace | grep '= 0$'
6206  execve("/usr/bin/systemctl", ["systemctl", "daemon-reload"], 0x7ffd34b2d750 /* 18 vars */) = 0
6207  execve("/usr/bin/udevadm", ["udevadm", "control", "--reload"], 0x7ffd34b2d750 /* 18 vars */) = 0
```

## What could not be determined

- Whether the reload subprocesses succeeded when run unprivileged (they were started; the engine
  ignores their status and exits 0). Under `sudo`, which `apply` documents as required, they act
  on the live manager.
- Whether a machine with a working SMU would accept the values (no SMU here); the point is that
  the executable is the live one, not a copy.
