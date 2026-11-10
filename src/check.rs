//! Checks: one verdict per knob, and a registry that is the single list of them.
//!
//! The registry is one table. Report order, the doc anchor each row points at, what `apply`
//! can reach, and the machine-readable IDs all derive from it, so a check cannot be added in
//! one place and forgotten in another — that drift is what the shell engine's audit waves
//! spent a day fixing.
//!
//! Three rows (MIRROR, MANAGED, SYNC) are computed from the *same* [`crate::converge::plan`]
//! that `apply` executes. That is deliberate: it makes "apply reported success but the host
//! is still wrong" structurally impossible rather than a promise.

use crate::ceil;
use crate::converge;
use crate::exit::Result;
use crate::host::Host;
use crate::intent::Intent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Judgeable and correct.
    Ok,
    /// A settled decision where live legitimately differs from the literal value.
    Accepted,
    /// A known quirk or an expected transient; not actionable.
    Warn,
    /// An actionable mismatch.
    Drift,
    /// Not verifiable here (needs root, a device, or a user session).
    Skipped,
}

impl Class {
    pub fn label(self) -> &'static str {
        match self {
            Class::Ok => "OK",
            Class::Accepted => "ACCEPTED",
            Class::Warn => "WARN",
            Class::Drift => "DRIFT",
            Class::Skipped => "SKIPPED",
        }
    }
}

/// How far `apply` reaches into a row. Printed so "apply exited 0" is never read as
/// "the report is green".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// apply's own writes can turn the row green.
    Full,
    /// apply owns the file half; the live effect has another owner.
    FileOnly,
    /// apply cannot reach the row at all.
    None,
}

