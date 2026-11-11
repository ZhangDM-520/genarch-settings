//! The scheduled passes: what the boot, resume and ceiling-watch triggers run.
//!
//! These are the only paths that touch the SMU, and the split between them is a safety
//! property rather than an optimisation:
//!
//! - **boot** applies firmware-adjacent settings, then repeats the SMU pass once after a
//!   settle delay so it lands *after* the platform-profile handlers that reset firmware
//!   limits on every power event.
//! - **resume** does the same work exactly **once**, with no sleep: systemd thaws
//!   `user.slice` only after the sleep hook exits, so a sleep here stalls every resume
//!   (measured: 5.1 s per wake).
//! - **ceil** is drift-only. It touches `scaling_max_freq` and nothing else — no SMU writes,
//!   no EPP writes — because register churn is the wedge race the SMU paths avoid.
//!
//! Everything written here goes through [`Host`], so a fixture rehearsal sees the same plan.

use crate::ceil;
use crate::exit::Result;
use crate::host::Host;
use crate::intent::Intent;
use crate::ryzenadj;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Boot,
    Resume,
    Ceil,
}

#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<String>,
}

impl Report {
    fn say(&mut self, line: impl Into<String>) {
        let line = line.into();
        eprintln!("genarch: {line}");
        self.lines.push(line);
    }
}

pub fn run(host: &dyn Host, intent: &Intent, pass: Pass) -> Result<Report> {
    let mut report = Report::default();
    // One writer at a time. A scheduled pass (the 2-minute ceil timer, the boot/resume hooks)
    // and an operator's `apply` both write kernel attributes; without the lock they interleave
    // and the last writer wins silently. A held lock means an apply is converging the same
    // knobs, so the pass stands down — and says so, because a silent skip is the failure this
    // engine exists to end. The timer runs again in two minutes.
    let _lock = match host.lock(crate::converge::LOCK_PATH) {
        Ok(lock) => lock,
        Err(e) if e.kind == crate::exit::Exit::Precondition => {
            report.say(format!("{pass:?} pass skipped: {}", e.message));
            return Ok(report);
        }
        Err(e) => return Err(e),
    };
    match pass {
        Pass::Ceil => apply_ceil(host, &mut report)?,
        Pass::Resume => {
            apply_smu(host, intent, &mut report)?;
            apply_host_state(host, intent, &mut report)?;
        }
        Pass::Boot => {
            apply_smu(host, intent, &mut report)?;
            apply_host_state(host, intent, &mut report)?;
            // The convergence pass exists because a platform_profile write (asusd does one
            // per power-source event) makes the EC reset SMU limits and EPP.
            if intent.passes.settle_secs > 0 {
                std::thread::sleep(std::time::Duration::from_secs(intent.passes.settle_secs));
                apply_smu(host, intent, &mut report)?;
            }
        }
    }
    Ok(report)
}

/// The ceiling watch: drift-only.
pub fn apply_ceil(host: &dyn Host, report: &mut Report) -> Result<()> {
    let policies = ceil::policies(host);
    if policies.is_empty() {
        report.say("ceil: no cpufreq policies found");
        return Ok(());
    }
    for policy in policies {
        let want = ceil::want_for_policy(host, &policy)?;
        let path = format!("{policy}/scaling_max_freq");
        let current = host.read_opt(&path).map(|s| s.trim().to_string());
        if current.as_deref() == Some(want.to_string().as_str()) {
            continue;
        }
        // The caught value is logged: the cap has been observed overwritten by an
        // unidentified actor, and these lines are what name it in the journal.
        report.say(format!(
            "CEIL DRIFT {} {} -> {}",
            policy.rsplit('/').next().unwrap_or(&policy),
            current.as_deref().unwrap_or("unreadable"),
            want
        ));
        host.write(&path, want.to_string().as_bytes(), None)?;
    }
    Ok(())
}

