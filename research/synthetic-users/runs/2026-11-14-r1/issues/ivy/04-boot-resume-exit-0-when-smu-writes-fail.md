# boot and resume exit 0 when every SMU write failed

**Severity:** medium
**Category:** bug

**Environment:** Arch guest (QEMU, no AMD SMU, no real cpufreq), `genarch-settings 0.4.0-1`,
staging tree `~/machine/root` with the `vm-generic` profile selected. `ryzenadj 0.19.0-1` is
installed but cannot see an SMU.

**Intent:** A scheduled pass that could not do its work must fail, so systemd and my monitoring
notice. "Firmware limits may still stand" is exactly the condition I need to alert on.

**Steps:**

```sh
GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot;  echo "EXIT=$?"
GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge resume; echo "EXIT=$?"
```

For contrast, a refused sysfs write in the same pass:

```sh
echo 4000000 > ~/machine/root/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
chmod 0444 ~/machine/root/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge ceil; echo "EXIT=$?"
```

**Expected:** AGENTS.md, "Facts that are easy to get wrong":

> A refused write in a pass is a failure (exit 3), never `.is_ok()`

and the top-level help:

> 3 apply step failed (partial)

**Actual:** Both passes report that both ryzenadj calls failed, then exit **0**. The sysfs path
in the same binary exits 3 correctly, so the SMU path is the exception. The boot unit runs
`ExecStart=/usr/bin/genarch-settings converge boot` with no `-` prefix, so systemd records the
unit as successful while the SMU work did not happen.

**Evidence:**

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge boot; echo "EXIT=$?"
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: ryzenadj failed: ryzenadj --power-saving (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
EXIT=0

$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings converge resume; echo "EXIT=$?"
genarch-settings: ryzenadj failed: ryzenadj --set-coall=0x0000FFFD (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: ryzenadj failed: ryzenadj --power-saving (1): Error: Unable to get SMU object (no AMD SMU on this machine)
genarch-settings: smu: 2 of 2 ryzenadj call(s) failed — firmware limits may still stand
EXIT=0
```

The contrasting sysfs refusal:

```
genarch-settings: CEIL DRIFT policy0 4000000 -> 3000000
genarch-settings: write /home/arch/machine/root/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq: Permission denied (os error 13)
EXIT=3
```

The unit that invokes boot (`/usr/lib/genarch-settings/mirror/etc/systemd/system/genarch-settings.service`):

```
[Service]
Type=oneshot
ExecStart=/usr/bin/genarch-settings converge boot
# Failure-tolerant: a missing intent or a locked apply must never fail the boot chain.
ExecStart=-/usr/bin/genarch-settings apply
```

**Why it matters:** an SMU write that fails at boot (a transient SMU error, a missing module,
a tool regression) is the one failure that leaves the firmware limits in place, and the exit
status says success. The warning is only in the journal, which is exactly the "quiet success"
case. On a real Zenbook the same code path runs; the VM's missing SMU only makes it easy to
trigger.

**What I could not determine:** whether a real SMU can fail transiently in a way the engine
would want to retry (no SMU here). Whether the intended policy is "SMU absence is always
tolerated" — the shipped text does not carve out SMU writes from the exit-3 rule.
