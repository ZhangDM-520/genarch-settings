# Scope

One host's tuning configuration, and the engine that applies it.

- **In scope** — the engine (`src/`), the declared state (`intent/`), and the laptop model's
  profile (`intent/profiles/zenbook-s16/`).
- **Out of scope** — the ledger of *why* a knob has its value
  (`system-tuning/docs/findings.md`), the benchmarks, and any second engine writing these paths.
  One path has exactly one owner; that is the rule the whole design exists to enforce.
- **Second machine** — a new directory under `profiles/` plus one line in `/etc/genarch-settings/profile`.
  If the base intent or the engine has to change for it, that is a bug in this scope, not a fork.
- **Not a distribution** — the package is built from this tree and ships the model's profile
  (not one unit's identity). It selects nothing on a host whose DMI does not match (`PROFILE`
  drift, exit 1).
