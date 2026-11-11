//! Behavioural gate for converge and the managed line-patch engine.
//!
//! Every case here is a lesson the shell engine paid for, restated as a property. The tests
//! run through [`FixtureHost`] — the same code paths as production, aimed at a scratch tree —
//! so they describe behaviour rather than implementation.

use std::path::{Path, PathBuf};

use genarch::converge::{self, Planned};
use genarch::host::{FixtureHost, Host};
use genarch::intent::{Intent, SCHEMA_VERSION};
use genarch::managed;
use genarch::tree;

struct Case {
    dir: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let dir = std::env::temp_dir().join(format!("genarch-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Case { dir }
    }

    fn live(&self) -> PathBuf {
        self.dir.join("live")
    }

    fn intent(&self) -> PathBuf {
        self.dir.join("intent")
    }

    fn host(&self) -> FixtureHost {
        FixtureHost::new(self.live())
    }

    fn write_live(&self, live: &str, content: &str) {
        let path = self.live().join(live.trim_start_matches('/'));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }

    fn write_intent_mirror(&self, live: &str, content: &str) {
        let path = self.intent().join("mirror").join(live.trim_start_matches('/'));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }

    fn write_managed(&self, name: &str, body: &str) {
        let dir = self.intent().join("managed");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.toml")), body).unwrap();
    }

    fn write_intent_meta(&self) {
        let dir = self.intent();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("intent.toml"), format!("schema = {SCHEMA_VERSION}\n")).unwrap();
    }

    fn load(&self) -> Intent {
        Intent::load(&self.intent(), &self.dir.join("no-overlay")).unwrap()
    }

    fn read_live(&self, live: &str) -> String {
        std::fs::read_to_string(self.live().join(live.trim_start_matches('/'))).unwrap()
    }

