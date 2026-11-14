# The `[omit]` tombstone is named in the docs but its file shape is written nowhere

- **Severity:** low
- **Category:** doc
- **Environment:** Arch Linux VM, genarch-settings 0.4.0-1, rehearsal root `~/machine/root`

## Intent

Perform the documented layer exercise: "disable one shipped file without forking it" and show
which layer wins. The shipped material must be enough to do this.

## Steps

1. Read every mention of `omit` in the installed docs.
2. Write what the sentence suggests in the operator layer: `~/root/etc/genarch-settings/intent.toml`,
   `[omit] mirror = ["etc/tlp.d/01-eco.conf"]`.
3. Run `GENARCH_SETTINGS_ROOT=... genarch-settings report`.
4. Guess again with the live path `/etc/tlp.d/01-eco.conf`.

## Expected

The shipped contract should show the record shape. It only says:
"mirrors and managed records are whole-file with an explicit `omit = [...]` tombstone, which
reaches only the layers below it" (`CONTEXT.md`), "`[omit]` in a layer removes a mirror or a
managed record declared under it" (`DESIGN.md`), and "`mirror` and `managed` are disjoint"
(`AGENTS.md`). No table key, path form, or example appears in `PROFILES.md`, `README.md`,
`CONTEXT.md` or `DESIGN.md`.

## Actual

The first guess is rejected at load; the accepted key and the live-path form are found only by
trial and by reading the error text. Once found it works (40 mirror targets become 39, the tlp
file disappears from `MIRROR`). The report gives no indication that a tombstone removed it — the
row count simply drops.

## Evidence

```
$ grep -rn "omit" /usr/share/doc/genarch-settings/*.md
... CONTEXT.md:26 ... explicit `omit = [...]` tombstone ...
... DESIGN.md:139 ... `omit` tombstone ...
... AGENTS.md:122 ... An `[omit]` tombstone reaches only the layers **below** ...

$ cat ~/work/root/etc/genarch-settings/intent.toml
schema = 3

[omit]
mirror = ["etc/tlp.d/01-eco.conf"]
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings report
genarch-settings: /home/arch/work/root/etc/genarch-settings: [omit] mirror names etc/tlp.d/01-eco.conf, which no layer below it ships
$ echo $?
2

$ cat ~/work/root/etc/genarch-settings/intent.toml
schema = 3

[omit]
mirror = ["/etc/tlp.d/01-eco.conf"]
$ GENARCH_SETTINGS_ROOT=$HOME/work/root genarch-settings report | head -2 | cut -c1-120
OK PROFILE profile 'qemu-i440fx' matches product=Standard PC (i440FX + PIIX, 1996) sys_vendor=QEMU findings#intent
DRIFT MIRROR 39 target(s) differ: /etc/NetworkManager/conf.d/99-eco-dns.conf ...
```

## What could not be determined

Whether `[omit] managed = [...]` takes a record name or a target path; the docs name only the
record concept, and I did not need it.
