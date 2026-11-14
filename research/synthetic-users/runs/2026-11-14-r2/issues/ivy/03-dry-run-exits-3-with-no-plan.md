# `apply --dry-run` exits 3 ("partial") and prints no plan when a managed target is absent, though the README says it always exits 0

**Severity:** medium
**Category:** bug
**Environment:** genarch-settings 0.4.0-1; reproduced on the real box and on staging root `~/machine/root.ppd` with a matching `vm-generic` profile.

## Intent

Put `apply --dry-run` in an Ansible diff task (or run it under `set -e`) and read its output before
trusting a real apply.

## Steps

```
GENARCH_SETTINGS_ROOT=$HOME/machine/root.item2 genarch-settings apply --dry-run; echo $?
GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings apply --dry-run; echo $?
sudo genarch-settings apply --dry-run; echo $?
```

(`root.item2` selects `zenbook-s16`; `root.ppd` selects the matching `vm-generic` profile with the
PPD unit removed.)

## Expected

The README makes two promises about exactly this path:

> `--dry-run` always exits 0: it is a preview, and an installer runs it under `set -e`.

and, for the refusal case:

> The plan is still computed and printed, but if a record it carries cannot be classified — an
> absent managed target, such as `asusd.ron` on a machine without the ASUS tooling — the whole
> apply is refused with "nothing was written" and exit 3.

So the preview should print the plan and exit 0; at worst the "cannot be classified" state belongs
to the engine's `2` (precondition) class, not `3` (partial), because nothing is written.

## Actual

Exit 3, four or eight lines, no `PLAN` line at all. `3` is the code the README assigns to a partial
apply; an Ansible task cannot use this to distinguish "preview found a blocker" from "some writes
landed". Nothing is written (`/var/lib/genarch-settings` is not created), so the code is also
factually wrong.

## Evidence

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.ppd genarch-settings apply --dry-run; echo EXIT=$?
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
```

```
$ sudo genarch-settings apply --dry-run; echo EXIT=$?
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
$ ls -d /var/lib/genarch-settings
ls: cannot access '/var/lib/genarch-settings': No such file or directory
```

For contrast, a classifiable dry run exits 0 and does print the plan:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root.pristine genarch-settings apply --dry-run; echo EXIT=$?
PROFILE vm-generic
PLAN 56 change(s)
  OWNED /etc/NetworkManager/conf.d/99-eco-dns.conf
  [...]
DRY-RUN nothing written
EXIT=0
```

## What could not be determined

- Whether the missing plan is deliberate ("refuse before planning") or the refusal happens after a
  plan that is simply not printed. The README says the plan is printed, so at minimum the document
  and the binary disagree.
- Which exit code the maintainer intends for a preview that finds a blocker. `0` is what the
  README promises; `2` would match the code used for a malformed intent (verified: a TOML parse
  error exits 2).