impl Reach {
    pub fn label(self) -> &'static str {
        match self {
            Reach::Full => "full",
            Reach::FileOnly => "file-only",
            Reach::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Verdict {
    pub id: &'static str,
    pub class: Class,
    pub detail: String,
}

pub struct Ctx<'a> {
    pub host: &'a dyn Host,
    pub intent: &'a Intent,
}

pub type CheckFn = fn(&Ctx<'_>) -> Result<Verdict>;

pub struct Check {
    pub id: &'static str,
    /// Doc anchor for the ledger entry, as `findings#ceil`.
    pub doc: &'static str,
    pub reach: Reach,
    pub run: CheckFn,
}

macro_rules! check {
    ($id:literal, $doc:literal, $reach:ident, $func:path) => {
        Check { id: $id, doc: $doc, reach: Reach::$reach, run: $func }
    };
}

/// The registry. One row per knob; order is report order.
pub const REGISTRY: &[Check] = &[
    check!("MIRROR", "findings#intent", Full, check_mirror),
    check!("MANAGED", "findings#managed", Full, check_managed),
    check!("SYNC", "findings#writeback", Full, check_sync),
    check!("CEIL", "findings#ceil", None, check_ceil),
    check!("CO", "findings#co", None, check_co),
    check!("SMU", "findings#ppt", None, check_smu),
    check!("POWER", "findings#eco", None, check_power),
    check!("WIFI", "findings#wifi", FileOnly, check_wifi),
    check!("IO", "findings#writeback", Full, check_io),
    check!("TIMER", "findings#timer", None, check_timer),
    check!("PRUNE", "findings#intent", Full, check_prune),
];

pub fn ids() -> Vec<&'static str> {
    REGISTRY.iter().map(|c| c.id).collect()
}

pub fn evaluate(ctx: &Ctx<'_>) -> Vec<Verdict> {
    REGISTRY
        .iter()
        .map(|check| {
            (check.run)(ctx).unwrap_or(Verdict {
                id: check.id,
                class: Class::Drift,
                detail: format!("check '{}' failed to run", check.id),
            })
        })
        .collect()
}

fn verdict(id: &'static str, class: Class, detail: impl Into<String>) -> Verdict {
    Verdict { id, class, detail: detail.into() }
}

/// Byte-exact mirror targets: this is the row that answers "is live what we shipped".
fn check_mirror(ctx: &Ctx<'_>) -> Result<Verdict> {
    let plan = converge::plan(ctx.intent, ctx.host)?;
    let stale: Vec<&str> = plan
        .changes
        .iter()
        .filter_map(|c| match c {
            converge::Planned::Owned { live, .. } => Some(live.as_str()),
            _ => None,
        })
        .collect();
    Ok(if stale.is_empty() {
        verdict(
            "MIRROR",
            Class::Ok,
            format!("{} mirror target(s) byte-equal", ctx.intent.mirror.len()),
        )
    } else {
        verdict(
            "MIRROR",
            Class::Drift,
            format!("{} target(s) differ: {}", stale.len(), stale.join(" ")),
        )
    })
}

fn check_managed(ctx: &Ctx<'_>) -> Result<Verdict> {
    let plan = converge::plan(ctx.intent, ctx.host)?;
    if !plan.anomalies.is_empty() {
        return Ok(verdict(
            "MANAGED",
            Class::Drift,
            format!("unclassifiable: {}", plan.anomalies.join("; ")),
        ));
    }
    let drifting: Vec<&str> = plan
        .changes
        .iter()
        .filter_map(|c| match c {
            converge::Planned::Managed { record, .. } => Some(record.as_str()),
            _ => None,
        })
        .collect();
    Ok(if drifting.is_empty() {
        verdict(
            "MANAGED",
            Class::Ok,
            format!("{} record(s) hold their line", ctx.intent.managed.len()),
        )
    } else {
        verdict(
            "MANAGED",
            Class::Drift,
            format!("{} record(s) drifted: {}", drifting.len(), drifting.join(" ")),
        )
    })
}

fn check_sync(ctx: &Ctx<'_>) -> Result<Verdict> {
    let plan = converge::plan(ctx.intent, ctx.host)?;
    if plan.sysctl.is_empty() {
        return Ok(verdict(
            "SYNC",
            Class::Ok,
            format!("{} runtime sysctl value(s) match intent", ctx.intent.sysctl.len()),
        ));
    }
    let listed: Vec<String> = plan
        .sysctl
        .iter()
        .map(|d| match &d.current {
            Some(cur) => format!("{}={cur} want {}", d.key, d.want),
            None => format!("{}=unreadable want {}", d.key, d.want),
        })
        .collect();
    Ok(verdict("SYNC", Class::Drift, listed.join(" ")))
}

fn check_ceil(ctx: &Ctx<'_>) -> Result<Verdict> {
    let policies = ceil::policies(ctx.host);
    if policies.is_empty() {
        return Ok(verdict("CEIL", Class::Skipped, "no cpufreq policies found"));
    }
    let mut bad = Vec::new();
    for policy in &policies {
        let want = match ceil::want_for_policy(ctx.host, policy) {
            Ok(w) => w,
            Err(e) => return Ok(verdict("CEIL", Class::Drift, e.message)),
        };
        let path = format!("{policy}/scaling_max_freq");
        let current = ctx.host.read_opt(&path).map(|s| s.trim().to_string());
        if current.as_deref() != Some(want.to_string().as_str()) {
            let name = policy.rsplit('/').next().unwrap_or(policy);
            bad.push(format!("{name}={} want {want}", current.as_deref().unwrap_or("unreadable")));
        }
    }
    Ok(if bad.is_empty() {
        verdict(
            "CEIL",
            Class::Ok,
            format!("all {} policies capped {} / {}", policies.len(), ceil::ZEN5_KHZ, ceil::ZEN5C_KHZ),
        )
    } else {
        verdict("CEIL", Class::Drift, bad.join(" "))
    })
}

/// CO is observable only as intent: the SMU does not read back a curve offset.
///
/// So this row guards the thing a rewrite can actually lose — the offset silently
/// disappearing from the intent — and says so, instead of pretending to measure the silicon.
fn check_co(ctx: &Ctx<'_>) -> Result<Verdict> {
    let co = &ctx.intent.ryzenadj.co;
    if co.all == 0 && co.per_core.is_empty() {
        return Ok(verdict(
            "CO",
            Class::Drift,
            "intent declares no curve-optimizer offset (hw-tune had coall + per-core)",
        ));
    }
    Ok(verdict(
        "CO",
        Class::Ok,
        format!("intent declares coall {} + {} per-core offset(s)", co.all, co.per_core.len()),
    ))
}

fn check_smu(ctx: &Ctx<'_>) -> Result<Verdict> {
    let result = ctx.host.run(&["ryzenadj".into(), "--info".into()]);
    let Ok(output) = result else {
        return Ok(verdict("SMU", Class::Skipped, "needs root (ryzenadj --info)"));
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let fast = smu_value(&text, "PPT LIMIT FAST");
    let slow = smu_value(&text, "PPT LIMIT SLOW");
    let stapm = smu_value(&text, "STAPM LIMIT");
    let (Some(fast), Some(slow)) = (fast, slow) else {
        return Ok(verdict(
            "SMU",
            Class::Skipped,
            "ryzenadj --info returned no power table (needs root, or ryzenadj is missing)",
        ));
    };
    let want = ctx.intent.ryzenadj.limits.fast.map(|v| v as f64 / 1000.0);
    let want_str = want.map(|w| format!("{w:.3}")).unwrap_or_else(|| "unset".into());
    if want_str != "unset" && (fast != want_str || slow != want_str) {
        return Ok(verdict(
            "SMU",
            Class::Drift,
            format!("PPT FAST={fast} SLOW={slow} want {want_str}"),
        ));
    }
    match stapm.as_deref() {
        Some(v) if v == want_str => Ok(verdict(
            "SMU",
            Class::Ok,
            format!("PPT FAST/SLOW/STAPM {v}"),
        )),
        // ACCEPTED is scoped to a readback BELOW the envelope: the firmware's own floor.
        Some(v) if v.parse::<f64>().map(|f| f < want.unwrap_or(f64::MAX)).unwrap_or(false) => Ok(
            verdict("SMU", Class::Accepted, format!("STAPM {v} W, firmware preference")),
        ),
        Some(v) => Ok(verdict(
            "SMU",
            Class::Drift,
            format!("STAPM {v} above the {want_str} envelope"),
        )),
        None => Ok(verdict(
            "SMU",
            Class::Drift,
            "STAPM readback unparseable (output format changed)",
        )),
    }
}

/// First decimal number on the `--info` line naming `field`.
fn smu_value(text: &str, field: &str) -> Option<String> {
    // The table is pipe-delimited: `| STAPM LIMIT | 24.000 | stapm-limit |`. The value is the
    // first digit run after the field name, not the first token after it — trimming whitespace
    // alone lands on the separator and parses nothing.
    let line = text.lines().find(|l| l.contains(field))?;
    let rest = &line[line.find(field)? + field.len()..];
    let start = rest.find(|c: char| c.is_ascii_digit())?;
    let number: String = rest[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!number.is_empty()).then_some(number)
}


fn check_power(ctx: &Ctx<'_>) -> Result<Verdict> {
    if ctx.intent.ryzenadj.limits.power_saving {
        Ok(verdict(
            "POWER",
            Class::Ok,
            "intent declares the SMU power-saving flag",
        ))
    } else {
        Ok(verdict(
            "POWER",
            Class::Drift,
            "intent does not declare --power-saving (the hidden eco behaviour is gone)",
        ))
    }
}

fn check_wifi(ctx: &Ctx<'_>) -> Result<Verdict> {
    let iface = ctx.intent.passes.wifi_interface.as_str();
    let result = ctx.host.run(&[
        "iw".into(),
        "dev".into(),
        iface.into(),
        "get".into(),
        "power_save".into(),
    ]);
    match result {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let on = text.to_lowercase().contains("power save: on");
            Ok(if on {
                verdict("WIFI", Class::Ok, format!("{iface} power_save on"))
            } else {
                verdict("WIFI", Class::Drift, format!("{iface} power_save off"))
            })
        }
        _ => Ok(verdict("WIFI", Class::Skipped, "iw unavailable or interface absent")),
    }
}

fn check_io(ctx: &Ctx<'_>) -> Result<Verdict> {
    // The writeback pair is the one sysctl family with a second writer (TLP rewrites it on
    // every power-source event), so it gets its own row even though SYNC covers it.
    let keys = ["vm.dirty_writeback_centisecs", "vm.dirty_expire_centisecs"];
    let mut bad = Vec::new();
    for key in keys {
        let want = ctx.intent.sysctl.get(key);
        let current = ctx
            .host
            .read_opt(&converge::proc_sys_path(key))
            .map(|raw| converge::norm_sysctl(&raw));
        match (want, current) {
            (Some(want), Some(current)) if converge::norm_sysctl(want) == current => {}
            (Some(want), current) => bad.push(format!(
                "{key}={} want {want}",
                current.as_deref().unwrap_or("unreadable")
            )),
            (None, _) => bad.push(format!("{key} not declared in intent")),
        }
    }
    Ok(if bad.is_empty() {
        verdict("IO", Class::Ok, "dirty writeback/expire hold their intent values")
    } else {
        verdict("IO", Class::Drift, bad.join(" "))
    })
}

fn check_timer(ctx: &Ctx<'_>) -> Result<Verdict> {
    const UNITS: &[&str] = &["genarch.service", "genarch-ceil.timer"];
    let mut not_enabled = Vec::new();
    for unit in UNITS {
        let result = ctx.host.run(&[
            "systemctl".into(),
            "is-enabled".into(),
            (*unit).to_string(),
        ]);
        let enabled = matches!(&result, Ok(o) if o.status.success());
        if !enabled {
            not_enabled.push(*unit);
        }
    }
    Ok(if not_enabled.is_empty() {
        verdict("TIMER", Class::Ok, format!("{UNITS:?} enabled"))
    } else {
        verdict(
            "TIMER",
            Class::Drift,
            format!("not enabled: {}", not_enabled.join(" ")),
        )
    })
}

/// Files that should not be on the host: declared removals plus anything the manifest
/// records but the intent no longer ships.
///
/// The derivation lives in [`converge::plan`] — the same one apply executes — so the verdict
/// and the action cannot disagree. A pending prune is **DRIFT**, not a warning: it is the
/// double-writer the cutover exists to prevent, and the report's exit code has to say so.
fn check_prune(ctx: &Ctx<'_>) -> Result<Verdict> {
    let plan = converge::plan(ctx.intent, ctx.host)?;
    let pending: Vec<&str> = plan
        .changes
        .iter()
        .filter_map(|c| match c {
            converge::Planned::Prune { live } => Some(live.as_str()),
            _ => None,
        })
        .collect();
    Ok(if pending.is_empty() {
        verdict(
            "PRUNE",
            Class::Ok,
            format!("{} declared removal(s), none present", ctx.intent.prune.len()),
        )
    } else {
        verdict(
            "PRUNE",
            Class::Drift,
            format!("{} path(s) to remove at apply: {}", pending.len(), pending.join(" ")),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::smu_value;

    #[test]
    fn reads_the_pipe_delimited_table() {
        let text = "| STAPM LIMIT         |    24.000 | stapm-limit |\n\
                    | PPT LIMIT FAST      |    31.000 | fast-limit  |\n\
                    | PPT LIMIT SLOW      |    20.000 | slow-limit  |\n";
        assert_eq!(smu_value(text, "STAPM LIMIT").as_deref(), Some("24.000"));
        assert_eq!(smu_value(text, "PPT LIMIT FAST").as_deref(), Some("31.000"));
        assert_eq!(smu_value(text, "PPT LIMIT SLOW").as_deref(), Some("20.000"));
        assert_eq!(smu_value(text, "PPT LIMIT APU"), None);
    }
}
