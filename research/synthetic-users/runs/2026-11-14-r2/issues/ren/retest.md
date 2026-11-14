# Round 2 retest

Box: fresh Arch VM, `genarch-settings 0.4.0-1`, staging tree `~/machine` (no profile selected in
its `root/`), rehearsal copies under `~/work`, `~/t2`, `~/t4`, `~/t5`, `~/t6`, `~/t7`, `~/t8`.
`strace` installed with `pacman -Sy strace`.

## 1. The profile and layer contract a porter is told to follow is not installed — **gone**

```
$ pacman -Ql genarch-settings | grep -E "PROFILES|CUTOVER|DESIGN"
genarch-settings /usr/share/doc/genarch-settings/DESIGN.md
genarch-settings /usr/share/doc/genarch-settings/PROFILES.md
```

`PROFILES.md` is installed and answers the porting questions I had last round: naming,
`status`/`source`, `[requires]`, band ceilings, and what a documented profile may carry. Two
things it still does not answer are filed separately (the `[omit]` shape, issue 02; `ryzenadj.toml`
table names, issue 03), and the profile gate it names is not runnable here (issue 04).

## 2. A selected profile with no `[requires]` is reported OK and its facts applied — **still**

```
$ mkdir -p ~/t2/etc/genarch-settings/profiles/no-requires
$ printf '# no [requires] at all\n' > ~/t2/etc/genarch-settings/profiles/no-requires/intent.toml
$ printf '[[class]]\nname = "x"\nkhz = 3500000\n' > ~/t2/etc/genarch-settings/profiles/no-requires/ceil.toml
$ printf 'profile = "no-requires"\n' > ~/t2/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/t2 genarch-settings report | grep -E "PROFILE|CEIL|SUMMARY"
OK PROFILE profile 'no-requires' declares no machine requirement findings#intent
DRIFT CEIL policy0=4000000 want 3500000 policy1=4000000 want 3500000 findings#ceil
SUMMARY ok=2 accepted=0 warn=0 drift=5 skipped=4 exit=1
```

The row is `OK`, not `SKIPPED` or `DRIFT`, and the profile's ceiling is judged against the host.
`README.md` says "A profile declares the machine it is for, and `report` says `PROFILE DRIFT`
(exit 1) when it is applied to another one" — a profile with no `[requires]` can never drift, so
the report asserts a fact it never checked.

## 3. A rehearsal still executes live ryzenadj, iw, systemctl and udevadm — **gone**

```
$ GENARCH_SETTINGS_ROOT=$HOME/work/root strace -f -e trace=execve -o ~/work/apply.strace genarch-settings apply > ~/work/apply.out 2>&1; echo $?
0
$ grep -E "execve\(" ~/work/apply.strace
4490  execve("/usr/bin/genarch-settings", ["genarch-settings", "apply"], 0x7fff0def5610 /* 19 vars */) = 0
```

The same trace on `converge boot`, `resume` and `ceil` (with `ryzenadj.toml` `[limits]` and
`passes.toml wifi_interface = "wlan0"` declared, so every affected path is configured) records no
external program at all; the engine reports the suppression itself:

```
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings converge boot; echo "exit=$?"
genarch-settings: ryzenadj failed: ryzenadj --stapm-limit=28000 --fast-limit=35000 --slow-limit=30000 (-1): rehearsal root: not executing ryzenadj
...
exit=3
```

No `ryzenadj`, `iw`, `systemctl` or `udevadm` process is spawned. The exit 3 wording is filed as
issue 06.

## 4. A profile mirror at the sysctl projection path is accepted and apply never converges — **gone**

```
$ mkdir -p ~/t4/etc/genarch-settings/profiles/p4/mirror/etc/sysctl.d
$ printf 'vm.swappiness = 60\n' > ~/t4/etc/genarch-settings/profiles/p4/mirror/etc/sysctl.d/90-genarch-settings.conf
$ GENARCH_SETTINGS_ROOT=$HOME/t4 genarch-settings report
genarch-settings: /etc/sysctl.d/90-genarch-settings.conf is written by the engine from sysctl.toml: a mirror cannot also own it
$ echo $?
2
```

