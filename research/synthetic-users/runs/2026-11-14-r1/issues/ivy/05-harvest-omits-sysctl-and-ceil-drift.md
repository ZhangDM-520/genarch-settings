# harvest prints nothing for a sysctl drift that report calls DRIFT

**Severity:** medium
**Category:** bug

**Environment:** Arch guest; `genarch-settings 0.4.0-1`. Reproduced on a converged copy of the
staging tree (`~/machine/root.e2e`, applied and ceil-passed; it is a copy of `root.pristine`,
which is a byte copy of the delivered `~/machine/root` taken before any apply) and on the real
install.

**Intent:** Use `harvest` to adopt a live value into the intent (the documented purpose), or at
least to see every discrepancy the report finds.

**Steps (staging, isolated to one sysctl):**

```sh
export GENARCH_SETTINGS_ROOT=$HOME/machine/root.e2e
echo 60 > $GENARCH_SETTINGS_ROOT/proc/sys/vm/swappiness   # intent wants 150
genarch-settings report;  echo "REPORT=$?"
genarch-settings harvest; echo "HARVEST=$?"
```

**Steps (real install, many sysctls differ):**

```sh
genarch-settings harvest | grep -i -E 'sysctl|swappiness|dirty|printk'; echo "GREP=$?"
```

**Expected:** The command's own help:

> Print the intent change that would match live. Writes nothing

and AGENTS.md:

> `harvest` is the only verb that proposes intent changes; it never writes.

The report already prints the change it should propose (`vm.swappiness=60 want 150`).

**Actual:** `harvest` exits 0 and prints zero lines. On the real install it prints only the
missing mirror files and the journald managed diff, and nothing for the 13 sysctl values the
report lists as `DRIFT SYNC`. Ceiling drift is likewise absent (I changed `scaling_max_freq` to
4000000 and `harvest` printed nothing).

**Evidence:**

Staging, sysctl-only drift:

```
$ genarch-settings report; echo "REPORT=$?"
DRIFT SYNC vm.swappiness=60 want 150 findings#writeback
...
SUMMARY ok=8 accepted=0 warn=0 drift=1 skipped=2 exit=1
REPORT=1

$ genarch-settings harvest; echo "HARVEST=$?"
0
HARVEST=0
```

(The `0` is `wc -l < harvest-output`; the file is empty.)

Ceiling drift, same converged copy:

```
$ echo 4000000 > $GENARCH_SETTINGS_ROOT/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq
$ genarch-settings report; echo "REPORT=$?"
DRIFT CEIL policy0=4000000 want 3000000 findings#ceil
...
REPORT=1

$ genarch-settings harvest; echo "HARVEST=$?"
0
HARVEST=0
```

Real install:

```
$ genarch-settings harvest
--- intent: /etc/NetworkManager/conf.d/99-eco-dns.conf (live missing)
--- intent: /etc/NetworkManager/conf.d/99-wifi-powersave.conf (live missing)
... (every mirror line, all "live missing") ...
--- managed journald in /etc/systemd/journald.conf
- #SystemMaxUse=
+ SystemMaxUse=256M

$ genarch-settings harvest | grep -i -E 'sysctl|swappiness|dirty|printk'; echo "GREP=$?"
GREP=1
```

The same real host's report has 13 sysctl differences (including
`vm.swappiness=60 want 150` and `vm.page-cluster=3 want 0`).

**Why it matters:** `harvest` is the advertised way to turn a running box into intent. It
silently omits the largest class of values, so an operator who harvests a host gets an intent
that does not describe it, with exit 0. The omission is invisible unless they diff the report
and the harvest output by hand.

**What I could not determine:** whether the omission is intentional. No shipped text limits
`harvest` to mirrors and managed records; the help and AGENTS both state the general purpose.
