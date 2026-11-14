# The package a second machine installs preselects the ASUS profile, so PROFILE is red from first boot

- **Severity:** medium
- **Category:** bug
- **Environment:** Arch Linux VM (QEMU, `product_name = "Standard PC (i440FX + PIIX, 1996)"`), genarch-settings 0.4.0-1 installed from `~/payload`

## Intent

Port the engine to a second model. `README.md` promises "A second AMD laptop is a directory
under `intent/profiles/` and one line in `/etc/genarch-settings/profile`", and `SCOPE.md` says
the package "selects nothing on a host whose DMI does not match (`PROFILE` drift, exit 1)".

## Steps

```sh
genarch-settings report | head -1
cat /etc/genarch-settings/profile
```

## Expected

A package that "selects nothing on a host whose DMI does not match" should start with no
profile selected (`PROFILE SKIPPED`), so a porter writes the one selection line.

## Actual

The package's own selection file is `profile = "zenbook-s16"`, and the first report on a foreign
machine is a `PROFILE` DRIFT naming the ASUS laptop. `README.md` is explicit that the package
ships the selection ("The package ships the `zenbook-s16` profile **and** its selection"), so
`SCOPE.md` and `README.md` contradict each other.

The consequence is worse than a red row: the package's `post_install` runs `genarch-settings
apply`, which refuses because the selected profile carries five managed records whose target
`/etc/asusd/asusd.ron` does not exist. Nothing is deployed — no mirror, no unit, no manifest —
and `systemctl enable` then fails because the units are themselves deployed by the apply. This
box is in exactly that state after a clean `pacman -U`, and every upgrade repeats it.

## Evidence

```
$ genarch-settings report | head -1
DRIFT PROFILE profile 'zenbook-s16' is not for this machine: product is "Standard PC (i440FX + PIIX, 1996)", profile requires ASUS Zenbook S 16 UM5606WA_UM5606WA; sys_vendor is "QEMU", profile requires ASUSTeK COMPUTER INC. findings#intent
$ cat /etc/genarch-settings/profile
profile = "zenbook-s16"
$ sed -n '/Second machine/,/drift/p' /usr/share/doc/genarch-settings/SCOPE.md
- **Second machine** — a new directory under `profiles/` plus one line in `/etc/genarch-settings/profile`.
  If the base intent or the engine has to change for it, that is a bug in this scope, not a fork.
- **Not a distribution** — the package is built from this tree and ships the model's profile
  (not one unit's identity). It selects nothing on a host whose DMI does not match (`PROFILE`
  drift, exit 1).

$ grep -n "genarch" /var/log/pacman.log | tail -6
[2026-11-14T15:27:05+0000] [ALPM] installed genarch-settings (0.4.0-1)
[2026-11-14T15:27:05+0000] [ALPM-SCRIPTLET] genarch-settings: refusing to apply: intent cannot be classified
[2026-11-14T15:27:05+0000] [ALPM-SCRIPTLET] genarch-settings: apply did not converge this host (see the message above)
[2026-11-14T15:27:05+0000] [ALPM-SCRIPTLET] genarch-settings: the package is installed; run 'genarch-settings report' to see why
[2026-11-14T15:27:05+0000] [ALPM-SCRIPTLET] Failed to enable unit: Unit genarch-settings.service does not exist

$ ls /etc/tlp.d /etc/systemd/system/genarch-settings.service /var/lib/genarch-settings
ls: cannot access '/etc/tlp.d': No such file or directory
ls: cannot access '/etc/systemd/system/genarch-settings.service': No such file or directory
ls: cannot access '/var/lib/genarch-settings': No such file or directory
```

## What could not be determined

Whether the intended distribution path for a second model is the PKGBUILD or `install.sh`.
`install.sh` does select nothing; the package does not.