Refused at load, before a plan exists, with a reason. Same message under `apply --dry-run`.

## 5. A wrong-machine profile is a hard exit-3 refusal, not the documented exit-1 apply — **still**

```
$ printf 'profile = "zenbook-s16"\n' > ~/t5/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/t5 genarch-settings apply
SUMMARY applied=0 failed=1 exit=3
genarch-settings: refusing to apply: intent cannot be classified
  asusd-ac-cmd: target /etc/asusd/asusd.ron does not exist
  ...
nothing was written
$ echo $?
3
```

The documented variant now exists for a *documented* profile on the same box:

```
$ printf 'profile = "framework-13-amd"\n' > ~/t5b/etc/genarch-settings/profile
$ GENARCH_SETTINGS_ROOT=$HOME/t5b genarch-settings apply 2>&1 | tail -1
SUMMARY planned=56 applied=56 failed=0 exit=1
```

So the behaviour is unchanged for the shipped measured profile (`zenbook-s16`, the only one with
managed records), while `README.md` still states "`apply` exits 1 when the selected profile does
not match the machine, even though it applied". `README.md` also now documents the exit 3 for an
unclassifiable record in the next paragraph, so the page contains both claims, and
`--help` calls exit 3 "apply step failed (partial)" although nothing was written.

## 6. harvest prints nothing for sysctl and ceiling drift — **gone**

```
$ GENARCH_SETTINGS_ROOT=$HOME/t6 genarch-settings harvest
...
--- sysctl kernel.printk (live 4 4 1 7)
- 3 3 3 3
+ 4 4 1 7
...
--- ceil policy0 (live 4000000)
--- ceil policy1 (live 4000000)
```

(with a profile selected for the ceil part). Exit 0.

## 7. report shows the winning value but never the layer or the profile provenance — **different**

`--json` now carries the profile:

```
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings report --json | python3 -c '...'
{'schema': 2, 'profile': {'name': 'qemu-i440fx', 'matched': True, 'requires': {'product': 'Standard PC (i440FX + PIIX, 1996)', 'sys_vendor': 'QEMU'}}, 'summary': {...}}
```

and the text row names it (`OK PROFILE profile 'qemu-i440fx' matches product=...`). The per-value
layer is still absent: after adding `"vm.swappiness" = "120"` to `~/work/root/etc/genarch-settings/
sysctl.toml` the report says only

```
vm.swappiness=60 want 120
```

with nothing saying 120 came from the operator layer and 150 from base — and the `[omit]` exercise
removed a mirror with no output line mentioning the tombstone at all.

## 8. A host with no profile selected exits 0 once the other rows are green — **still (partial reproduction)**

```
$ GENARCH_SETTINGS_ROOT=$HOME/t8 genarch-settings apply > ~/t8.apply 2>&1; echo $?
0
$ GENARCH_SETTINGS_ROOT=$HOME/t8 genarch-settings report
SKIPPED PROFILE no profile selected (base intent only) findings#intent
OK MIRROR 39 mirror target(s) byte-equal findings#intent
OK MANAGED 2 record(s) hold their line findings#managed
OK SYNC 16 runtime sysctl value(s) match intent findings#writeback
SKIPPED CEIL ...
DRIFT TIMER not enabled: genarch-settings.service genarch-settings-ceil.timer findings#timer
OK PRUNE 40 owned path(s) in the manifest, none to remove findings#intent
SUMMARY ok=4 accepted=0 warn=0 drift=1 skipped=6 exit=1
```

Everything except `TIMER` is green or `SKIPPED`, and `SKIPPED` contributes no drift, so on a host
where `TIMER` is green the summary is exit 0 with no profile selected. I could not produce that
end-to-end here: under a rehearsal root `TIMER` is always `DRIFT` (filed as issue 07), and the
real root of this box is not converged. The design is unchanged: `PROFILE SKIPPED` is treated as
success; `README.md` calls it the normal state until an operator writes the selection, and
`CONTEXT.md` says "`SKIPPED` means unverified, not fine. It counts as green".
