# Package install exits 0 and deploys nothing

**Severity:** high
**Category:** bug

**Environment:** Arch guest (QEMU, kernel 7.2.11-arch1-1). `genarch-settings 0.4.0-1`
installed from `/home/arch/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst`. DMI is
`Standard PC (i440FX + PIIX, 1996)` / `QEMU`. `power-profiles-daemon` and `asusd` are not
installed.

**Intent:** Install the engine from the package files in `~/payload` the way its own material
says, then walk away. The package is supposed to leave the host converged.

**Steps:**

```sh
sudo pacman -U --noconfirm /home/arch/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
pacman -Q genarch-settings
pacman -Ql genarch-settings | grep '^genarch-settings /etc/'
ls -la /var/lib/genarch-settings
systemctl list-unit-files | grep -i genarch
ls -d /etc/tlp.d /usr/lib/systemd/system-sleep/genarch-settings
```

I ran this twice: first as a reinstall over the pre-installed package, then after a full
`sudo pacman -R --noconfirm genarch-settings`, to exercise both `post_install` and
`post_upgrade`. Both runs produced the same hook output and `pacman` exit 0.

**Expected:** The package's install hook says, verbatim:

> Deploy the intent's mirrors and units, then enable what the package wires up.

and `install.sh` says:

> The engine is idempotent: installing over an existing one re-plans and converges, it does not
> reset anything.

A successful `pacman -U` should therefore leave the mirrors, the sysctl projection, the unit
files and the enabled units on disk.

**Actual:** The hook's `apply` refuses and the hook's `systemctl enable` fails. Both calls are
suffixed with `|| true`, so `pacman` exits 0 and the package is recorded as installed. Nothing
is deployed: no manifest, no mirror files, no unit files. The only package-owned live file is
`/etc/genarch-settings/profile`.

**Evidence:**

Install hook (`/var/lib/pacman/local/genarch-settings-0.4.0-1/install`):

```
post_install() {
  ...
  /usr/bin/genarch-settings apply || true
  systemctl daemon-reload
  udevadm control --reload || true
  enabled=$(units)
  if [ -n "$enabled" ]; then
    systemctl enable $enabled || true
  fi
}
```

Install transaction (after a full package removal and reinstall):

```
:: Processing package changes...
installing genarch-settings...
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
Failed to enable unit: Unit genarch-settings.service does not exist
:: Running post-transaction hooks...
(1/1) Arming ConditionNeedsUpdate...
EXIT=0
```

State afterwards:

```
== package ==
genarch-settings 0.4.0-1
== live /etc files owned ==
genarch-settings /etc/
genarch-settings /etc/genarch-settings/
genarch-settings /etc/genarch-settings/profile
== manifest ==
ls: cannot access '/var/lib/genarch-settings': No such file or directory
== units ==
grep_exit=1
== deployed mirror dirs ==
ls: cannot access '/etc/tlp.d': No such file or directory
ls: cannot access '/usr/lib/systemd/system-sleep/genarch-settings': No such file or directory
```

The package-owned live paths are the whole story: `pacman -Ql genarch-settings | grep
'^genarch-settings /var/lib/'` prints nothing, and under `/etc` the package owns only the
profile file.

**Why it matters:** the only success signal an unattended installer (Ansible, a provisioning
script) sees is `pacman`'s exit status. It is 0. The machine is not tuned, no unit exists to
retry later, and the next boot runs nothing. The error text is on the install transcript and
nowhere else. This is the "reports success having done nothing" case.

**What I could not determine:** whether a host whose DMI matches the shipped `zenbook-s16`
profile converges end to end (no such hardware here). I did verify the apply path itself works
when every managed target exists: a staging copy with a fabricated `/etc/asusd/asusd.ron` and
the PPD unit present applied 62 changes and exited 1 for the profile mismatch, as documented.