    fn backups(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![self.live()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains(".bak-"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        out.sort();
        out
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn apply_writes_the_target_and_keeps_a_dated_pre_image() {
    let case = Case::new("pre-image");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/tlp.d/01-eco.conf", "new\n");
    case.write_live("/etc/tlp.d/01-eco.conf", "old\n");

    let intent = case.load();
    let host = case.host();
    let (plan, outcome) = converge::apply(&intent, &host, false).unwrap();

    assert_eq!(plan.changes.len(), 1);
    assert_eq!(case.read_live("/etc/tlp.d/01-eco.conf"), "new\n");
    assert_eq!(outcome.backups.len(), 1);
    let backup = &outcome.backups[0];
    assert!(backup.to_string_lossy().contains(".bak-"), "{backup:?}");
    let on_disk = case.live().join(backup.to_string_lossy().trim_start_matches('/'));
    assert_eq!(std::fs::read_to_string(on_disk).unwrap(), "old\n");
}

#[test]
fn a_converged_host_plans_nothing() {
    let case = Case::new("idempotent");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/a.conf", "same\n");
    case.write_live("/etc/a.conf", "same\n");

    let intent = case.load();
    let host = case.host();
    let (plan, _) = converge::apply(&intent, &host, false).unwrap();
    assert!(plan.is_empty(), "{plan:?}");
    let after = converge::plan(&intent, &host).unwrap();
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn sysctl_multi_value_is_compared_whole() {
    let case = Case::new("sysctl-multi");
    case.write_intent_meta();
    case.write_live("/proc/sys/kernel/printk", "3\t3\t3\t3\n");
    let intent_dir = case.intent();
    std::fs::write(intent_dir.join("sysctl.toml"), "[values]\n\"kernel.printk\" = \"3 3 3 3\"\n")
        .unwrap();

    let intent = case.load();
    let host = case.host();
    let plan = converge::plan(&intent, &host).unwrap();
    // The tab-rendered live value and the space-written intent are the same value: capturing
    // only the first token made this re-assert forever and every apply exited 1.
    assert!(plan.sysctl.is_empty(), "{:?}", plan.sysctl);

    // A genuinely different value still shows up, whole.
    std::fs::write(intent_dir.join("sysctl.toml"), "[values]\n\"kernel.printk\" = \"4 4 4 4\"\n")
        .unwrap();
    let intent = case.load();
    let plan = converge::plan(&intent, &host).unwrap();
    assert_eq!(plan.sysctl.len(), 1);
    assert_eq!(plan.sysctl[0].want, "4 4 4 4");
}

#[test]
fn managed_patch_preserves_every_unclaimed_byte() {
    let case = Case::new("managed-bytes");
    case.write_intent_meta();
    let before = "[Journal]\nStorage=persistent\nSystemMaxUse=32M\nMaxFileSize=8M\n";
    case.write_live("/etc/systemd/journald.conf", before);
    case.write_managed(
        "journald",
        "name = \"journald\"\ntarget = \"/etc/systemd/journald.conf\"\nmatch = '^#?[[:space:]]*SystemMaxUse='\nline = \"SystemMaxUse=256M\"\n",
    );

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();

    let after = case.read_live("/etc/systemd/journald.conf");
    assert_eq!(
        after,
        "[Journal]\nStorage=persistent\nSystemMaxUse=256M\nMaxFileSize=8M\n"
    );
    // Line count is preserved: the patch replaces a line, it does not append one.
    assert_eq!(before.lines().count(), after.lines().count());
}

#[test]
fn managed_zero_match_refuses_and_writes_nothing() {
    let case = Case::new("managed-nomatch");
    case.write_intent_meta();
    let before = "[Journal]\nStorage=persistent\n";
    case.write_live("/etc/systemd/journald.conf", before);
    case.write_managed(
        "journald",
        "name = \"journald\"\ntarget = \"/etc/systemd/journald.conf\"\nmatch = '^SystemMaxUse='\nline = \"SystemMaxUse=256M\"\n",
    );

    let intent = case.load();
    let host = case.host();
    let err = converge::apply(&intent, &host, false).unwrap_err();
    assert_eq!(err.kind, genarch::exit::Exit::Partial);
    assert!(err.message.contains("0 lines match"), "{}", err.message);
    assert_eq!(case.read_live("/etc/systemd/journald.conf"), before);
    assert!(case.backups().is_empty(), "nothing may be written, not even a pre-image");
}

#[test]
fn managed_multi_match_refuses_to_choose() {
    let case = Case::new("managed-multi");
    case.write_intent_meta();
    let before = "SystemMaxUse=32M\nSystemMaxUse=64M\n";
    case.write_live("/etc/systemd/journald.conf", before);
    case.write_managed(
        "journald",
        "name = \"journald\"\ntarget = \"/etc/systemd/journald.conf\"\nmatch = '^SystemMaxUse='\nline = \"SystemMaxUse=256M\"\n",
    );

    let intent = case.load();
    let host = case.host();
    let err = converge::apply(&intent, &host, false).unwrap_err();
    assert!(err.message.contains("refusing to choose"), "{}", err.message);
    assert_eq!(case.read_live("/etc/systemd/journald.conf"), before);
}

#[test]
fn managed_section_scoping_ignores_a_lookalike_section() {
    let case = Case::new("managed-section");
    case.write_intent_meta();
    let before = "[Other]\nKey=wrong\n[Journal]\nKey=right\n[JournalFoo]\nKey=also-wrong\n";
    case.write_live("/etc/x.conf", before);
    case.write_managed(
        "journal",
        "name = \"journal\"\ntarget = \"/etc/x.conf\"\nsection = \"Journal\"\nmatch = '^Key='\nline = \"Key=patched\"\n",
    );

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();
    assert_eq!(
        case.read_live("/etc/x.conf"),
        "[Other]\nKey=wrong\n[Journal]\nKey=patched\n[JournalFoo]\nKey=also-wrong\n"
    );
}

#[test]
fn system_sleep_pre_image_goes_to_a_sibling_directory() {
    let case = Case::new("sleep-backup");
    case.write_intent_meta();
    case.write_intent_mirror("/usr/lib/systemd/system-sleep/genarch", "#!/bin/sh\nexit 0\n");
    case.write_live("/usr/lib/systemd/system-sleep/genarch", "#!/bin/sh\nexit 1\n");

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();

    // systemd executes EVERY file in that directory: a sibling `.bak` would run on resume.
    let dir = case.live().join("usr/lib/systemd");
    assert!(
        dir.join("system-sleep-backups/genarch.bak-" .to_string() + &genarch::clock::today()).exists(),
        "pre-image must live in system-sleep-backups/"
    );
    let siblings: Vec<_> = std::fs::read_dir(dir.join("system-sleep"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains(".bak"))
        .collect();
    assert!(siblings.is_empty(), "no .bak may sit next to a sleep hook: {siblings:?}");
}

#[test]
fn prune_removes_an_owned_target_that_left_the_intent() {
    let case = Case::new("prune");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/keep.conf", "keep\n");
    case.write_live("/etc/keep.conf", "keep\n");
    case.write_live("/etc/gone.conf", "stale\n");
    // The manifest is the ownership record: this engine wrote /etc/gone.conf earlier.
    case.write_live(
        converge::Manifest::live_path(),
        "owned = [\"/etc/keep.conf\", \"/etc/gone.conf\"]\n",
    );

    let intent = case.load();
    let host = case.host();
    // The derivation lives in `plan`, so the PRUNE verdict and the apply cannot disagree:
    // it was previously decided inside apply, which let `report` call the host clean while a
    // stale file (and therefore a second writer) was still on disk.
    let plan = converge::plan(&intent, &host).unwrap();
    assert!(
        plan.changes
            .iter()
            .any(|c| matches!(c, Planned::Prune { live } if live == "/etc/gone.conf")),
        "plan must see the manifest orphan: {plan:?}"
    );

    let (_, outcome) = converge::apply(&intent, &host, false).unwrap();
    assert!(outcome.pruned.contains(&"/etc/gone.conf".to_string()), "{outcome:?}");
    assert!(!host.exists("/etc/gone.conf"));
    assert!(host.exists("/etc/keep.conf"));

    let manifest = converge::Manifest::load(&host).unwrap();
    assert!(manifest.owned.contains("/etc/keep.conf"));
    assert!(!manifest.owned.contains("/etc/gone.conf"));
}

#[test]
fn prune_verdict_is_drift_not_a_warning() {
    // A stale owned file is the double-writer this engine exists to prevent. Reporting it as
    // WARN kept the exit code at 0, so a host with a stale deploy still read green.
    let case = Case::new("prune-verdict");
    case.write_intent_meta();
    case.write_intent_mirror("/usr/local/sbin/eco-power.sh", "shipped\n");
    case.write_live("/usr/local/sbin/eco-power.sh", "stale\n");
    let host = case.host();
    converge::apply(&case.load(), &host, false).unwrap();
    // The engine now owns the path; the intent stops shipping it, so the manifest is the
    // only record of what must disappear.
    std::fs::remove_file(case.intent().join("mirror/usr/local/sbin/eco-power.sh")).unwrap();
    std::fs::write(
        case.intent().join("intent.toml"),
        format!("schema = {SCHEMA_VERSION}\n"),
    )
    .unwrap();

    let intent = case.load();
    let verdicts = genarch::check::evaluate(&genarch::check::Ctx { host: &host, intent: &intent });
    let prune = verdicts.iter().find(|v| v.id == "PRUNE").unwrap();
    assert_eq!(prune.class, genarch::check::Class::Drift, "{prune:?}");

    converge::apply(&intent, &host, false).unwrap();
    let verdicts = genarch::check::evaluate(&genarch::check::Ctx { host: &host, intent: &intent });
    let prune = verdicts.iter().find(|v| v.id == "PRUNE").unwrap();
    assert_eq!(prune.class, genarch::check::Class::Ok, "{prune:?}");
}

#[test]
fn sysctl_projection_is_owned_and_pruned_when_it_leaves_the_intent() {
    // The projection is a file the engine writes, so it has to be in the manifest. It was
    // not: emptying sysctl.toml left /etc/sysctl.d/90-genarch.conf live and unreported, which
    // is exactly the "two homes for one value" the design exists to kill.
    let case = Case::new("projection");
    case.write_intent_meta();
    std::fs::write(
        case.intent().join("sysctl.toml"),
        "[values]\n\"vm.swappiness\" = \"100\"\n",
    )
    .unwrap();

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();
    assert!(host.exists(converge::RENDERED_SYSCTL_CONF));
    let manifest = converge::Manifest::load(&host).unwrap();
    assert!(
        manifest.owned.contains(converge::RENDERED_SYSCTL_CONF),
        "{manifest:?}"
    );

    std::fs::write(case.intent().join("sysctl.toml"), "[values]\n").unwrap();
    let intent = case.load();
    let plan = converge::plan(&intent, &host).unwrap();
    assert!(
        plan.changes.iter().any(|c| matches!(
            c,
            Planned::Prune { live } if live == converge::RENDERED_SYSCTL_CONF
        )),
        "{plan:?}"
    );
    converge::apply(&intent, &host, false).unwrap();
    assert!(!host.exists(converge::RENDERED_SYSCTL_CONF));
}

#[test]
fn crlf_line_endings_survive_a_patch() {
    // Rebuilding the file with `lines()` + `join("\n")` turned a CRLF target LF-only: every
    // other byte is supposed to survive, and the terminator is a byte.
    let case = Case::new("crlf");
    case.write_intent_meta();
    case.write_managed(
        "app",
        "name = \"app\"\ntarget = \"/etc/app.conf\"\nmatch = '^key = '\nline = \"key = 2\"\n",
    );
    case.write_live("/etc/app.conf", "key = 1\r\nother = x\r\n");

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();
    assert_eq!(case.read_live("/etc/app.conf"), "key = 2\r\nother = x\r\n");
}

#[test]
fn pre_image_keeps_the_source_mode() {
    // A pre-image written 0644 restores a 0755 hook as non-executable: systemd runs it and
    // fails with 203/EXEC, or the sleep hook is silently skipped. The mode is part of the
    // backup, not decoration.
    use std::os::unix::fs::PermissionsExt;

    let case = Case::new("pre-image-mode");
    case.write_intent_meta();
    case.write_intent_mirror("/usr/lib/systemd/system-sleep/hook", "new\n");
    case.write_live("/usr/lib/systemd/system-sleep/hook", "old\n");
    std::fs::set_permissions(
        case.live().join("usr/lib/systemd/system-sleep/hook"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    let intent = case.load();
    let host = case.host();
    converge::apply(&intent, &host, false).unwrap();

    let backups = case.backups();
    assert_eq!(backups.len(), 1, "{backups:?}");
    let mode = std::fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "pre-image lost the exec bit: {backups:?}");
}

#[test]
fn a_refused_pass_write_is_a_failure_not_a_silent_skip() {
    // `apply_host_state` used `write(..).is_ok()`: a refused sysfs write (or a wrong vintage
    // of procfs) made the pass exit 0 and say nothing, so the operator saw a green pass and
    // an unchanged machine.
    let case = Case::new("pass-write");
    case.write_intent_meta();
    std::fs::write(
        case.intent().join("sysctl.toml"),
        "[values]\n\"vm.swappiness\" = \"100\"\n",
    )
    .unwrap();
    // A directory where a file must be: every write to it fails, on any filesystem.
    std::fs::create_dir_all(case.live().join("proc/sys/vm/swappiness")).unwrap();

    let intent = case.load();
    let host = case.host();
    let err = genarch::pass::run(&host, &intent, genarch::pass::Pass::Resume).unwrap_err();
    assert!(err.message.contains("write"), "{}", err.message);
}

#[test]
fn lock_conflict_is_a_precondition_failure_naming_the_holder() {
    let case = Case::new("lock");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/a.conf", "new\n");
    case.write_live("/etc/a.conf", "old\n");

    let intent = case.load();
    let host = case.host();
    let first = host.lock(converge::LOCK_PATH).unwrap();
    let err = converge::apply(&intent, &host, false).unwrap_err();
    assert_eq!(err.kind, genarch::exit::Exit::Precondition);
    assert!(err.message.contains("lock held by"), "{}", err.message);
    drop(first);
    converge::apply(&intent, &host, false).unwrap();
}

#[test]
fn non_deployable_names_never_deploy() {
    let case = Case::new("stray");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/real.conf", "real\n");
    case.write_intent_mirror("/etc/real.conf.bak-20260101", "stray\n");

    // The loader refuses a mirror that holds a backup name outright: deploying it would be a
    // silent overwrite of a pre-image, and in system-sleep/ it would execute. One home for the
    // rule, and it fails before a plan exists.
    let err = Intent::load(&case.intent(), &case.dir.join("no-overlay")).unwrap_err();
    assert!(err.message.contains("non-deployable"), "{}", err.message);
}

#[test]
fn dry_run_writes_nothing_but_reports_the_plan() {
    let case = Case::new("dry-run");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/a.conf", "new\n");
    case.write_live("/etc/a.conf", "old\n");

    let intent = case.load();
    let host = case.host();
    let (plan, outcome) = converge::apply(&intent, &host, true).unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert!(outcome.applied.is_empty());
    assert_eq!(case.read_live("/etc/a.conf"), "old\n");
    assert!(host.recorded_writes().is_empty());
    assert!(!host.exists(converge::Manifest::live_path()));
}

#[test]
fn managed_record_with_a_bad_pattern_fails_before_any_write() {
    let case = Case::new("bad-pattern");
    case.write_intent_meta();
    case.write_live("/etc/x.conf", "a\n");
    case.write_managed(
        "broken",
        "name = \"broken\"\ntarget = \"/etc/x.conf\"\nmatch = '(unclosed'\nline = \"a\"\n",
    );

    let intent = case.load();
    let host = case.host();
    let err = converge::apply(&intent, &host, false).unwrap_err();
    assert!(err.message.contains("bad anchor pattern"), "{}", err.message);
}

#[test]
fn is_deployable_name_matches_the_paid_for_list() {
    for bad in [
        "x.bak",
        "x.bak-20260101",
        "x~",
        "x.orig",
        "x.rej",
        "x.swp",
        ".#x",
        "x.tmp",
    ] {
        assert!(!tree::is_deployable_name(bad), "{bad} must not deploy");
    }
    for good in ["x.conf", "eco-power", "hw-tune.fish", "10-eco.conf"] {
        assert!(tree::is_deployable_name(good), "{good} must deploy");
    }
}

#[test]
fn a_plan_carries_the_managed_record_it_will_write() {
    // The variant used to hold a record *name* and `apply` searched the intent for it, so a
    // plan could name a record its own intent no longer had. The plan is now executable on
    // its own: this test consumes it without consulting the intent at all.
    let case = Case::new("plan-carries-record");
    case.write_intent_meta();
    case.write_live("/etc/m.conf", "SystemMaxUse=32M\n");
    case.write_managed(
        "journald",
        "name = \"journald\"\ntarget = \"/etc/m.conf\"\nmatch = '^SystemMaxUse='\nline = \"SystemMaxUse=256M\"\n",
    );

    let intent = case.load();
    let host = case.host();
    let plan = converge::plan(&intent, &host).unwrap();
    let Planned::Managed { record } = &plan.changes[0] else {
        panic!("expected a managed change: {plan:?}");
    };
    assert_eq!(record.name, "journald");
    assert_eq!(record.line, "SystemMaxUse=256M");

    genarch::managed::replace(&host, record).unwrap();
    assert_eq!(case.read_live("/etc/m.conf"), "SystemMaxUse=256M\n");
}

#[test]
fn planned_variants_are_reachable_from_the_plan() {
    let case = Case::new("planned");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/a.conf", "new\n");
    case.write_live("/etc/a.conf", "old\n");
    case.write_live("/etc/m.conf", "SystemMaxUse=32M\n");
    case.write_managed(
        "journald",
        "name = \"journald\"\ntarget = \"/etc/m.conf\"\nmatch = '^SystemMaxUse='\nline = \"SystemMaxUse=256M\"\n",
    );

    let intent = case.load();
    let host = case.host();
    let plan = converge::plan(&intent, &host).unwrap();
    assert!(plan.changes.iter().any(|c| matches!(c, Planned::Owned { .. })));
    assert!(plan.changes.iter().any(|c| matches!(c, Planned::Managed { .. })));
}

#[test]
fn probe_reports_the_three_terminal_states() {
    let case = Case::new("probe");
    case.write_live("/etc/p.conf", "Key=old\n");
    let record = genarch::intent::ManagedRecord {
        name: "p".into(),
        target: "/etc/p.conf".into(),
        match_expr: "^Key=".into(),
        line: "Key=new".into(),
        section: None,
        doc: None,
    };
    let host = case.host();
    assert_eq!(managed::probe(&host, &record).unwrap().state, managed::State::Differs {
        current: "Key=old".into()
    });
    managed::replace(&host, &record).unwrap();
    assert_eq!(managed::probe(&host, &record).unwrap().state, managed::State::Ok);

    let missing = genarch::intent::ManagedRecord {
        target: "/etc/absent.conf".into(),
        ..record
    };
    assert_eq!(
        managed::probe(&host, &missing).unwrap().state,
        managed::State::MissingTarget
    );
}

/// The migration property: the intent derived from the shell tree must reproduce it exactly.
/// This is the gate that makes the cutover safe, so it runs against the real tree when it is
/// present on this machine.
#[test]
fn import_round_trip_reproduces_the_shell_state_tree() {
    let state = Path::new("/home/<user>/Projects/system-tuning/state");
    if !state.join("owned").is_dir() {
        eprintln!("skipping: no shell state tree at {}", state.display());
        return;
    }
    let case = Case::new("import-roundtrip");
    // The importer emits a machine profile, never a base: put it where a profile lives and
    // select it, which is exactly how the package ships it.
    let base = case.dir.join("base");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("intent.toml"), format!("schema = {SCHEMA_VERSION}\n")).unwrap();
    let out = base.join("profiles/imported");
    let summary = genarch::import::run(state, &out).unwrap();
    assert!(summary.mirror > 40, "expected the whole owned set, got {}", summary.mirror);

    let owned = state.join("owned");
    let mut checked = 0;
    for entry in walk(&owned) {
        let rel = entry.strip_prefix(&owned).unwrap();
        let live = format!("/{}", rel.display());
        let base = live.rsplit('/').next().unwrap();
        if !tree::is_deployable_name(base) {
            continue;
        }
        let imported = out.join("mirror").join(rel);
        let want = std::fs::read(&entry).unwrap();
        let got = std::fs::read(&imported).unwrap_or_else(|e| {
            panic!("import lost {}: {e}", entry.display());
        });
        assert_eq!(want, got, "byte mismatch for {live}");
        checked += 1;
    }
    assert!(checked > 40, "checked only {checked} files");

    // The intent must load, and loading must see the same bytes.
    let overlay = case.dir.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    std::fs::write(overlay.join("profile"), "profile = \"imported\"\n").unwrap();
    let intent = Intent::load(&base, &overlay).unwrap();
    assert_eq!(intent.profile.as_ref().map(|p| p.name.as_str()), Some("imported"));
    assert_eq!(intent.mirror.len(), checked);
    assert_eq!(intent.managed.len(), 8, "eight managed records in the shell tree");
    assert!(intent.sysctl.len() >= 8, "sysctl values collapsed from sysctl.d");
    assert_eq!(intent.ryzenadj.co.all, Some(-7));
    assert_eq!(
        intent.ryzenadj.co.per_core,
        Some(vec![-24, -24, -21, -21, -9, -11, -8, -6])
    );
    assert_eq!(intent.ryzenadj.limits.fast, Some(31000));
    assert_eq!(intent.ryzenadj.limits.power_saving, Some(true));
    // hw-tune's queue has nothing the knobs do not model, so the escape hatch must stay empty:
    // a non-empty raw list here would mean the fish script's shell syntax leaked into argv.
    assert!(
        intent.ryzenadj.raw.is_empty(),
        "unexpected raw args carried from hw-tune.fish: {:?}",
        intent.ryzenadj.raw
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_v2_intent_is_refused_instead_of_silently_losing_its_prune_list() {
    // `prune` is gone: the manifest is the ownership record. An old intent must not load with
    // its removals quietly ignored — that is the one failure mode a schema bump exists to
    // prevent.
    let case = Case::new("v2-intent");
    std::fs::create_dir_all(case.intent()).unwrap();
    std::fs::write(
        case.intent().join("intent.toml"),
        format!("schema = {SCHEMA_VERSION}\nprune = [\"/etc/eco-legacy.conf\"]\n"),
    )
    .unwrap();

    let err = Intent::load(&case.intent(), &case.dir.join("no-overlay")).unwrap_err();
    assert!(err.message.contains("prune"), "{}", err.message);

    // And a v2 schema number is refused with the schema message rather than parsed.
    std::fs::write(case.intent().join("intent.toml"), "schema = 2\n").unwrap();
    let err = Intent::load(&case.intent(), &case.dir.join("no-overlay")).unwrap_err();
    assert!(err.message.contains("schema 2"), "{}", err.message);
}

#[test]
fn sysctl_table_is_projected_into_one_file() {
    let case = Case::new("sysctl-projection");
    std::fs::create_dir_all(case.intent()).unwrap();
    std::fs::write(case.intent().join("intent.toml"), format!("schema = {SCHEMA_VERSION}\n")).unwrap();
    std::fs::write(
        case.intent().join("sysctl.toml"),
        "[values]\n\"vm.swappiness\" = \"150\"\n\"kernel.sysrq\" = \"190\"\n",
    )
    .unwrap();
    case.write_live("/proc/sys/vm/swappiness", "150\n");
    case.write_live("/proc/sys/kernel/sysrq", "190\n");

    let intent = case.load();
    let host = case.host();
    let plan = converge::plan(&intent, &host).unwrap();
    // Runtime values already match, but the projection file is missing: that alone is a change.
    assert_eq!(plan.changes.len(), 1, "{plan:?}");
    converge::apply(&intent, &host, false).unwrap();

    let rendered = case.read_live(converge::RENDERED_SYSCTL_CONF);
    assert!(rendered.starts_with("# Generated by genarch"), "{rendered}");
    assert!(rendered.contains("kernel.sysrq = 190"), "{rendered}");
    assert!(rendered.contains("vm.swappiness = 150"), "{rendered}");
    // Sorted: the file must not churn between applies.
    let sysrq = rendered.find("kernel.sysrq").unwrap();
    let swappiness = rendered.find("vm.swappiness").unwrap();
    assert!(sysrq < swappiness, "keys are not sorted: {rendered}");
    assert!(converge::plan(&intent, &host).unwrap().is_empty());
}

#[test]
fn a_path_cannot_be_declared_by_two_owners() {
    // A whole-file write and a line patch on the same path fight over the same bytes, and a
    // mirror the engine owns while a managed record patches it can never converge.
    let case = Case::new("one-owner");
    case.write_intent_meta();
    case.write_intent_mirror("/etc/dup.conf", "x\n");
    case.write_managed(
        "dup",
        "name = \"dup\"\ntarget = \"/etc/dup.conf\"\nmatch = '^k = '\nline = \"k = 1\"\n",
    );

    let err = Intent::load(&case.intent(), &case.dir.join("no-overlay")).unwrap_err();
    assert!(err.message.contains("one path, one owner"), "{}", err.message);
}

#[test]
fn a_pass_stands_down_while_an_apply_holds_the_lock() {
    // The 2-minute ceil timer and an operator's apply both write kernel attributes. Without a
    // lock the last writer wins silently; with it, the pass says it skipped instead.
    let case = Case::new("pass-lock");
    case.write_intent_meta();

    let intent = case.load();
    let host = case.host();
    let held = host.lock(converge::LOCK_PATH).unwrap();

    let report = genarch::pass::run(&host, &intent, genarch::pass::Pass::Ceil).unwrap();
    assert!(
        report.lines.iter().any(|line| line.contains("pass skipped")),
        "{:?}",
        report.lines
    );

    drop(held);
    let report = genarch::pass::run(&host, &intent, genarch::pass::Pass::Ceil).unwrap();
    assert!(
        !report.lines.iter().any(|line| line.contains("pass skipped")),
        "{:?}",
        report.lines
    );
}
