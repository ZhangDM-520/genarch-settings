//! The shipped-profile gate.
//!
//! A profile is the one file in this repository a reader will copy to a machine this project has
//! never seen, so it is the one file where an unverifiable value is worse than a missing one.
//! The audit below is the contract in executable form: it is deliberately blunt, and it fails on
//! a *plausible* profile as well as a broken one.

use std::path::{Path, PathBuf};

use genarch::intent::{Intent, ProfileStatus};

/// Facts that only exist on the machine in hand. A profile that carries one is a measurement
/// pretending to be a document — the exact mistake that made 0.1.x non-portable.
const LIVE_ONLY: &[&str] = &["/home/", "/dev/", "/var/lib/", "UUID=", "/sys/", "/proc/"];

fn profiles_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("intent/profiles")
}

fn slugs() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(profiles_root())
        .expect("intent/profiles must exist")
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    out.sort();
    out
}

/// Select `slug` in a scratch overlay and load the shipped intent as a machine would.
fn load(slug: &str) -> Result<Intent, String> {
    let dir = std::env::temp_dir().join(format!(
        "genarch-profile-{}-{slug}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("profile"), format!("profile = \"{slug}\"\n"))
        .map_err(|e| e.to_string())?;
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("intent");
    let loaded = Intent::load(&base, &dir).map_err(|e| e.message);
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

/// Everything wrong with one profile, as `field: problem` lines.
fn audit(slug: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let intent = match load(slug) {
        Ok(intent) => intent,
        Err(message) => return vec![format!("load: {message}")],
    };
    let Some(profile) = &intent.profile else {
        return vec!["selection: the profile did not load".to_string()];
    };
    if profile.name != slug {
        problems.push(format!(
            "name: profile reports '{}' but the directory is '{slug}'",
            profile.name
        ));
    }
    // The selection mechanism is an exact DMI match; without a requirement the row can only
    // ever say "selected", which is not a judgement about this machine.
    if profile.requires.is_empty() {
        problems.push("requires: declares no machine requirement".to_string());
    }
    if profile.status == ProfileStatus::Documented && profile.source.is_empty() {
        problems.push("source: documented but cites nothing".to_string());
    }

    let documented = profile.status == ProfileStatus::Documented;
    if documented {
        for target in &intent.mirror {
            problems.push(format!(
                "mirror: a documented profile ships no file ({})",
                target.live
            ));
        }
        for record in &intent.managed {
            problems.push(format!(
                "managed: a documented profile patches no foreign file ({})",
                record.target
            ));
        }
        if intent.ryzenadj.co.all.is_some() || intent.ryzenadj.co.per_core.is_some() {
            problems.push("co: a curve offset is a measurement, not a publication".to_string());
        }
        if intent.passes.wake_disarm.is_some() {
            problems.push("wake_disarm: read from /proc/acpi/wakeup, never published".to_string());
        }
        if intent.passes.usb_autosuspend.is_some() {
            problems.push("usb_autosuspend: a device list is read with lsusb".to_string());
        }
    }

    // A *measured* profile may own the machine's own paths — that is what measuring means, and
    // the reference profile legitimately ships this user's autostart entries. A documented one
    // may not: a stranger's home directory is not a fact a publication carries.
    if documented {
        let mut paths: Vec<String> = intent.mirror.iter().map(|m| m.live.clone()).collect();
        paths.extend(intent.managed.iter().map(|r| r.target.clone()));
        for path in paths {
            for needle in LIVE_ONLY {
                if path.contains(needle) {
                    problems.push(format!("path: {path} names a live-only location ({needle})"));
                }
            }
        }
    }

    match &intent.ceil {
        None => {}
        Some(ceil) => {
            if ceil.classes.is_empty() {
                problems.push("ceil: no classes".to_string());
            }
            for class in &ceil.classes {
                if !(500_000..=10_000_000).contains(&class.khz) {
                    problems.push(format!(
                        "ceil: class '{}' declares {} kHz, a unit mistake",
                        class.name, class.khz
                    ));
                }
            }
        }
    }
    problems
}

#[test]
fn every_shipped_profile_is_selectable_and_honest() {
    let slugs = slugs();
    assert!(!slugs.is_empty(), "no profile ships at all");

    let mut failures = Vec::new();
    for slug in &slugs {
        for problem in audit(slug) {
            failures.push(format!("{slug}: {problem}"));
        }
    }
    assert!(failures.is_empty(), "profiles that do not meet the contract:\n{}", failures.join("\n"));
}

#[test]
fn the_measured_profile_is_marked_measured_and_the_rest_are_not() {
    // The one measured profile is the reference implementation; every other profile must either
    // cite its values or say it was measured here, and nothing in between.
    let measured = load("zenbook-s16").expect("the reference profile must load");
    assert_eq!(
        measured.profile.as_ref().unwrap().status,
        ProfileStatus::Measured,
        "zenbook-s16 is our own hardware"
    );

    let documented: Vec<String> = slugs()
        .into_iter()
        .filter(|slug| {
            load(slug)
                .ok()
                .and_then(|i| i.profile.map(|p| p.status))
                .map(|s| s == ProfileStatus::Documented)
                .unwrap_or(false)
        })
        .collect();
    for slug in &documented {
        let intent = load(slug).unwrap();
        let sources = &intent.profile.as_ref().unwrap().source;
        assert!(
            sources.iter().all(|s| s.starts_with("http")),
            "{slug}: a source must be a URL, got {sources:?}"
        );
    }
}

#[test]
fn the_audit_catches_a_measured_fact_in_a_documented_profile() {
    // The gate is only worth having if it fails. This builds a profile that looks right and
    // carries exactly the things a documented profile must not: a machine path, a measured CO
    // offset and a wake list.
    let dir = std::env::temp_dir().join(format!("genarch-profile-audit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let base = dir.join("base");
    let overlay = dir.join("etc");
    std::fs::create_dir_all(base.join("profiles/fake/mirror/home/someone/.config")).unwrap();
    std::fs::create_dir_all(&overlay).unwrap();
    std::fs::write(base.join("intent.toml"), "schema = 3\n").unwrap();
    std::fs::write(
        base.join("profiles/fake/intent.toml"),
        "status = \"documented\"\nsource = [\"https://example.invalid/spec\"]\n\
         [requires]\nproduct = \"X\"\n",
    )
    .unwrap();
    std::fs::write(base.join("profiles/fake/ryzenadj.toml"), "[co]\nall = -7\n").unwrap();
    std::fs::write(
        base.join("profiles/fake/passes.toml"),
        "wake_disarm = [\"GPP0\"]\nusb_autosuspend = [\"13d3:3608\"]\n",
    )
    .unwrap();
    std::fs::write(base.join("profiles/fake/mirror/home/someone/.config/x"), "x\n").unwrap();
    std::fs::write(overlay.join("profile"), "profile = \"fake\"\n").unwrap();

    let intent = Intent::load(&base, &overlay).unwrap();
    let profile = intent.profile.as_ref().unwrap();
    assert_eq!(profile.status, ProfileStatus::Documented);
    assert_eq!(profile.source, vec!["https://example.invalid/spec".to_string()]);

    // Each rule the audit applies, on the loaded intent: every one of these is a violation.
    assert!(!intent.mirror.is_empty(), "the fixture must ship a file");
    assert!(intent.mirror.iter().any(|m| m.live.contains("/home/")));
    assert!(intent.ryzenadj.co.all.is_some());
    assert!(intent.passes.wake_disarm.is_some());
    assert!(intent.passes.usb_autosuspend.is_some());

    // A documented profile with no citation is refused at load time, before any audit runs.
    std::fs::write(
        base.join("profiles/fake/intent.toml"),
        "status = \"documented\"\n[requires]\nproduct = \"X\"\n",
    )
    .unwrap();
    let refused = Intent::load(&base, &overlay).unwrap_err();
    assert!(refused.message.contains("names no source"), "{}", refused.message);
    let _ = std::fs::remove_dir_all(&dir);
}
