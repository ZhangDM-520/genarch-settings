# `pacman -U` exits 0 although its post-install apply refused to converge

**Severity:** medium
**Category:** enhancement (blocks unattended package rollouts gated on exit status)
**Environment:** Arch Linux, QEMU/KVM, fresh install from `~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst`; genarch-settings 0.4.0-1.

## Intent

Roll the package onto three headless boxes from Ansible and fail the play when the box did not
converge, without parsing human output.

## Steps

```
sudo pacman -R --noconfirm genarch-settings
sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
echo $?
genarch-settings report; echo $?
```

## Expected

The package install is the documented install path:

> On Arch, with the recipe next to the `gsa` recipe set:
> `makepkg -f && sudo pacman -U genarch-settings-0.4.0-1-x86_64.pkg.tar.zst`

An unattended rollout should be able to gate on the command it runs. The engine's own contract is
explicit that a failed convergence is not success:

> Exit codes keep the shell engine's contract: `0` clean · `1` drift · `2` precondition · `3` partial.

## Actual

`pacman -U` returns 0. The scriptlet's `apply` returned 3, wrote nothing, and the scriptlet's
`systemctl enable` also failed, but the package transaction is reported successful. The only
signal is prose on the captured stream; `$?` says success and `report` is a separate command the
operator has to know to run.

## Evidence

```
$ sudo pacman -U --noconfirm ~/payload/genarch-settings-0.4.0-1-x86_64.pkg.tar.zst
INSTALL_EXIT=0
[...]
:: Processing package changes...
installing genarch-settings...
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  [...]
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
genarch-settings: apply did not converge this host (see the message above)
genarch-settings: the package is installed; run 'genarch-settings report' to see why
Failed to enable unit: Unit genarch-settings.service does not exist
:: Running post-transaction hooks...
$ sudo ls -la /var/lib/genarch-settings
ls: cannot access '/var/lib/genarch-settings': No such file or directory
$ genarch-settings report; echo $?
[...]
SUMMARY ok=3 accepted=0 warn=0 drift=5 skipped=3 exit=1
1
```


## What could not be determined

- Whether a package scriptlet is allowed to fail the transaction in the maintainer's model; the
  scriptlet comment says the install "stays non-fatal — a boot chain must not break", so this may
  be deliberate. If so, the README's Install section needs to say how an unattended installer is
  supposed to detect the failure (for example "run `genarch-settings report` after install").
- Whether `power-profiles-daemon` is expected to be present on a server (see issue 01).
