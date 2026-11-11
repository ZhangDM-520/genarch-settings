//! Report formatting and exit-code derivation.
//!
//! Two renderings of one verdict set: the operator table and a stable JSON document. The
//! JSON is the machine seam — a documentation gate or a monitoring probe reads it and does
//! not have to parse prose.
//!
//! Exit code: DRIFT means 1, everything else is 0. `WARN` and `SKIPPED` are deliberately not
//! failures: a warning is a known quirk, and a skipped row means "unverified here", which is
//! not the same as "wrong". Saying otherwise trains the operator to ignore the exit code.

use crate::check::{self, Class, Reach, Verdict};
use crate::exit::Exit;

pub fn exit_code(verdicts: &[Verdict]) -> Exit {
    if verdicts.iter().any(|v| v.class == Class::Drift) {
        Exit::Drift
    } else {
        Exit::Clean
    }
}

pub fn table(verdicts: &[Verdict]) -> String {
    let mut out = String::new();
    for verdict in verdicts {
        let doc = check::REGISTRY
            .iter()
            .find(|c| c.id == verdict.id)
            .map(|c| c.doc)
            .unwrap_or("");
        if doc.is_empty() {
            out.push_str(&format!("{} {} {}\n", verdict.class.label(), verdict.id, verdict.detail));
        } else {
            out.push_str(&format!(
                "{} {} {} {}\n",
                verdict.class.label(),
                verdict.id,
                verdict.detail,
                doc
            ));
        }
    }
    out.push_str(&summary_line(verdicts));
    out
}

fn count(verdicts: &[Verdict], class: Class) -> usize {
    verdicts.iter().filter(|v| v.class == class).count()
}

pub fn summary_line(verdicts: &[Verdict]) -> String {
    format!(
        "SUMMARY ok={} accepted={} warn={} drift={} skipped={} exit={}",
        count(verdicts, Class::Ok),
        count(verdicts, Class::Accepted),
        count(verdicts, Class::Warn),
        count(verdicts, Class::Drift),
        count(verdicts, Class::Skipped),
        exit_code(verdicts).code()
    )
}

/// Which rows `apply` can reach, as one line. `apply` exiting 0 never implies the report is
/// green, so the reach is printed rather than implied.
pub fn scope_line() -> String {
    let mut full = Vec::new();
    let mut file_only = Vec::new();
    let mut intent = Vec::new();
    let mut none = Vec::new();
    for check in check::REGISTRY {
        match check.reach {
            Reach::Full => full.push(check.id),
            Reach::FileOnly => file_only.push(check.id),
            Reach::Intent => intent.push(check.id),
            Reach::None => none.push(check.id),
        }
    }
    let join = |ids: Vec<&str>| {
        if ids.is_empty() {
            "-".to_string()
        } else {
            ids.join(",")
        }
    };
    format!(
        "APPLY-SCOPE full={} file-only={} intent={} none={}",
        join(full),
        join(file_only),
        join(intent),
        join(none)
    )
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Stable schema: `{"schema":1,"checks":[{"id","class","detail","doc","reach"}],"summary":{…}}`.
pub fn json(verdicts: &[Verdict]) -> String {
    let mut checks = Vec::new();
    for verdict in verdicts {
        let entry = check::REGISTRY.iter().find(|c| c.id == verdict.id);
        let doc = entry.map(|c| c.doc).unwrap_or("");
        let reach = entry.map(|c| c.reach).unwrap_or(Reach::None);
        checks.push(format!(
            "{{\"id\":\"{}\",\"class\":\"{}\",\"detail\":\"{}\",\"doc\":\"{}\",\"reach\":\"{}\"}}",
            verdict.id,
            verdict.class.label(),
            json_escape(&verdict.detail),
            doc,
            reach.label()
        ));
    }
    format!(
        "{{\"schema\":1,\"checks\":[{}],\"summary\":{{\"ok\":{},\"accepted\":{},\"warn\":{},\"drift\":{},\"skipped\":{},\"exit\":{}}}}}",
        checks.join(","),
        count(verdicts, Class::Ok),
        count(verdicts, Class::Accepted),
        count(verdicts, Class::Warn),
        count(verdicts, Class::Drift),
        count(verdicts, Class::Skipped),
        exit_code(verdicts).code()
    )
}
