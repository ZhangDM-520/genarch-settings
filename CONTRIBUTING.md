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
genarch-settings report            # drift=0 is the bar
genarch-settings apply --dry-run   # PLAN 0 on a converged host
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

Name the check ID or the knob when a change touches one, and end the message with this trailer
pair. This block is the trailers' **one home**; `AGENTS.md` and
`.github/copilot-instructions.md` point here rather than restating it, so rotating the identity
is a one-line change.

```
Assisted-by: LLM (human-reviewed)
Signed-off-by: ZhangDM-520 <jixianqiang1540@gmail.com>
```

These two lines are the whole set; do not add a `Co-authored-by` trailer.

## Licence

MIT — see `LICENSE`. By contributing you agree your work is licensed the same way.
