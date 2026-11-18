# genarch-settings — Copilot instructions

Single Rust binary converging one Arch host to a declared, layered **intent** (`report` read-only, `apply` plan-then-apply, `converge <boot|resume|ceil>` scheduled passes). Ships the engine plus the documented model profiles (`zenbook-s16` is the measured one); knob evidence and judgement live in the separate `system-tuning` repository.

## Build, test, lint

```sh
cargo test                    # unit + converge/layers/cli/profiles + cross-repo contract
cargo build --release --locked
cargo clippy --all-targets    # ad hoc; warnings worth fixing
cargo test --test <converge|layers|profiles|cli> [test_name]
cargo test --lib <test_name>
```

- **No CI by owner decision**: run `cargo test` locally before committing. Behavioural tests use `FixtureHost` over scratch trees — never run tests as root or debug by mutating live host state (`apply --dry-run`, or `GENARCH_SETTINGS_ROOT=/path GENARCH_SETTINGS_INTENT=/path` for an unprivileged rehearsal).
- **Cross-repo contract (`tests/contract.rs`)**: reads `SYSTEM_TUNING_ROOT` (default: a `system-tuning` sibling checkout under `$HOME/Projects`) and fails if absent, or if `check::REGISTRY`, `tests/contract.tsv` and `system-tuning/docs/contract.tsv` diverge or miss a `##` heading in `findings.md`. Edit all three together when adding or renaming a check.
- **Live & packaging commands**: `./target/debug/genarch-settings {report [--json],harvest,units,completions <shell>,import <state> <profile-dir>}`, `sudo ./target/debug/genarch-settings {apply [--dry-run],converge <boot|resume|ceil>}`, `makepkg -f` (ships the measured profile's selection), `sudo ./install.sh` (portable; installs all profiles, selects none unless `--profile`).

## Architecture

1. **`host` seam (`trait Host`: `RealHost` / `FixtureHost`)**: Every live `/etc`, `/sys`, `/proc`, manifest, process (`Host::run`), and lock (`host.lock()`) access goes through `Host`. Exceptions are offline input reads (`intent` package tree, `import` legacy `state/`).
2. **`intent` layers (`base → profile → operator`)**: `intent.toml` + typed TOML (`sysctl.toml`, `ryzenadj.toml`, `passes.toml`, `ceil.toml`), byte-exact `mirror/`, and anchored `managed/*.toml` under `/usr/lib/genarch-settings`, `profiles/<name>` (from `/etc/genarch-settings/profile`), and `/etc/genarch-settings`. Scalars merge per key; lists replace (`[units]` unions); `[omit]` tombstones reach only lower layers; schema is 3 (v2 refused).
3. **`converge` & `managed`**: `plan` is unprivileged/read-only; `apply` snapshots byte+mode pre-images under an exclusive lock, executes the plan, re-plans, and exits `1` on remaining drift (`--dry-run` exits `0`). `managed` patches one anchored line (0 or >1 matches abort without writing).
4. **`check::REGISTRY` & `report`**: Single table driving order, JSON schema 2, doc anchors, and apply reach (`PROFILE`, `MIRROR`, `MANAGED`, `SYNC`, `PRUNE` share `converge::plan`; `Reach::Intent` rows `CO`/`POWER` report declared state only).
5. **`tree` & `ceil`**: `tree` breaks `intent↔converge` cycles; `ceil` shares one band/CPU map across reader and writer.
6. **`ryzenadj`, `pass` & `import`**: `ryzenadj` builds argv only; `pass` writes scheduled sysfs/SMU values under lock; `import` guarantees `import → load → render` byte round-trip.
7. **Exit codes (operator contract)**: `0` clean, `1` drift, `2` precondition, `3` partial (`plan.anomalies` refuses before `apply` writes anything).

## Conventions and hazards

- **No host identity in shipped profiles (`tests/profiles.rs`)**: `intent/` never names a home directory, a disk UUID, a partition device or a user/checkout path (`import` finds `hw-tune.fish` by filename), and `intent/profiles/*` may ship only model facts.
- **Pre-images before every write**: dated `<file>.bak-YYYYMMDD`, byte-exact, owner and mode carried on the open fd, one per target per day, never deleted.
- **Kernel attributes are written in place**: `/sys` and `/proc` refuse a sibling temp, and sysfs parses one `write(2)` syscall — no temp+rename, no `write_all`.
- **Ask before committing or pushing**; a commit subject names the check ID or the knob, and the trailers live in `CONTRIBUTING.md`.

Vocabulary is `CONTEXT.md`; interface reasoning `docs/DESIGN.md`; what a profile may ship, and where a unit-specific fact belongs instead, is `docs/PROFILES.md`.
