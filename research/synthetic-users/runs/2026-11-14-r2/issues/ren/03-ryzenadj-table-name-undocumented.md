# `ryzenadj.toml`'s section name is undocumented; the natural guess is refused

- **Severity:** low
- **Category:** doc
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1, rehearsal root `~/machine/root`

## Intent

Write the `ryzenadj.toml` of a new measured profile from `PROFILES.md` alone.

## Steps

1. `PROFILES.md` says: "`ryzenadj.toml` — only if the envelope is published **in ryzenadj terms**
   (`stapm`/`fast`/`slow`, mW)". Follow it literally by putting those keys under `[values]`, the
   same spelling `sysctl.toml` uses.
2. Run `GENARCH_SETTINGS_ROOT=... genarch-settings report`.
3. Fix to the only accepted spelling the error prints.

## Expected

The doc that names the file and its three values should also name the table they live in (or
show a five-line example), as it does for `[requires]` and for `[[class]]` in `ceil.toml`.

## Actual

`[values]` is refused with a loader error that is the only place `limits` appears in the shipped
material; `PROFILES.md` never says it. The reference profile
`/usr/lib/genarch-settings/profiles/zenbook-s16/ryzenadj.toml` does show `[limits]`, so it is
recoverable by reading package data, not by reading the guide.

## Evidence

```
$ cat /usr/share/doc/genarch-settings/PROFILES.md | grep -n "ryzenadj"
61:- `ryzenadj.toml` — only if the envelope is published **in ryzenadj terms** (`stapm`/`fast`/
62:  `slow`, mW). A vendor "TDP" is not one: leave it out and let `SMU` report `SKIPPED`.

$ cat ~/work/root/etc/genarch-settings/profiles/qemu-i440fx/ryzenadj.toml
[values]
stapm = 28000
fast = 35000
slow = 30000
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings report
1 | [values]
  |  ^^^^^^
unknown field `values`, expected one of `co`, `limits`, `raw`
$ echo $?
2
```

## What could not be determined

Whether `[raw]` carries arbitrary argv (the docs forbid raw argv for CO but say nothing about
this table).
