//! The external seam: the verbs, one machine-readable flag, and exit codes that operators and
//! systemd both depend on. The help text is contract — the shell engine's replacement must not
//! silently change what a verb means.

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

/// The installed binary name. One home: the clap name, the error prefix and the completion
/// scripts all read it, and `tests/cli.rs` fails if they disagree.
pub const BINARY: &str = "genarch-settings";

#[derive(Debug, Parser)]
#[command(
    name = BINARY,
    about = "Converge one Arch host to a declared intent, and judge whether it holds.",
    long_about = "genarch-settings is the tuning engine for this host. `report` judges the live system \
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
    /// intent must not need a second edit in `genarch-settings.install`.
    Units,
    /// Run a scheduled pass. Called by systemd units and udev rules, not by hand.
    Converge {
        #[arg(value_enum)]
        pass: PassArg,
    },
    /// Print a completion script for one shell on stdout.
    ///
    /// The package and `install.sh` write this to the shell's standard completion directory,
    /// so a script can never describe verbs or flags the binary does not have.
    Completions {
        /// The shell to generate for.
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Derive a machine profile from a legacy `state/` tree (migration only).
    Import {
        /// The `state/` directory of the shell engine.
        state: PathBuf,
        /// The profile directory to write, e.g. `intent/profiles/zenbook-s16`.
        profile_dir: PathBuf,
    },
}

/// The shells a completion script is generated for. Exactly the three this host's tools use;
/// a fourth is a deliberate change, not a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

impl From<CompletionShell> for clap_complete::Shell {
    fn from(shell: CompletionShell) -> Self {
        match shell {
            CompletionShell::Bash => clap_complete::Shell::Bash,
            CompletionShell::Zsh => clap_complete::Shell::Zsh,
            CompletionShell::Fish => clap_complete::Shell::Fish,
        }
    }
}

/// The completion script itself. Generated from the live command tree, never hand-written:
/// that is the whole point of a verb rather than a static file in the package.
pub fn completions(shell: CompletionShell) -> String {
    let mut command = Cli::command();
    let mut out = Vec::new();
    let generator: clap_complete::Shell = shell.into();
    clap_complete::generate(generator, &mut command, BINARY, &mut out);
    String::from_utf8(out).expect("a completion script is UTF-8")
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
