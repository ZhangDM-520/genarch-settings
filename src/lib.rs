//! genarch: the tuning engine for one Arch host.
//!
//! Layering, outermost first:
//!
//! - [`cli`] parses the four verbs and maps outcomes to exit codes.
//! - [`check`] holds the registry and one check per knob; [`report`] formats verdicts.
//! - [`intent`] resolves the layers into one declared state; [`converge`] plans and applies
//!   it; [`managed`] patches single lines in foreign files; [`import`] derives a machine
//!   profile from a legacy `state/` tree; [`tree`] owns what may ship.
//! - [`host`] is the seam to the machine: the only module that touches the filesystem,
//!   the process table or systemd, and the only one tests substitute.
//!
//! Nothing above [`host`] is allowed to open a file or run a process directly. That is
//! what makes the whole engine testable against a fixture tree.

pub mod ceil;
pub mod clock;
pub mod check;
pub mod cli;
pub mod converge;
pub mod exit;
pub mod host;
pub mod import;
pub mod intent;
pub mod managed;
pub mod pass;
pub mod report;
pub mod ryzenadj;
pub mod tree;

pub use exit::{Exit, Failure, Result};
