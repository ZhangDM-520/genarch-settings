# AGENTS.md — genarch-settings (native tuning engine)

Last reviewed: 2026-11-10.

## Project overview

The tuning engine for one Arch host (ASUS Zenbook S 16 UM5606WA, Ryzen AI 9 HX 370, CachyOS).
One Rust binary (`genarch`) carries the declared intent, the read-only judge, the converge pass,
the ryzenadj settings and the systemd units. It replaces the shell `eco` stack that lived in
`~/Projects/system-tuning` (the ledger, the runbook and the benchmarks stay there).

Vocabulary: `CONTEXT.md`. Interface reasoning: `docs/DESIGN.md`. Knob judgement and evidence live
in `system-tuning/docs/findings.md` — this repository does not restate a verdict, and it does not
restate a knob value in prose.

## Commands

| Command | Effect |
|---|---|
| `cargo test` | the gate: unit tests + `tests/converge.rs` (behavioural, fixture-hosted) |
| `cargo build --release` | the binary |
| `makepkg -f` | the package (binary + intent + units + udev rules) |
| `./target/debug/genarch report [--json]` | judge live against intent; unprivileged |
| `sudo ./target/debug/genarch apply [--dry-run]` | converge live; `--dry-run` writes nothing |
| `./target/debug/genarch harvest` | the intent change that would match live |
| `sudo ./target/debug/genarch converge <boot\|resume\|ceil>` | the scheduled passes |

Rehearsal without touching the machine: `GENARCH_ROOT=/path/to/copy` aims every live path at a
copy of the tree; `GENARCH_INTENT=/path/to/intent` swaps the intent. Both are read-only unless
you run `apply` without `--dry-run`.

There is no lint config; `cargo clippy` is ad hoc.

## Architecture

