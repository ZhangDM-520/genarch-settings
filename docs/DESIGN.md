# DESIGN — the interface decisions behind the engine

This is the reasoning record. `CONTEXT.md` defines the words; this file says why the seams are
where they are.

## Why a rewrite at all

The shell engine converged this host correctly, but its correctness lived in conventions rather
than in a type system: globals shared between `lib/*.sh` sourced in a fixed order, a sysctl
comparison that silently truncated multi-word values, a class map duplicated between a reader and
a writer, and a report whose rows were string-matched by the tests. Every one of those was found
by an audit, not by the compiler. A native engine moves those properties into the type system and
the gate.

## The one seam that matters: `Host`

`trait Host` is the only way the engine touches the machine. Two adapters:

- `RealHost` — the live root, optionally aimed at a copy (`GENARCH_ROOT`) for a rehearsal.
- `FixtureHost` — a scratch tree; records writes, answers `run` from a canned table.

The shell engine's entire testability rested on one path-prefix hack (`ECO_ROOT`). Making it a
trait with two adapters is what lets the behavioural tests in `tests/converge.rs` assert
byte-level guarantees without a VM. **No module above `host` touches live host state directly**;
the manifest, the apply lock (`Host::lock`), pre-image bytes and modes (`read_bytes`,
`mode_of`) and every `/etc`, `/sys`, `/proc` read or write go through the trait. Two exceptions
are input data, not host state: `intent` reads its own package tree and `import` reads an offline
`state/` tree. If a future module needs live state, the port gets a method — that is the cost of
the seam and it is worth paying.

`ryzenadj` is a second, narrower seam: it is a true external binary, so the engine builds argv and
the fixture records it. That makes the packing arithmetic testable exhaustively (see
`ryzenadj::tests`) without an SMU.

## Why the intent is hybrid

Structured TOML for values the engine computes; byte-exact mirrors for files it must not
interpret. The alternative — one format for both — forces the engine to parse `fstab`, `tlp.conf`,
`asusd.ron` and a systemd unit, and each parser is a way to silently rewrite a vendor file's
meaning. The mirror keeps "deploy this exactly" as a primitive.

The cost is one asymmetry: a mirror file cannot be validated beyond "deployable name" and "live
byte-equal". That is accepted, and it is why the mirror is restricted to files whose content is
its own contract.

## Why sysctls are not a file

The shell engine shipped five `sysctl.d` files *and* re-asserted the values at runtime. That is
two homes for one value and an ordering rule (`10-` before `99-`) that silently decides
precedence. The engine applies values from one table directly to `/proc/sys`, so:

- there is one home for a value;
- ordering cannot hide anything;
- the comparison is whole-value (the multi-word `kernel.printk` bug becomes impossible);
- a rehearsal can read the same path from a fixture tree.

## Why four verbs

`report`, `apply`, `harvest`, `converge <pass>`. Fewer would overload `apply` with question-asking
behaviour; more would put engine internals on the operator's command line. `import` exists only
for the migration and is not part of the steady-state surface.

Exit codes are inherited unchanged (0 clean / 1 drift / 2 precondition / 3 partial) because they
are what timers, hooks and the runbook branch on. The *report surface* is free to change: it is
now driven by the registry table, and the JSON document (`schema: 2`, with a top-level `profile`
object) is the machine seam that a documentation gate reads instead of parsing prose.
`apply` inherits the drift rule too: a profile that does not match this machine exits 1 after
the apply, because the writes succeeded against another machine's facts.

## Why the passes are three and not one

- `boot` repeats the SMU pass after a settle delay so it lands after the platform-profile handlers
  that reset firmware limits on every power event.
- `resume` does the same work once and **never sleeps**: systemd thaws `user.slice` only after the
  sleep hook exits, so a sleep there stalls every resume (measured 5.1 s in the shell era).
- `ceil` is drift-only and touches `scaling_max_freq` alone; register churn is the wedge race the
  SMU paths stay away from.

Encoding the split in the interface means a unit cannot accidentally ask for the wrong one.

## Accepted trade-offs

- **A stray in the mirror is a hard failure**, not a skipped file. It is package data; a backup
  name there is a packaging bug that must be seen before a plan exists.
- **`GENARCH_ROOT` exists in the CLI** even though tests do not use it. It is the only way to
  rehearse a plan against a copy of the live tree, which the cutover depends on.
- **`CO` is judged from intent, not from silicon**: the SMU does not read a curve offset back.
  The row guards the thing a rewrite can lose (the offset disappearing), and says so.
- **No async runtime, no daemon.** Nothing needs it; a second caller would make the seam real,
  and there is no second caller.

## Write semantics (settled 2026-11-11)

Two corrections after the first review, both measured rather than reasoned:

- **Atomic where it is possible, in place where it is not.** `sysfs`/`procfs` refuse a sibling
  temp file (`touch /proc/sys/kernel/printk.tmp` → ENOENT; `.tmp` under a policy dir → EACCES),
  so `/sys` and `/proc` writes go straight to the existing attribute. An earlier uniform
  temp+rename made `converge ceil` and every runtime sysctl correction fail with exit 3 while
  the knob stayed wrong. Everything else is `create_new` + `O_NOFOLLOW` + fsync + rename.
