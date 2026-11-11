//! The external seam: four verbs, one machine-readable flag, and exit codes that
//! operators and systemd both depend on. The help text is contract — the shell
//! engine's replacement must not silently change what a verb means.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "genarch",
    about = "Converge one Arch host to a declared intent, and judge whether it holds.",
    long_about = "genarch is the tuning engine for this host. `report` judges the live system \
                  read-only; `apply` converges it to the shipped intent; `harvest` proposes the \
                  intent change that would match live; `converge` runs the scheduled passes that \
                  systemd and udev trigger.\n\nExit codes: 0 clean (rows may be SKIPPED where a \
                  check needs root), 1 drift present or persisted after apply, 2 precondition \
                  failure, 3 apply step failed (partial).",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub verb: Verb,
}

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// Judge the live system against intent. Read-only; runs unprivileged.
    Report {
        /// Emit the stable machine-readable schema instead of the operator table.
        #[arg(long)]
        json: bool,
    },
    /// Converge the live system to intent. Needs root; idempotent.
    Apply {
        /// Print the plan and write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the intent change that would match live. Writes nothing.
    Harvest,
    /// Print the units the intent expects enabled, one per line.
    ///
    /// The install scripts call this instead of restating the list: a unit renamed in the
    /// intent must not need a second edit in `genarch.install`.
    Units,
    /// Run a scheduled pass. Called by systemd units and udev rules, not by hand.
    Converge {
        #[arg(value_enum)]
        pass: PassArg,
    },
    /// Derive a machine profile from a legacy `state/` tree (migration only).
    Import {
        /// The `state/` directory of the shell engine.
        state: PathBuf,
        /// The profile directory to write, e.g. `intent/profiles/zenbook-s16`.
        profile_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PassArg {
    /// Boot pass: SMU and firmware-adjacent settings, then a convergence re-apply.
    Boot,
    /// Resume pass: single pass, no sleep — a sleep hook blocks the user.slice thaw.
    Resume,
    /// Ceiling watch: drift-only, touches only `scaling_max_freq`, no SMU writes.
    Ceil,
}

impl PassArg {
    pub fn as_pass(self) -> crate::pass::Pass {
        match self {
            PassArg::Boot => crate::pass::Pass::Boot,
            PassArg::Resume => crate::pass::Pass::Resume,
            PassArg::Ceil => crate::pass::Pass::Ceil,
        }
    }
}
