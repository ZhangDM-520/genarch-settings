//! The line-patch engine: own exactly one line inside a file we do not own.
//!
//! This is the module that lets the engine manage `/etc/asusd/asusd.ron`, a vendor systemd
//! unit or `journald.conf` without taking them over. Its contract, all of it paid for:
//!
//! - **The anchor is the identity.** A record matches by pattern, not by line number, so a
//!   package update that adds lines above it cannot make the engine edit the wrong thing.
//! - **Zero matches and several matches are hard errors**, and nothing is written. Guessing
//!   which of two candidates was meant is how a config silently becomes wrong.
//! - **Every other byte survives.** Only the matched line is replaced; the file is rebuilt
//!   with the original line endings and a trailing newline preserved exactly.
//! - **The write is atomic** (sibling temp + rename), so a crash cannot leave a half file.
//!
//! Section scoping exists because `journald.conf`-shaped files repeat keys per section; a
//! bare pattern there would match the wrong one.

use regex::Regex;

use crate::exit::{Failure, Result};
use crate::host::Host;
use crate::intent::ManagedRecord;

/// What a probe found. `Differs` is the only state `apply` acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// The single anchor already holds the intended line.
    Ok,
    /// The single anchor holds something else (the case `apply` converges).
    Differs { current: String },
    /// No line matched the anchor.
    NoMatch,
    /// More than one line matched: the record is ambiguous, which is an error, not a choice.
    MultiMatch { count: usize },
    /// The target file does not exist at all.
    MissingTarget,
}

impl State {
    pub fn is_error(&self) -> bool {
        matches!(self, State::NoMatch | State::MultiMatch { .. } | State::MissingTarget)
    }
}

/// A probe result with the facts a caller needs to report it.
#[derive(Debug, Clone)]
pub struct Probe {
    pub state: State,
    /// 1-based line number of the single match, when there is exactly one.
    pub line_no: Option<usize>,
    pub message: String,
}

pub fn probe(host: &dyn Host, record: &ManagedRecord) -> Result<Probe> {
    if !host.exists(&record.target) {
        return Ok(Probe {
            state: State::MissingTarget,
            line_no: None,
            message: format!("{}: target {} does not exist", record.name, record.target),
        });
    }
    let text = host.read(&record.target)?;
    let lines: Vec<&str> = text.lines().collect();
    let re = compile(&record.match_expr, &record.name)?;

    let (start, end) = match &record.section {
        Some(section) => match section_region(&lines, section) {
            Some(range) => range,
            None => {
                return Ok(Probe {
                    state: State::NoMatch,
                    line_no: None,
                    message: format!(
                        "{}: section [{section}] not found in {}",
                        record.name, record.target
                    ),
                });
            }
        },
        None => (0, lines.len()),
    };

    let hits: Vec<usize> = (start..end).filter(|i| re.is_match(lines[*i])).collect();
    match hits.len() {
        0 => Ok(Probe {
            state: State::NoMatch,
            line_no: None,
            message: format!(
                "{}: 0 lines match anchor in {}{}",
                record.name,
                record.target,
                record
                    .section
                    .as_ref()
                    .map(|s| format!(" section [{s}]"))
                    .unwrap_or_default()
            ),
        }),
        1 => {
            let idx = hits[0];
            let current = lines[idx].to_string();
            let state = if current == record.line {
                State::Ok
            } else {
                State::Differs { current: current.clone() }
            };
            Ok(Probe {
                state,
                line_no: Some(idx + 1),
                message: format!("{}: {} line {}", record.name, record.target, idx + 1),
            })
        }
        n => Ok(Probe {
            state: State::MultiMatch { count: n },
            line_no: None,
            message: format!(
                "{}: {n} lines match anchor in {} — refusing to choose",
                record.name, record.target
            ),
        }),
    }
}

/// Replace the anchored line, preserving everything around it.
///
/// The caller must have probed first; `apply` never patches what it has not classified.
pub fn replace(host: &dyn Host, record: &ManagedRecord) -> Result<()> {
    let probe = probe(host, record)?;
    let line_no = match (&probe.state, probe.line_no) {
        (State::Differs { .. }, Some(n)) => n,
        (State::Ok, _) => return Ok(()),
        _ => {
            return Err(Failure::partial(format!(
                "{}: refusing to patch ({})",
                record.name, probe.message
            )));
        }
    };

    let text = host.read(&record.target)?;
    let had_trailing_newline = text.ends_with('\n');
    // `lines()` drops the terminators, so rebuilding joins with `\n` and re-adds the
    // trailing newline only when the original had one.
    let mut lines: Vec<&str> = text.lines().collect();
    lines[line_no - 1] = record.line.as_str();
    let mut rebuilt = lines.join("\n");
    if had_trailing_newline {
        rebuilt.push('\n');
    }
    host.write(&record.target, rebuilt.as_bytes(), None)
}

fn compile(pattern: &str, name: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|e| {
        Failure::precondition(format!("managed '{name}': bad anchor pattern {pattern:?}: {e}"))
    })
}

/// Half-open line range of a `[section]` block.
///
/// A section runs from its header to the next header (or EOF). Header matching is exact on
/// the bracketed name, so `[Journal]` cannot be confused with `[JournalFoo]`.
fn section_region(lines: &[&str], section: &str) -> Option<(usize, usize)> {
    let header = format!("[{section}]");
    let start = lines
        .iter()
        .position(|line| line.trim() == header)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            let t = line.trim();
            t.starts_with('[') && t.ends_with(']')
        })
        .map(|(i, _)| i)
        .unwrap_or(lines.len());
    Some((start + 1, end))
}