- **The manifest owns what the engine writes.** `owned` is prune-eligible (mirrors + the sysctl
  projection); `touched` records the foreign files patched through a managed record and is never
  pruned. The projection used to be written but unowned, so emptying `sysctl.toml` left
  `/etc/sysctl.d/90-genarch.conf` live and unreported — a second home for every value.

## Write semantics, round 2 (settled 2026-11-11)

The first fix round cured the kernel-filesystem blocker but introduced three hazards of its own,
found in external review and fixed here:

- **Ownership travels with the file.** A root-written temp is `0:0`, so `rename` reset the owner of
  every user-owned target. The owner is now carried from the existing metadata with `fchown` on the
  open descriptor, and a root run fails rather than silently producing a root-owned file. (Measured
  live: two `~/.config/autostart` entries were root-owned after the cutover.) A **new** target, which
  has no metadata to inherit, takes the owner of the directory it lands in.
- **Descriptor, not path.** `fchmod`/`fchown` happen before `close`; a path-based `chmod` after
  close follows a symlink planted in a user-writable directory.
- **`system-sleep/` is never a staging area.** Its temp files go to `system-sleep-backups/`,
  because systemd-sleep executes every file in the directory, whatever its name.

Also in this round: kernel attributes use exactly one `write(2)`; a rehearsal root truncates;
`mirror` and `managed` are disjoint; the scheduled passes take the apply lock and log a skip
instead of interleaving with an apply.

## Layers and profiles (settled 2026-11-11)

The engine was generic and the shipped data was one laptop: `/etc/fstab` with this disk's UUIDs,
this user's autostart entries, this silicon's offsets, and — through absent fields — this SoC's
wake devices and Bluetooth ID compiled into the binary. A second AMD laptop was a fork.

- **Three layers, one intent.** Base (`/usr/lib/genarch`) carries only what is true of any host;
  the profile (`profiles/<name>`, selected by `profile = "<name>"` in `/etc/genarch/profile`)
  carries every measurement and identity; the operator (`/etc/genarch`) has the last word. Base
  and profile are themselves layered per class, so an operator file does not shadow a whole tree.
- **Per-class merge, not whole-file.** Scalar maps merge per key (`sysctl`, `ryzenadj`, `passes`);
  lists replace wholesale; mirrors and managed records are whole-file with an `[omit]` tombstone.
  Copying sixteen keys to change three was the interface that made a fork cheaper than a config.
- **A tombstone reaches only the layers below it.** `[omit]` in a layer removes a mirror or a
  managed record declared under it; a base tombstone that matched an operator file would make
  the base the last word. Every tombstone must name something below it, so a stale or
  mistaken one is a load error.
- **Units are the one union.** An "expected enabled" entry is an expectation a layer adds and
  cannot withdraw another's; replacing would let a profile silently drop `genarch.service` from
  the report. Every other class merges per key or replaces wholesale.
- **Empty is legal, and it is not a guess.** With no profile the intent is the base, the rows that
  need a machine fact say SKIPPED, and no default is invented. A profile declares `[requires]`
  (DMI product/vendor); a mismatch is `PROFILE` drift (exit 1), not a refusal to load.
- **Ownership outlives selection.** A deselected profile's mirrors stay in the manifest as dormant
  ownership, because a lost selection must not prune `/etc/fstab`. Deleting the profile does hand
  its files back.
- **A ceiling band is a SKU clock, not a core class.** A documented profile cannot know CPU
  numbering, so a band class applies by `cpuinfo_max_freq` — the highest declared clock the unit
  admits. Several bands per profile are therefore normal: one DMI product string routinely
  covers SKUs 200–400 MHz apart, and declaring each clock lets one profile serve all of them
  without capping a fast unit or writing a value above a slow one's admitted maximum.
- **Ceilings and pass literals are data.** `ceil.toml` maps CPU sets to kHz (reader and writer share
  it), and `passes.toml` carries the wake list, USB ID, interface and settle delay. The code
  defaults are gone; there is nothing left to inherit silently.
- **`tree` breaks the intent↔converge cycle.** Walking, the deployable-name rule and the live-path
  mapping live there, so `intent` no longer calls into `converge` and the importer stops carrying a
  second walker.

## Open

- **Per-MTM machine identity.** `[requires]` is exact equality, which is the right default: a
  near-miss must not apply another machine's facts. But some vendors put the sales code in
  `product_name` — a Lenovo ThinkPad Z13 Gen 2 reports `21JVCTO1WW`, and PSREF lists 1,276 MTMs
  across two CPUs for one model name. Exact matching cannot express "any 21JV unit", so that
  platform is deliberately uncovered rather than covered by two arbitrary MTM profiles. The
  options are per-MTM profiles (correct, narrow) or a declared prefix mode with its own
  precedence rule; the latter is a matching-mode change and has not been decided.
- License and repository metadata (owner decision).

## Closed

- **The cross-repo contract** (2026-11-11): `system-tuning/docs/contract.tsv` is the owner copy,
  `tests/contract.tsv` is the pinned copy, and `contract.rs` asserts three things — the pinned
  file equals the compiled registry, it equals the owner copy, and every anchor resolves to a
  real `##` heading in `findings.md`. A missing ledger is a failure, not a skip.
