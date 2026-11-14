# `apply --dry-run` exits 3, though the README says a preview always exits 0

- **Severity**: medium
- **Category**: bug
- **Environment**: Arch Linux 7.2.11-arch1-1 (QEMU), `genarch-settings 0.4.0-1`;
  rehearsal root `~/machine/root` (`GENARCH_SETTINGS_ROOT`)

## Intent

Preview the plan for a machine before anything is written — the way the README's install story
does, and the way an installer can safely call a preview under `set -e`.

## Steps

```
~/machine.sh ~/machine
export GENARCH_SETTINGS_ROOT=$HOME/machine/root
rm $HOME/machine/root/usr/lib/systemd/system/power-profiles-daemon.service
genarch-settings apply --dry-run
echo "EXIT:$?"
```

## Expected

The README, "What it does":

> `--dry-run` always exits 0: it is a preview, and an installer runs it under `set -e`.

## Actual

Exit 3, with no plan at all — only the refusal:

```
genarch-settings: refusing to apply: intent cannot be classified
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
SUMMARY applied=0 failed=1 exit=3
EXIT:3
```

## Evidence

Verbatim run above. The same tree without the missing target:

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply --dry-run >/dev/null; echo "EXIT:$?"
EXIT:0
```

so the nonzero exit is specific to the refusal path, and a preview that is meant to be a preview
still kills an installer that branches on the exit code.

## What could not be determined

Whether the intent is to exempt only `--dry-run` or to give the refusal a distinct code; the
README states the "always 0" rule without exceptions, and the refusal is documented separately as
"exit 3", so the two shipped sentences disagree with each other.
