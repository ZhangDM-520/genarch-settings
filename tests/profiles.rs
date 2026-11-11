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
    slugs_at(&profiles_root())
}

fn slugs_at(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root)
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
    load_at(profiles_root().parent().expect("intent/"), slug)
}

fn load_at(intent_root: &Path, slug: &str) -> Result<Intent, String> {
    // Unique per call, not per test: two tests loading the same slug in parallel would
    // otherwise share (and delete) one scratch overlay.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "genarch-profile-{}-{slug}-{seq}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("profile"), format!("profile = \"{slug}\"\n"))
        .map_err(|e| e.to_string())?;
    let loaded = Intent::load(intent_root, &dir).map_err(|e| e.message);
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

/// Files this profile *layer* ships, relative to its own directory.
///
/// The resolved intent also carries the base layer's mirrors, so it cannot answer "does this
/// profile ship a file" — the question is about the profile's own tree.
fn layer_files(root: &Path, slug: &str) -> Vec<(String, PathBuf)> {
    let root = root.join(slug);
    let mut out = Vec::new();
    for dir in ["mirror", "managed"] {
        let mut stack = vec![root.join(dir)];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(&root) {
                    out.push((rel.display().to_string(), path));
                }
            }
        }
    }
    out.sort();
    out
}

/// Everything wrong with one profile, as `field: problem` lines.
fn audit(slug: &str) -> Vec<String> {
    audit_at(&profiles_root(), slug)
}

/// The audit, over any profiles root, so the gate can be pointed at a deliberately bad profile.
fn audit_at(profiles: &Path, slug: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let intent_root = profiles.parent().expect("intent/").to_path_buf();
    let intent = match load_at(&intent_root, slug) {
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
        for (rel, _) in layer_files(profiles, slug) {
            problems.push(format!(
                "file: a documented profile ships nothing of its own ({rel})"
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
    // may not: a stranger's home directory is not a fact a publication carries. This looks at
    // the profile layer's own files, since the base legitimately ships `/etc` paths.
    if documented {
        for (rel, _) in layer_files(profiles, slug) {
            for needle in LIVE_ONLY {
                if rel.contains(needle) {
                    problems.push(format!("path: {rel} names a live-only location ({needle})"));
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
fn no_two_profiles_claim_the_same_machine() {
    // `[requires]` is exact equality, so two profiles with the same product string make one of
    // them unreachable — the loser is silently never selected, which reads as "no profile for
    // this machine" rather than as a bug.
    let mut claimed: Vec<(String, String)> = Vec::new();
    for slug in slugs() {
        if let Ok(intent) = load(&slug)
            && let Some(profile) = &intent.profile
            && let Some(product) = profile.requires.get("product")
        {
            claimed.push((product.clone(), slug));
        }
    }
    claimed.sort();
    for pair in claimed.windows(2) {
        if pair[0].0 == pair[1].0 {
            panic!(
                "profiles '{}' and '{}' both require product = {:?}; one can never be selected",
                pair[0].1, pair[1].1, pair[0].0
            );
        }
    }
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

    // The audit must fire on every one of these, not merely observe them.
    let problems = audit_at(&base.join("profiles"), "fake");
    let joined = problems.join("\n");
    for expected in ["file:", "co:", "wake_disarm:", "usb_autosuspend:", "path:"] {
        assert!(
            problems.iter().any(|p| p.starts_with(expected)),
            "audit missed {expected}:\n{joined}"
        );
    }

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
