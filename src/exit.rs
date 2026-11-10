//! Exit codes are the operator contract: they are what scripts, timers and the
//! runbook branch on. The mapping is inherited from the shell engine and must not
//! change while units depend on it.
//!
//! 0  clean           — no DRIFT (rows may be SKIPPED where a check needs root)
//! 1  drift           — DRIFT present (report) or persisted after apply
//! 2  precondition    — bad argument, lock held, missing/invalid intent
//! 3  partial         — an apply step failed; applied/not-applied are reported

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Clean,
    Drift,
    Precondition,
    Partial,
}

impl Exit {
    pub fn code(self) -> i32 {
        match self {
            Exit::Clean => 0,
            Exit::Drift => 1,
            Exit::Precondition => 2,
            Exit::Partial => 3,
        }
    }
}

/// The error every command returns. `kind` decides the exit code; the message is
/// operator-facing and must name the target that failed.
#[derive(Debug)]
pub struct Failure {
    pub kind: Exit,
    pub message: String,
}

impl Failure {
    pub fn precondition(message: impl Into<String>) -> Self {
        Failure { kind: Exit::Precondition, message: message.into() }
    }

    pub fn partial(message: impl Into<String>) -> Self {
        Failure { kind: Exit::Partial, message: message.into() }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

pub type Result<T> = std::result::Result<T, Failure>;

/// Conversions from the crate-internal error types used by the modules.
impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Failure::partial(format!("i/o: {e}"))
    }
}