1. **`host` is the only seam.** `trait Host` + `RealHost` / `FixtureHost`. No module above it may
   open a file or spawn a process *for live host state* — every read/write of `/etc`, `/sys`,
   `/proc`, the manifest and the apply lock goes through it, and the lock is `host.lock()` rather
   than a `flock` call in `converge`. Two deliberate exceptions, both **input data** rather than
   host state: `intent` reads its own package tree (and `GENARCH_INTENT`'s rehearsal copy), and
   `import` reads an offline `system-tuning/state` tree; both use `std::fs` directly. A new need
   on live state still goes on the trait.
2. **`intent` is the declared state** — typed TOML (`sysctl.toml`, `ryzenadj.toml`, `passes.toml`),
   byte-exact `mirror/` files, and `managed/*.toml` anchored-line records. `/etc/genarch` overrides
   `/usr/lib/genarch` whole-file; nothing merges.
3. **`converge` is plan then apply.** `plan` writes nothing and needs no privilege; `apply`
   performs exactly the plan, after a pre-image for every target, under an exclusive lock, then
   re-plans and fails with exit 1 if anything survived.
4. **`managed` owns one line** inside a foreign file, identified by an anchor pattern. Zero or
   several matches are hard errors and write nothing.
5. **`check` + `report`** — one registry table (`check::REGISTRY`) drives report order, doc
   anchors, apply reach and the JSON schema. Three rows (MIRROR, MANAGED, SYNC) are computed from
   the same `converge::plan` that `apply` executes, so "apply said ok but live is wrong" is
   structurally impossible.
6. **`ceil`, `ryzenadj`, `pass`** — the ceiling class map has one home shared by reader and writer;
   ryzenadj is argv-building only (packing arithmetic tested); `pass` dispatches boot/resume/ceil.
7. **`import`** derives the intent from the shell engine's `state/` tree. Its acceptance is a
   round-trip property, not a review: import → load → render must reproduce the source bytes.

## Conventions

- **One home per fact.** A knob value lives once in the intent; the ledger records why. A rule
  (deployable names, sysctl normalisation, ceiling classes) lives in exactly one function.
- **Pre-images before every write**, dated, never deleted, **byte-exact and mode-preserving**:
  the copy is `read_bytes` + the source's permission bits. A 0644 copy of a 0755 hook restores a
  non-executable hook (203/EXEC, or silently skipped in `system-sleep/`).
- **Kernel attributes are written in place.** `sysfs` and `procfs` cannot host a sibling temp file
  or a rename, so `/sys` and `/proc` writes bypass the atomic path; everything else is
  temp + fsync + rename with `O_EXCL|O_NOFOLLOW` on the temp name.
- **The manifest is the ownership record** (`owned` = prune-eligible, `touched` = foreign files
  patched and never pruned). The sysctl projection is an engine-written file and therefore lives
  in `owned`; a target that leaves the intent is pruned from the record, so no value keeps a
  second home.
- **The manifest is the ownership record.** A target that leaves the intent is pruned by the
  engine; nothing is deleted on a hunch.
- **Every live mutation says which pass it belongs to.** SMU writes are boot/resume only; the
  ceiling watch is drift-only.
- Commit messages carry the check ID or knob when they change one, plus the
  `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>` trailer.

## Facts that are easy to get wrong

| Fact | How to check / why it matters |
| --- | --- |
| A dotted key in `sysctl.toml` **must be quoted** (`"kernel.printk" = "3 3 3 3"`); bare `kernel.printk = …` parses as a table and silently changes the name | The loader rejects a nested table with that hint. Found while writing the importer, which now quotes every dotted key. |
| sysctl comparison is whitespace-normalised (`converge::norm_sysctl`): `/proc/sys` renders tabs, the intent writes spaces | Comparing raw strings made the shell engine re-assert forever and exit 1 on a converged host. `sysctl_multi_value_is_compared_whole` is the guard. |
| A mirror entry whose name looks like a backup (`*.bak*`, `*~`, `*.orig`, `*.tmp`, `.#*`) makes the **intent fail to load** | It is package data: the failure must be seen before a plan exists, not skipped at deploy time. One home: `converge::is_deployable_name`. |
| `hw-tune.fish`'s CO writes embed fish command substitution (`(printf …)`); they are derived from the CO knobs and must not be carried as raw argv | The importer skips `--set-coall`/`--set-coper` lines and any token with a shell construct; the round-trip test asserts the raw list stays empty. |
| Mode is preserved on an existing target and taken from the intent for a new one | An exec-bit regression is invisible until a unit fails with 203/EXEC. |
| `resume` must never sleep | A sleep hook blocks the `user.slice` thaw; measured 5.1 s per resume in the shell era. |
| The report's `TIMER` row is expected to DRIFT until this package's units are enabled | It is the only row that is red on a fully converged host running the shell engine. |
| `apply` cannot delete a file it does not own; it prunes only what the manifest lists | That is why unmanaging something is safe, and why a second engine must never own the same path. A managed target goes in `touched`, never `owned`: removing the record must not delete the package's file. |
| `/sys` and `/proc` cannot host a temp file, so kernel attributes are written **in place** | Measured 2026-11-11: `touch /proc/sys/kernel/printk.tmp` → ENOENT, `.tmp` under a policy dir → EACCES, and the uniform temp+rename path made `converge ceil` exit 3 with `scaling_max_freq` unchanged. `kernel_fs()` + `write_in_place()` are the one home; the fixture test simulates a directory that refuses new entries. |
| A pre-image copies **bytes and the source mode** | A 0644 copy of a 0755 hook restores a non-executable hook (203/EXEC, or silently skipped in `system-sleep/`). `pre_image_keeps_the_source_mode` is the guard; `Host::read_bytes` exists because a mirror may be opaque. |
| A refused write in a pass is a failure (exit 3), never `.is_ok()` | The `pass` code swallowed sysfs/procfs errors, so a pass exited 0 while the governor/wake/USB/sysctl value never moved. `a_refused_pass_write_is_a_failure_not_a_silent_skip` is the guard. |
