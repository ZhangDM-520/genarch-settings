# genarch-settings — Copilot instructions

A single Rust binary that converges one Arch host to a declared, layered **intent**. It judges
live state read-only (`report`), plans then applies (`apply`), and runs three scheduled passes
(`boot`, `resume`, `ceil`). This repository is the engine plus the profile for this laptop model
(ASUS Zenbook S 16 UM5606WA, Ryzen AI 9 HX 370). The judgement ledger lives in the separate
`system-tuning` repository.

Domain vocabulary is defined in `CONTEXT.md`; use those words exactly (intent, layer, profile,
mirror, managed record, plan, drift, verdict, reach, pass, manifest, pre-image). Interface
reasoning is in `docs/DESIGN.md`; what may ship in a profile, and where the rest goes, in
`docs/PROFILES.md`; architecture rules and hard-won pitfalls in `AGENTS.md`. Do not restate knob
values from the ledger in prose.

## Build, test, lint

Requires a recent Rust toolchain (edition 2024).

```sh
cargo test                    # the gate: unit + converge/layers/cli/profiles + cross-repo contract
cargo build --release --locked
cargo clippy --all-targets    # ad hoc; no lint config, warnings worth fixing
```

There is no CI by owner decision — run the gate locally before committing.

Single test / one suite:

```sh
cargo test --test converge a_converged_host_plans_nothing   # one integration test
cargo test --test layers a_tombstone_reaches_only_the_layers_below_it
cargo test --lib per_core_packs_index_above_offset          # one unit test
cargo test --test profiles                                  # the shipped-profile gate
cargo test --test cli                                       # completion scripts vs the verb tree
```

`tests/contract.rs` reads the ledger through `SYSTEM_TUNING_ROOT` (default
`~/Projects/system-tuning`) and **fails loudly** when that checkout is absent. It asserts the
pinned `tests/contract.tsv` equals the compiled `check::REGISTRY` and the owner copy in
`system-tuning/docs/contract.tsv`, and that every doc anchor resolves to a `##` heading in
`findings.md`. A new or renamed check ID therefore edits the registry, `tests/contract.tsv` and
`system-tuning/docs/contract.tsv` in one change — the two copies disagreeing fails the gate.

Behavioural tests never run as root: they construct `FixtureHost` over a scratch tree, record
writes, and assert byte/mode/atomicity behaviour through `Host`, not by calling module internals.

Useful live commands (unprivileged unless noted):

```sh
./target/debug/genarch-settings report [--json]
sudo ./target/debug/genarch-settings apply [--dry-run]
./target/debug/genarch-settings harvest
./target/debug/genarch-settings units
sudo ./target/debug/genarch-settings converge <boot|resume|ceil>
```

Rehearse without touching the machine: `GENARCH_SETTINGS_ROOT=/path/to/copy` redirects every
live path; `GENARCH_SETTINGS_INTENT=/path/to/intent` swaps the intent. Both are read-only unless
`apply` runs without `--dry-run`.

Packaging and install:

```sh
makepkg -f            # the Arch package: binary + intent + units + udev rules + completions
sudo ./install.sh     # portable path; installs every profile but selects none unless --profile
```

`install.sh` is the portable path: it installs every profile but selects none, so a fresh host
runs the base intent until an operator writes the selection. The PKGBUILD ships this host's
`profile = "zenbook-s16"` selection.

## Architecture

`src/lib.rs` documents the layering; the load-bearing big picture is:

- **`host` is the only seam to the machine.** `trait Host` with `RealHost` and `FixtureHost`.
  No module above it may open a file or spawn a process *for live host state* — every `/etc`,
  `/sys`, `/proc`, manifest and apply-lock access goes through the trait (`host.lock()`, not a
  `flock` call in `converge`). Two exceptions are input data, not host state: `intent` reads its
  own package tree, and `import` reads an offline `state/` tree.
- **`intent` resolves three layers**, precedence `base → profile → operator`:
  base (`/usr/lib/genarch-settings`), profile (`{base,overlay}/profiles/<name>`, selected by
  `profile = "<name>"` in `/etc/genarch-settings/profile`), operator (`/etc/genarch-settings`).
  Scalar maps merge per key; lists replace wholesale; `[units]` is the one union; mirrors and
  managed records are whole-file with an `[omit]` tombstone that reaches only the layers below
  it. No profile means base only — never invent host defaults. Schema is 3; a v2 intent is
  refused, not silently stripped of its `prune` list.
