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
2. **`intent` is the declared state, resolved from layers** — typed TOML (`sysctl.toml`,
   `ryzenadj.toml`, `passes.toml`, `ceil.toml`), byte-exact `mirror/` files, and
   `managed/*.toml` anchored-line records. Layers, in precedence order: **base**
   (`/usr/lib/genarch`), **profile** (`{base,overlay}/profiles/<name>`, selected by
   `/etc/genarch/profile`), **operator** (`/etc/genarch`). Scalar maps merge per key, lists
   replace wholesale, mirrors and managed records are whole-file with an `[omit]` tombstone in
   a layer's `intent.toml`. No profile means base only: nothing is guessed, and the rows that
   need a machine fact report SKIPPED. `tree` owns the walk, the deployable-name rule and the
   live-path mapping for both the loader and the importer.
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
| A mirror entry whose name looks like a backup (`*.bak*`, `*~`, `*.orig`, `*.tmp`, `.#*`) makes the **intent fail to load** | It is package data: the failure must be seen before a plan exists, not skipped at deploy time. One home: `tree::is_deployable_name`. |
| `hw-tune.fish`'s CO writes embed fish command substitution (`(printf …)`); they are derived from the CO knobs and must not be carried as raw argv | The importer skips `--set-coall`/`--set-coper` lines and any token with a shell construct; the round-trip test asserts the raw list stays empty. |
| Mode is preserved on an existing target and taken from the intent for a new one | An exec-bit regression is invisible until a unit fails with 203/EXEC. |
| `resume` must never sleep | A sleep hook blocks the `user.slice` thaw; measured 5.1 s per resume in the shell era. |
| The report's `TIMER` row is expected to DRIFT until this package's units are enabled | It is the only row that is red on a fully converged host running the shell engine. |
| `apply` cannot delete a file it does not own; it prunes only what the manifest lists | That is why unmanaging something is safe, and why a second engine must never own the same path. A managed target goes in `touched`, never `owned`: removing the record must not delete the package's file. |
| `/sys` and `/proc` cannot host a temp file, so kernel attributes are written **in place** | Measured 2026-11-11: `touch /proc/sys/kernel/printk.tmp` → ENOENT, `.tmp` under a policy dir → EACCES, and the uniform temp+rename path made `converge ceil` exit 3 with `scaling_max_freq` unchanged. `kernel_fs()` + `write_in_place()` are the one home; the fixture test simulates a directory that refuses new entries. |
| A pre-image copies **bytes and the source mode** | A 0644 copy of a 0755 hook restores a non-executable hook (203/EXEC, or silently skipped in `system-sleep/`). `pre_image_keeps_the_source_mode` is the guard; `Host::read_bytes` exists because a mirror may be opaque. |
| A root write must carry over **ownership**, not just mode | The temp is created 0:0, so `rename` made user files root-owned — measured live: `~/.config/autostart/org.fcitx.Fcitx5.desktop` and `org.kde.kdeconnect.daemon.desktop` were `0:0` after the cutover apply, stripping the user's write access. `RealHost::write` now `fchown`s the temp from the existing metadata before the rename (a **new** target inherits the owner of the directory it lands in), and a root run fails rather than silently leaving root-owned files. |
| Mode and ownership are set on the **file descriptor**, never the path | A path-based `chmod` after `close` follows a symlink: an unprivileged actor who wins the race in a user-writable directory turns a root `chmod` into `chmod` of an arbitrary file. `fchmod`/`fchown` on the open fd, then `fsync`, then `rename`. |
| Nothing transient may sit in `/usr/lib/systemd/system-sleep/` | systemd-sleep executes every file there, whatever its name. Writes to that directory stage their temp in the sibling `system-sleep-backups/` (same filesystem, so the rename stays atomic); a crash can no longer leave an executable `.tmp` the next resume would run. `staging_live()` is the one home. |
| A kernel attribute is written with **one** `write(2)`, never `write_all` | `sysfs` `store()` runs once per syscall and is not a stream: a short write followed by the remainder becomes a second, unrelated value (EINVAL or a malformed setting). `write_in_place` issues one `write` and errors on a short count. |
| `GENARCH_ROOT` rehearsals truncate kernel-attribute files | A rehearsal root is a normal filesystem; without `O_TRUNC` a shorter value left the old tail behind. Real `/sys` and `/proc` refuse `O_TRUNC`, so the flag is conditional on the root being `/`. |
| One path, one owner: `mirror` and `managed` are disjoint | Overlap made a whole-file write fight a line patch on the same bytes. `Intent::validate` refuses it. Several managed records on one target stay legal — that is how `asusd.ron` is patched at five anchors. |
| A **deselected** profile's mirrors stay owned (dormant) and are never pruned | With no profile selected the intent stops shipping `/etc/fstab`, and "no longer shipped" would prune it — deleting a system file because a config line went missing. `Intent::dormant` keeps them in `owned_targets`; only a profile that stops shipping a path hands it back. |
| A profile's `[requires]` mismatch is **PROFILE drift (exit 1)**, not a load error | The operator must be able to see the drift and fix the selection; `converge` prints the mismatch beside the plan and re-shows it after the apply. `genarch apply` has no `--force`. |
| A v2 `intent.toml` is refused, not silently stripped of its `prune` list | Schema 3 removed the declared removal list (the manifest owns pruning). `Meta` denies unknown fields, so an old intent fails loudly instead of applying without its removals. |
| Scheduled passes take the apply lock | The 2-minute ceil timer and an operator's `apply` both write kernel attributes; without the lock the last writer wins silently. A held lock makes the pass log `pass skipped` and return clean — the timer runs again in two minutes. |
| A refused write in a pass is a failure (exit 3), never `.is_ok()` | The `pass` code swallowed sysfs/procfs errors, so a pass exited 0 while the governor/wake/USB/sysctl value never moved. `a_refused_pass_write_is_a_failure_not_a_silent_skip` is the guard. |
