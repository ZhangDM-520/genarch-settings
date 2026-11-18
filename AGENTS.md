# AGENTS.md — genarch-settings

Last reviewed: 2026-11-17.

## What this repository is

One Rust binary that converges one Arch host to a declared, layered **intent**: `report` judges read-only, `apply` plans then applies, `converge <boot|resume|ceil>` runs the scheduled passes. The repository is the engine, the host-agnostic base intent and the machine profiles; the ledger of *why* a knob has its value, and the other side of the cross-repo contract, live in the separate `system-tuning` repository, whose retired shell `eco` stack this engine replaces and which remains the rollback artifact.

Mental model: `src/` (one module per seam: `host`, `intent`, `tree`, `converge`, `managed`, `check`/`report`, `ryzenadj`, `ceil`, `pass`, `import`, `cli`), `intent/` (base + `profiles/<slug>/`), `tests/` (fixture-hosted behavioural, layer, profile, CLI and cross-repo contract gates), `install.sh`/`PKGBUILD` (two packaging paths over one intent tree). Vocabulary is `CONTEXT.md`; interface reasoning `docs/DESIGN.md`; what a profile may ship `docs/PROFILES.md`.

## How to work here

- Gate: `cargo test` before every commit, then `cargo build --release --locked`. `cargo clippy --all-targets` is ad hoc; there is no CI by owner decision.
- Narrower runs: `cargo test --test <converge|layers|profiles|cli> [name]`, `cargo test --lib <name>`.
- Live: unprivileged for `report`, `harvest`, `units`, `completions`, `import`; root for `apply [--dry-run]` and `converge <boot|resume|ceil>`.
- Rehearse with `GENARCH_SETTINGS_ROOT` / `GENARCH_SETTINGS_INTENT`; never debug by mutating live host state, and never run the gate as root.
- The cross-repo contract reads the `system-tuning` checkout (`SYSTEM_TUNING_ROOT` overrides the default location) and fails loudly when it is absent, so a missing ledger is red, not vacuously green.
- Packaging: `makepkg -f` (ships the measured profile's selection); `sudo ./install.sh` (portable, installs every profile but selects none unless `--profile`).
- Commit subjects name the check ID or the knob; trailers are in `CONTRIBUTING.md`.
- Ask before committing or pushing.

Full command reference and the cross-repo rule digest are in `.github/copilot-instructions.md`; this file wins if the two disagree.

## Rules and invariants

- **One home per fact**: a knob value lives once in `intent/`; its rationale in `system-tuning/docs/findings.md`; a rule (`tree::is_deployable_name`, `converge::norm_sysctl`, the ceiling class map, the write policy) in exactly one function; commit trailers in `CONTRIBUTING.md`.
- **Host facts never enter the base layer, and a unit's identity never enters a shipped profile**: disk UUIDs, home-directory paths, partition devices and silicon offsets belong to a profile (model facts) or the operator layer `/etc/genarch-settings` (one unit's facts). `tests/profiles.rs` enforces it.
- **Every live mutation goes through `Host`** (`RealHost`/`FixtureHost`), including the manifest, the apply lock (`Host::lock`), pre-image bytes and modes. Offline input reads are the only exceptions: the `intent` package tree and `import`'s legacy `state/` tree.
- **Pre-images before every write**: dated `<file>.bak-YYYYMMDD`, byte-exact, owner and mode carried on the open fd (`fchown`/`fchmod` before `fsync`/`rename`), one per target per day, never deleted.
- **Kernel attributes are written in place**: `/sys` and `/proc` refuse a sibling temp, and sysfs parses one `write(2)` syscall — no temp+rename, no `write_all`.
- **The manifest is the ownership record**: `owned` is prune-eligible (mirrors and the generated sysctl projection), `touched` holds managed targets and is never pruned; a deselected profile's paths stay dormant-owned so a lost selection cannot prune `/etc/fstab`.
- **Exit codes are the operator contract**: `0` clean, `1` drift, `2` precondition, `3` partial. `apply`'s `0` means the plan it can reach was applied — `CEIL`/`TIMER` are reach `none` and `Reach::Intent` rows (`CO`, `POWER`) are declared, not measured, so a green apply never implies a green report.
- **Pass isolation**: SMU writes belong to `boot`/`resume`; `ceil` is drift-only; every scheduled pass takes the apply lock; `resume` must never sleep.
- **A new or renamed check ID edits three places at once**: `check::REGISTRY`, `tests/contract.tsv`, and `system-tuning/docs/contract.tsv`, with every doc anchor resolving to a `##` heading in `findings.md`.
- **Ask before committing or pushing.**

## Pitfalls index

Detail and mechanisms: the maintainer's private warehouse holds the pitfall and lesson records. Each line here is the actionable rule.

- Quote dotted sysctl keys (`"kernel.printk" = …`) — a bare key parses as a nested TOML table and silently renames the knob.
- Compare sysctl values whole and whitespace-normalised (`converge::norm_sysctl`) — `/proc/sys` emits tabs, intent writes spaces.
- A backup-shaped mirror name (`*.bak*`, `*~`, `*.tmp`) fails intent load by design: package data with a stray is a packaging bug, not a file to skip.
- CO `Some(0)` emits `0x00000000` to clear the firmware offset; only `None` leaves it untouched.
- Schema 3 refuses v2 intents (no `prune`, unknown fields denied) — pruning belongs to the manifest.
- `[omit]` tombstones reach only the layers below and must match something there; a base tombstone is a load error.
- `[requires]` mismatch is `PROFILE` drift (exit 1); a missing managed target is an `ANOMALY` that refuses the whole apply first (exit 3, nothing written).
- An empty `cpus` array is a frequency band; an unparseable list is refused — never read a typo as a band.
- `system-sleep/` executes every file in the directory: stage temps in sibling `system-sleep-backups/`.
- A rehearsal root (`GENARCH_SETTINGS_ROOT`) runs no external tools and truncates writes; command-backed rows report `SKIPPED`.
- `mirror` and `managed` are disjoint; a mirror at the projection path is refused at load.
- `[units] enabled` is the unit list's only home, and completions come from the clap tree — installers keep no copies.
- `TIMER` drifts until the `genarch-settings` units are enabled; that is the expected pre-activation state.
- A failed pass write is a failure (exit 3), never a silent skip.

## Memory

Durable project memory — action logs, design and profile records, lessons, review history, settled and open decisions — lives in the maintainer's private warehouse (start at its `AGENTS.md`; its lessons index is the pitfall route). Search it by meaning before re-deriving anything. This repository carries the live rules; the warehouse carries the history and the evidence.