- **`converge` is plan then apply.** `plan` writes nothing and needs no privilege; `apply`
  performs exactly that plan, after a pre-image for every target, under an exclusive lock, then
  re-plans and exits 1 if anything survived. `--dry-run` always exits 0.
- **`managed` owns one anchored line** in a foreign file. Zero or several anchor matches are
  hard errors and write nothing.
- **`check::REGISTRY` is the single table** behind report order, JSON IDs, doc anchors, and how
  far `apply` reaches each row. Adding a row means editing the registry, `tests/contract.tsv` and
  the ledger's `docs/contract.tsv` together. The report is eleven rows; `--json` is schema 2
  (top-level `profile`, then `checks`, then `summary`). MIRROR, MANAGED, SYNC and PRUNE are
  computed from the *same* `converge::plan` that `apply` executes, so "apply said ok but live is
  wrong" cannot happen structurally. `Reach::Intent` rows (CO, POWER) mean "declared", not
  "measured"; a green `apply` never implies a green report.
- **`tree` owns package-tree walking**, the deployable-name rule, and live-path mapping for both
  the loader and the importer. It exists to break the `intent↔converge` cycle.
- **`pass` dispatches boot/resume/ceil**; `ceil` owns the ceiling class map shared by reader and
  writer; `ryzenadj` is argv-building only (packing arithmetic is unit-tested).
- **`import` derives a profile from a legacy shell `state/` tree.** Its acceptance is a
  round-trip property: import → load → render reproduces the source bytes.
- **Exit codes are the operator contract**: `0` clean, `1` drift, `2` precondition, `3` partial.
  `apply` exits 1 on a profile mismatch *when the plan classifies*; an absent managed target
  refuses the whole apply first (exit 3, nothing written). Do not change this mapping while
  units/hooks depend on it.

On disk, `intent/` is the base package data: `intent.toml` (schema, `[units]`, `[smu]` labels),
one typed file per knob family (`sysctl.toml`, `ryzenadj.toml`, `passes.toml`, `ceil.toml`),
`mirror/<live path>` deployed byte for byte, and `managed/<name>.toml` anchored-line records. A
profile repeats that shape under `intent/profiles/<slug>/`. A new typed knob family is a file
plus a registry row; a new mirror or managed record needs no code.

## Conventions that are easy to miss

The authoritative list is `AGENTS.md` ("Conventions" and "Facts that are easy to get wrong") —
read it before changing `src/`. What is specific to working in this repository:

- **One home per fact.** A knob value lives once in the intent; a rule (deployable names, sysctl
  normalisation, ceiling classes, write policy) lives in exactly one function. When you are about
  to restate a fact in a second file, point at the first instead — that is why the commit
  trailers live only in `CONTRIBUTING.md`.
- **A change is judged by declared-vs-live agreement, not by compiling** (`CONTRIBUTING.md`).
  Never debug by mutating the live host: read `report`, preview with `apply --dry-run`, or drive
  `FixtureHost` in a test. A write needs `sudo` and is not a diagnostic.
- **No host identity in anything that ships.** `intent/` is package data read by every host that
  installs the package, so a profile carries no path under `/home/`, no disk UUID, no partition
  device and no other one-unit identity, *measured or documented*. Why, and where that value
  belongs instead: `docs/PROFILES.md`. `tests/profiles.rs` enforces it on file names, on a
  mirror's content and on a managed record's `target`.
- **Nothing in the engine may name a person, a home or a checkout.** `import` finds
  `hw-tune.fish` by name under `state/owned` (and refuses when there are several); a test that
  needs the ledger reads `SYSTEM_TUNING_ROOT`, not an absolute path.
- `.github/copilot-instructions.md` (this file) carries commands, the cross-file big picture and
  navigation. The rules, and the pitfalls that paid for them, live in `AGENTS.md`; if the two ever
  disagree, `AGENTS.md` wins.

## Commits and scope

- `SCOPE.md`: the engine (`src/`), declared state (`intent/`), and the machine profile
  (`intent/profiles/zenbook-s16/`) are in scope; the judgement ledger and benchmarks are out of
  scope. A second machine is a profile directory plus one selection line, not a fork.
- Commit messages name the check ID or knob when one changes; the trailers are specified once, in
  `CONTRIBUTING.md`.
- Do not commit or push without asking the user.
