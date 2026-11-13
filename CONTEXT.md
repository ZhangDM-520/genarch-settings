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
`passes.toml`, `ceil.toml`), byte-exact **mirror** files, and **managed records**. Loaded from
three **layers** (see below), never from one tree.

**Layer** — one root in precedence order: **base** (`/usr/lib/genarch-settings`), **profile**
(`/usr/lib/genarch-settings/profiles/<name>` or `/etc/genarch-settings/profiles/<name>`), **operator**
(`/etc/genarch-settings`). Precedence is base → profile → operator; the operator layer always wins.
Scalar maps merge per key, lists replace wholesale (the one union is `[units]`: each layer adds
an expected-enabled entry and cannot withdraw another's), and mirrors and managed records are
whole-file with an explicit `omit = [...]` tombstone, which reaches only the layers below it.

**Profile** — the machine-specific layer: a directory shaped like the base, selected by the
`profile = "<name>"` line in `/etc/genarch-settings/profile`. It carries every fact about the
*machine model* that is a measurement or an identity — this silicon's curve offsets, this
machine's ceilings, this model's device IDs. It does **not** carry one unit's own identity: a
disk's `fstab`, a user's home paths and any other value unique to the laptop in hand belong in
the operator layer, because a profile ships as package data and is read by every host that
installs it. The base carries only what is true of any AMD laptop. **Empty is legal**: with no
profile selected, absent sections do nothing and the rows that depend on them report `SKIPPED` —
there are no host defaults in the binary.

A profile also states how it came to be: **documented** (every value published and cited) or
**measured** (observed on the machine). The distinction is load-bearing, not cosmetic — a
documented profile ships no `mirror/`, no curve offset and no wake list, because those cannot be
published. `docs/PROFILES.md` is the contract.

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
owner), `intent` (judged against the intent alone, so a green row means "declared", not
"measured"), `none`. Printed with every plan, because apply exiting 0 never implies the report is
green.

**Pass** — a scheduled unit of work triggered outside the operator's hands: `boot`, `resume`,
`ceil`. The split is a safety property, not an optimisation (see `docs/DESIGN.md`).

**Manifest** — the engine's ownership record (`/var/lib/genarch-settings/manifest.toml`). It is what makes
"one writer per file" checkable and what lets a target leave the intent without being forgotten
on disk (**prune**). It is the only source of a removal: there is no declared prune list. A path
belonging to a profile that is merely not selected stays owned (**dormant**), so a lost selection
cannot delete a system file.

**Pre-image** — the dated `.bak-YYYYMMDD` copy taken before a write. Never deleted.

## Load-bearing rules

- A knob's value lives once, in the intent. The ledger does not restate it.
- A write the engine cannot classify is refused, not guessed (zero or several anchor matches).
- Every write is preceded by a pre-image and applied atomically.
- The live host is never the place to make a change: intent → plan → apply.
- `SKIPPED` means unverified, not fine. It counts as green only because the alternative is
  training an operator to ignore the exit code.
