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
trait with two adapters is what lets the 17 behavioural tests in `tests/converge.rs` assert
byte-level guarantees without a VM. **No module above `host` opens a file or spawns a process.**
If a future module needs to, the port gets a method — that is the cost of the seam and it is
worth paying.

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
now driven by the registry table, and the JSON document (`schema: 1`) is the machine seam that a
documentation gate reads instead of parsing prose.

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

## Open

- License and repository metadata (owner decision).
- The cross-repo contract file: check IDs + doc anchors, owned by `system-tuning` (which keeps the
  ledger) and pinned by a copy here, so a drifted anchor is red on both sides.
- Whether `hw-tune.fish` should be pruned at cutover (it is imported for byte-parity today; the
  engine replaces it, so the cutover intent drops it and `apply` prunes it).
