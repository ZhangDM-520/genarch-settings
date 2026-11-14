# An unusable intent exits 3 (partial apply) even though nothing was written

**Severity:** medium
**Category:** bug

**Environment:** Arch guest; `genarch-settings 0.4.0-1`. Cases reproduced on the real install
and on copies of the staging tree (`root.pristine` is a byte copy of the delivered
`~/machine/root` taken before any apply; `root.bad` is a copy of it).

**Intent:** Branch on exit status from Ansible/cron. `2` must mean "fix the config", `3` must
mean "some writes happened, re-run and inspect". If an intent the engine cannot classify
returns `3` with no writes, a retry loop cannot tell it apart from a real partial apply.

**Steps:**

```sh
# real install: unclassifiable managed targets
sudo genarch-settings apply --dry-run; echo "EXIT=$?"

# staging: malformed TOML (a precondition the engine does classify)
cp -a ~/machine/root.pristine ~/machine/root.bad
printf 'kernel.printk = "3 3 3 3"\n' > ~/machine/root.bad/etc/genarch-settings/sysctl.toml
GENARCH_SETTINGS_ROOT=$HOME/machine/root.bad genarch-settings report;        echo "EXIT=$?"
GENARCH_SETTINGS_ROOT=$HOME/machine/root.bad genarch-settings apply --dry-run; echo "EXIT=$?"

# staging: two anchor matches in a managed target
printf '[Journal]\n#SystemMaxUse=\nStorage=auto\nSystemMaxUse=999M\n' \
  > ~/machine/root/etc/systemd/journald.conf
GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply; echo "EXIT=$?"

# staging: a genuine partial apply (a read-only directory midway through the plan)
cp -a ~/machine/root.pristine ~/machine/root.partial4
mkdir -p ~/machine/root.partial4/etc/systemd/journald.conf.d
chmod 0555 ~/machine/root.partial4/etc/systemd/journald.conf.d
GENARCH_SETTINGS_ROOT=$HOME/machine/root.partial4 genarch-settings apply; echo "EXIT=$?"
```

**Expected:** The shipped help defines the mapping:

> Exit codes: 0 clean (rows may be SKIPPED where a check needs root), 1 drift present or
> persisted after apply, 2 precondition failure, 3 apply step failed (partial).

and CONTEXT says the write is refused, not guessed:

> A write the engine cannot classify is refused, not guessed (zero or several anchor matches).

An intent that cannot be classified is a precondition: the engine itself prints "nothing was
written".

**Actual:** The unclassifiable cases exit **3**, the same code as a mid-apply write failure. The
same condition is called **drift (1)** by `report`, while a TOML parse error exits **2** from
both verbs. So one class ("the intent cannot be used, nothing was attempted") produces 1 in
`report`, 2 for a parse error, and 3 for an unclassifiable managed record.

**Evidence:**

Real install:

```
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-bat-cmd: target /etc/asusd/asusd.ron does not exist
  asusd-epp: target /etc/asusd/asusd.ron does not exist
  asusd-profile-ac: target /etc/asusd/asusd.ron does not exist
  asusd-profile-bat: target /etc/asusd/asusd.ron does not exist
  ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist
nothing was written
EXIT=3
```

Parse error (same class of "cannot use the intent"):

```
genarch-settings: parse /home/arch/machine/root.bad/etc/genarch-settings/sysctl.toml: TOML parse error at line 1, column 1
  |
1 | kernel.printk = "3 3 3 3"
  | ^^^^^^
unknown field `kernel`, expected `values`

REPORT_EXIT=2
DRY_EXIT=2
```

`report` for the same unclassifiable record:

```
DRIFT MANAGED unclassifiable: ppd-conflict: target /usr/lib/systemd/system/power-profiles-daemon.service does not exist findings#managed
SUMMARY ok=1 accepted=0 warn=0 drift=4 skipped=6 exit=1
REPORT_EXIT=1
```

Two anchor matches:

```
genarch-settings: refusing to apply: intent cannot be classified
  journald: 2 lines match anchor in /etc/systemd/journald.conf — refusing to choose
nothing was written
EXIT=3
```

A genuine partial apply also exits 3 (a read-only directory in the middle of the plan):

```
EXIT=3
genarch-settings: create /home/arch/machine/root.partial4/etc/systemd/journald.conf.d/.99-eco-size.conf.4872.tmp: Permission denied (os error 13)
```

So `3` means both "nothing was written, the intent is unclassifiable" and "some writes landed".

**Why it matters:** Ansible's retry policy and my own runbooks key on these codes. `3` is the
"partial" code; an unclassifiable intent is not partial and retrying cannot help. `report` says
`1` for the same state, so a check step and an apply step classify the same defect differently.

**What I could not determine:** whether any caller in the shipped units relies on `3` for the
unclassifiable case (the units call `converge`, not `apply`). The intent behind the code
mapping is documented only as the four-line help text.