fn apply_smu(host: &dyn Host, intent: &Intent, report: &mut Report) -> Result<()> {
    let calls = ryzenadj::apply(host, &intent.ryzenadj)?;
    let failed = calls.iter().filter(|c| !c.ok).count();
    if calls.is_empty() {
        report.say("smu: no ryzenadj settings declared");
    } else if failed == 0 {
        report.say(format!(
            "smu: {} ryzenadj call(s) applied (CO {}, {} per-core offset(s), limits {}, power-saving {})",
            calls.len(),
            intent.ryzenadj.co.all,
            intent.ryzenadj.co.per_core.len(),
            if intent.ryzenadj.limits.fast.is_some() { "set" } else { "unset" },
            intent.ryzenadj.limits.power_saving
        ));
    } else {
        report.say(format!(
            "smu: {failed} of {} ryzenadj call(s) failed — firmware limits may still stand",
            calls.len()
        ));
    }
    Ok(())
}

fn apply_host_state(host: &dyn Host, intent: &Intent, report: &mut Report) -> Result<()> {
    // cpuidle governor
    let governor_path = "/sys/devices/system/cpu/cpuidle/current_governor";
    if host.exists(governor_path) {
        let want = intent.passes.idle_governor.as_str();
        if host.read_opt(governor_path).map(|s| s.trim().to_string()).as_deref() != Some(want) {
            // A refused write is a failure the operator must see: a pass that "succeeds"
            // while the governor never moved is the silent divergence this engine exists
            // to end.
            host.write(governor_path, want.as_bytes(), None)?;
            report.say(format!("cpuidle governor={want}"));
        }
    }

    apply_ceil(host, report)?;

    // ACPI wake sources: only disarm the ones the intent names, and only when armed.
    if let Some(wakeup) = host.read_opt("/proc/acpi/wakeup") {
        for device in &intent.passes.wake_disarm {
            let armed = wakeup.lines().any(|line| {
                let mut fields = line.split_whitespace();
                fields.next() == Some(device.as_str()) && line.contains("*enabled")
            });
            if armed {
                host.write("/proc/acpi/wakeup", device.as_bytes(), None)?;
                report.say(format!("wake disabled: {device}"));
            }
        }
    }

    // USB autosuspend for the named devices.
    if let Ok(entries) = host.walk("/sys/bus/usb/devices") {
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            let name = match entry.path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let id_vendor = host.read_opt(&format!("/sys/bus/usb/devices/{name}/idVendor"));
            let id_product = host.read_opt(&format!("/sys/bus/usb/devices/{name}/idProduct"));
            let (Some(vendor), Some(product)) = (id_vendor, id_product) else { continue };
            let id = format!("{}:{}", vendor.trim(), product.trim());
            if intent.passes.usb_autosuspend.contains(&id) {
                let control = format!("/sys/bus/usb/devices/{name}/power/control");
                if host.exists(&control) {
                    host.write(&control, b"auto", None)?;
                    report.say(format!("bt usb autosuspend: {name}"));
                }
            }
        }
    }

    // WiFi power save: the radio must sleep between beacons.
    let iface = intent.passes.wifi_interface.as_str();
    let argv = vec![
        "iw".to_string(),
        "dev".to_string(),
        iface.to_string(),
        "set".to_string(),
        "power_save".to_string(),
        "on".to_string(),
    ];
    if host.run(&argv).map(|o| o.success).unwrap_or(false) {
        report.say(format!("{iface} power_save=on"));
    }

    reassert_sysctl(host, intent, report)?;
    Ok(())
}

/// Re-assert the runtime sysctl values.
///
/// Not redundant with the intent file: these drift independently of any config, because TLP
/// rewrites the dirty-writeback pair on every power-source event.
fn reassert_sysctl(host: &dyn Host, intent: &Intent, report: &mut Report) -> Result<()> {
    let mut changed = Vec::new();
    for (key, want) in &intent.sysctl {
        let path = crate::converge::proc_sys_path(key);
        let current = host.read_opt(&path).map(|raw| crate::converge::norm_sysctl(&raw));
        if current.as_deref() == Some(crate::converge::norm_sysctl(want).as_str()) {
            continue;
        }
        host.write(&path, want.as_bytes(), None)?;
        changed.push(key.clone());
    }
    if !changed.is_empty() {
        report.say(format!("sysctl re-asserted: {}", changed.join(" ")));
    }
    Ok(())
}
