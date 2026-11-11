# Contributing

The engine converges a live machine, so a change is judged by whether the declared state and the
live one still agree — not by whether it compiles.

## The gate

```sh
cargo test                 # unit + behavioural (fixture host) + layer + cross-repo contract
cargo clippy --all-targets # ad hoc; there is no lint config, and warnings are worth fixing
```

There is no CI, by owner decision. The gate is what you run before you commit, and the contract
test reads `system-tuning/docs/findings.md` through `SYSTEM_TUNING_ROOT` (default
`~/Projects/system-tuning`); without that checkout it fails loudly rather than passing vacuously.

On the host itself:

```sh
genarch report            # drift=0 is the bar
genarch apply --dry-run   # PLAN 0 on a converged host
```

`harvest` is the only verb that proposes intent changes; it never writes.

## Rules a change must not break

`AGENTS.md` is the authoritative list; the three that catch most mistakes:

1. **One home per fact.** A knob value lives once in the intent. A rule (deployable names, sysctl
   normalisation, ceiling classes, the write policy) lives in exactly one function.
2. **Host facts never enter the base layer.** Disk UUIDs, user paths, silicon offsets, interface
   names and device IDs belong in a profile; the base must stay true of any host.
3. **Every live mutation goes through `Host`.** No module above `host` opens a file or spawns a
   process for live state, and the fixture must exercise the same policy the machine does.

## Commits

Name the check ID or the knob when a change touches one, and add:

```
Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
```

## Licence

MIT — see `LICENSE`. By contributing you agree your work is licensed the same way.
