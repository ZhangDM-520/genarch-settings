# CONTEXT — the domain of the tuning engine

Vocabulary shared by the code, the tests, the config syntax and the ledger in `system-tuning`.
Use these words exactly: a synonym here is a bug in the docs.

## The domain

One Arch host has a **declared intent**; the engine judges how far **live** has drifted from it
and converges live back. Nothing about the host is "configured by a script" any more: every
knob is a value in the intent or a file the engine deploys verbatim.

## Concepts

**Knob** — one tunable thing with one value (a sysctl, a curve offset, a power limit, a
ceiling). A knob has exactly one home for its value; the ledger records *why*, never the number.

**Intent** — the whole declared state: typed config (`sysctl.toml`, `ryzenadj.toml`,
`passes.toml`), byte-exact **mirror** files, and **managed records**. Loaded from
`/usr/lib/genarch` with whole-file overrides from `/etc/genarch`.

**Mirror** — a file the engine deploys byte for byte and does not interpret. It is how the
engine owns `/etc/fstab`-shaped files without becoming a parser for every vendor format.

**Managed record** — one anchored line inside a file the engine does *not* own. Identity is the
anchor pattern, not a line number.

**Plan** — the complete list of divergences between intent and live, computed before anything is
written. `apply --dry-run` prints it; `apply` executes exactly it.

**Drift** — a divergence the plan found. Drift after an apply is exit 1, never silence.

**Verdict** — one row of the report: a **class** (`OK`, `ACCEPTED`, `WARN`, `DRIFT`, `SKIPPED`)
plus a one-line explanation and the doc anchor that owns the reasoning.

**Reach** — how far `apply` can move a row: `full`, `file-only` (the live effect has another
owner), `none`. Printed with every plan, because apply exiting 0 never implies the report is
green.

**Pass** — a scheduled unit of work triggered outside the operator's hands: `boot`, `resume`,
`ceil`. The split is a safety property, not an optimisation (see `docs/DESIGN.md`).

**Manifest** — the engine's ownership record (`/var/lib/genarch/manifest.toml`). It is what makes
"one writer per file" checkable and what lets a target leave the intent without being forgotten
on disk (**prune**).

**Pre-image** — the dated `.bak-YYYYMMDD` copy taken before a write. Never deleted.

## Load-bearing rules

- A knob's value lives once, in the intent. The ledger does not restate it.
- A write the engine cannot classify is refused, not guessed (zero or several anchor matches).
- Every write is preceded by a pre-image and applied atomically.
- The live host is never the place to make a change: intent → plan → apply.
- `SKIPPED` means unverified, not fine. It counts as green only because the alternative is
  training an operator to ignore the exit code.
