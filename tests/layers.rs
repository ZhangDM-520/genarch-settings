//! The layer seam: precedence, per-key merge, tombstones and the profile contract.
//!
//! Each case here is a property the single-root intent could not express: a user changing
//! three sysctls without copying sixteen, disabling a shipped mirror without forking, and a
//! profile that says which machine it belongs to.

use std::path::{Path, PathBuf};

use genarch::check::{self, Class, Ctx};
use genarch::host::{FixtureHost, Host};
use genarch::intent::{Intent, SCHEMA_VERSION};

struct Tree {
    dir: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Tree {
        let dir = std::env::temp_dir().join(format!("genarch-layers-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("base")).unwrap();
        std::fs::create_dir_all(dir.join("etc")).unwrap();
        Tree { dir }.write("intent.toml", &format!("schema = {SCHEMA_VERSION}\n"))
    }

    fn base(&self) -> PathBuf {
        self.dir.join("base")
    }

    fn overlay(&self) -> PathBuf {
        self.dir.join("etc")
    }

    /// Write under the base layer.
    ///
    /// The builders consume and return the tree so no intermediate value is dropped: the
    /// cleanup lives in `Drop`, and a `&self` builder would have destroyed the tree it built.
    fn write(mut self, rel: &str, body: &str) -> Tree {
        self.put(&self.base(), rel, body);
        self
    }

    /// Write under the operator layer.
    fn write_overlay(mut self, rel: &str, body: &str) -> Tree {
        self.put(&self.overlay(), rel, body);
        self
    }

    /// Write under `profiles/<name>/` of the base layer.
    fn write_profile(mut self, name: &str, rel: &str, body: &str) -> Tree {
        let root = self.base().join("profiles").join(name);
        self.put(&root, rel, body);
        self
    }

    fn put(&mut self, root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body).unwrap();
    }

    fn select(mut self, name: &str) -> Tree {
        self.put(&self.overlay(), "profile", &format!("profile = \"{name}\"\n"));
        self
    }

    fn host(&self) -> FixtureHost {
        FixtureHost::new(self.dir.join("live"))
    }

    fn load(&self) -> genarch::Result<Intent> {
        Intent::load(&self.base(), &self.overlay())
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_layer_merges_a_scalar_map_per_key_and_the_operator_wins() {
    let tree = Tree::new("sysctl-merge")
        .write("sysctl.toml", "[values]\n\"vm.a\" = \"1\"\n\"vm.b\" = \"2\"\n\"vm.c\" = \"3\"\n")
        .write_profile("p1", "intent.toml", "[requires]\n")
        .write_profile("p1", "sysctl.toml", "[values]\n\"vm.b\" = \"20\"\n")
        .write_overlay("sysctl.toml", "[values]\n\"vm.c\" = \"300\"\n")
        .select("p1");

    let intent = tree.load().unwrap();
    let values: Vec<(&str, &str)> =
        intent.sysctl.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(values, vec![("vm.a", "1"), ("vm.b", "20"), ("vm.c", "300")]);
}

#[test]
fn a_later_layer_overrides_a_mirror_and_a_tombstone_removes_one() {
    let tree = Tree::new("mirror-layers")
        .write("mirror/etc/base.conf", "base\n")
        .write("mirror/etc/unwanted.conf", "shipped\n")
        .write_overlay("mirror/etc/base.conf", "operator\n")
        .write_profile(
            "p1",
            "intent.toml",
            "[omit]\nmirror = [\"/etc/unwanted.conf\"]\n",
        )
        .select("p1");

    let intent = tree.load().unwrap();
    let live: Vec<(&str, &[u8])> =
        intent.mirror.iter().map(|m| (m.live.as_str(), m.bytes.as_slice())).collect();
    assert_eq!(live, vec![("/etc/base.conf", b"operator\n".as_slice())]);
}

#[test]
fn a_tombstone_that_names_nothing_is_refused() {
    // A tombstone is a statement about a file the engine ships. When it matches nothing it is
    // a typo or a stale override, and ignoring it would leave the file deployed.
    let tree = Tree::new("bad-tombstone")
        .write("mirror/etc/kept.conf", "x\n")
        .write_overlay("intent.toml", "[omit]\nmirror = [\"/etc/not-shipped.conf\"]\n");

    let err = tree.load().unwrap_err();
    assert!(err.message.contains("no layer ships"), "{}", err.message);

    let tree = Tree::new("bad-managed-tombstone")
        .write_overlay("intent.toml", "[omit]\nmanaged = [\"ghost\"]\n");
    let err = tree.load().unwrap_err();
    assert!(err.message.contains("no layer declares"), "{}", err.message);
}

#[test]
fn a_managed_record_is_replaced_by_name_or_removed_by_its_name() {
    let record = |line: &str| {
        format!(
            "name = \"j\"\ntarget = \"/etc/j.conf\"\nmatch = '^k = '\nline = \"{line}\"\n"
        )
    };
    let tree = Tree::new("managed-layers")
        .write("managed/j.toml", &record("k = 1"))
        .write("managed/keep.toml", &record("k = 9"))
        .write_profile("p1", "managed/j.toml", &record("k = 2"))
        .select("p1");

    let intent = tree.load().unwrap();
    let j = intent.managed.iter().find(|r| r.name == "j").unwrap();
    assert_eq!(j.line, "k = 2", "a later layer replaces the whole record, not its keys");

    let tree = Tree::new("managed-omit")
        .write("managed/j.toml", &record("k = 1"))
        .write_overlay("intent.toml", "[omit]\nmanaged = [\"j\"]\n");
    let intent = tree.load().unwrap();
    assert!(intent.managed.is_empty(), "a tombstone removes the record");
}

#[test]
fn lists_replace_wholesale_and_scalars_merge_per_key() {
    let tree = Tree::new("passes-merge")
        .write(
            "passes.toml",
            "wake_disarm = [\"GPP0\"]\nusb_autosuspend = [\"13d3:3608\"]\nidle_governor = \"teo\"\n",
        )
        .write_profile(
            "p1",
            "passes.toml",
            "wake_disarm = [\"XHC0\"]\nsettle_secs = 2\n",
        )
        .select("p1");

    let intent = tree.load().unwrap();
    assert_eq!(intent.passes.wake_disarm, Some(vec!["XHC0".to_string()]));
    // The profile said nothing about these, so the layer below keeps them.
    assert_eq!(intent.passes.usb_autosuspend, Some(vec!["13d3:3608".to_string()]));
    assert_eq!(intent.passes.idle_governor.as_deref(), Some("teo"));
    assert_eq!(intent.passes.settle_secs, Some(2));
}

#[test]
fn no_profile_selected_means_base_only_and_no_guessed_host_defaults() {
    let tree = Tree::new("base-only")
        .write("sysctl.toml", "[values]\n\"vm.a\" = \"1\"\n")
        .write("mirror/etc/base.conf", "x\n")
        .write("intent.toml", &format!("schema = {SCHEMA_VERSION}\n\n[units]\nenabled = [\"genarch.service\"]\n"));

    let intent = tree.load().unwrap();
    assert!(intent.profile.is_none());
    assert!(intent.ceil.is_none(), "no ceiling may be guessed");
    assert!(intent.passes.wake_disarm.is_none(), "no wake list may be guessed");
    assert!(intent.ryzenadj.co.all.is_none(), "no CO offset may be guessed");
    assert_eq!(intent.mirror.len(), 1);
    assert_eq!(intent.units, vec!["genarch.service".to_string()]);

    // And the rows that depend on a profile say so instead of inventing a verdict.
    let host = tree.host();
    let verdicts = check::evaluate(&Ctx { host: &host, intent: &intent });
    let class = |id: &str| verdicts.iter().find(|v| v.id == id).unwrap().class;
    assert_eq!(class("PROFILE"), Class::Skipped);
    assert_eq!(class("CEIL"), Class::Skipped);
    assert_eq!(class("CO"), Class::Skipped);
    assert_eq!(class("POWER"), Class::Skipped);
}

#[test]
fn a_profile_requirement_is_profile_drift_not_a_refusal_to_load() {
    let tree = Tree::new("requires")
        .write_profile(
            "p1",
            "intent.toml",
            "[requires]\nproduct = \"Right Machine\"\nsys_vendor = \"Vendor\"\n",
        )
        .select("p1");
    let host = tree.host();
    std::fs::create_dir_all(tree.dir.join("live/sys/class/dmi/id")).unwrap();
    std::fs::write(tree.dir.join("live/sys/class/dmi/id/product_name"), "Right Machine\n").unwrap();
    std::fs::write(tree.dir.join("live/sys/class/dmi/id/sys_vendor"), "Vendor\n").unwrap();

    let intent = tree.load().unwrap();
    assert_eq!(intent.profile.as_ref().unwrap().name, "p1");
    let verdicts = check::evaluate(&Ctx { host: &host, intent: &intent });
    let profile = verdicts.iter().find(|v| v.id == "PROFILE").unwrap();
    assert_eq!(profile.class, Class::Ok, "{profile:?}");

    // The same intent on another laptop: still loads, and the report says why it is wrong.
    std::fs::write(tree.dir.join("live/sys/class/dmi/id/product_name"), "Other Machine\n").unwrap();
    std::fs::write(tree.dir.join("live/sys/class/dmi/id/sys_vendor"), "Vendor\n").unwrap();
    let intent = tree.load().unwrap();
    let verdicts = check::evaluate(&Ctx { host: &host, intent: &intent });
    let profile = verdicts.iter().find(|v| v.id == "PROFILE").unwrap();
    assert_eq!(profile.class, Class::Drift, "{profile:?}");
    assert!(profile.detail.contains("product"), "{}", profile.detail);
}

#[test]
fn only_a_profile_may_declare_requires_and_only_known_facts() {
    let tree = Tree::new("requires-scope")
        .write("intent.toml", &format!("schema = {SCHEMA_VERSION}\n[requires]\nproduct = \"X\"\n"));
    let err = tree.load().unwrap_err();
    assert!(err.message.contains("only a profile"), "{}", err.message);

    let tree = Tree::new("requires-unknown").write_profile(
        "p1",
        "intent.toml",
        "[requires]\nfirmware = \"X\"\n",
    );
    let tree = tree.select("p1");
    let err = tree.load().unwrap_err();
    assert!(err.message.contains("not a machine fact"), "{}", err.message);
}

#[test]
fn deselecting_a_profile_does_not_unmanage_its_mirrors() {
    // Ownership survives selection. Otherwise a lost `/etc/genarch/profile` would read the
    // profile's mirrors as withdrawn and prune them — `/etc/fstab` included.
    let tree = Tree::new("dormant")
        .write("mirror/etc/base.conf", "base\n")
        .write_profile("p1", "mirror/etc/fstab", "uuid=1\n")
        .select("p1");
    let home = |tree: &Tree| FixtureHost::new(tree.dir.join("live"));
    let host = home(&tree);
    let selected = tree.load().unwrap();
    genarch::converge::apply(&selected, &host, false).unwrap();
    assert!(host.exists("/etc/fstab"));

    // The selection is gone: the base intent no longer ships /etc/fstab, but the profile that
    // does is still on disk.
    std::fs::remove_file(tree.overlay().join("profile")).unwrap();
    let base_only = tree.load().unwrap();
    assert!(base_only.dormant.contains("/etc/fstab"), "{:?}", base_only.dormant);
    let plan = genarch::converge::plan(&base_only, &host).unwrap();
    assert!(
        !plan.changes.iter().any(|c| matches!(c, genarch::converge::Planned::Prune { .. })),
        "a deselected profile must not hand its files back: {plan:?}"
    );
    // The manifest keeps the path owned, so reselecting does not look like a new write.
    let manifest = genarch::converge::Manifest::load(&host).unwrap();
    assert!(manifest.owned.contains("/etc/fstab"), "{manifest:?}");

    // A profile that actually stops shipping the path is a different statement: it is pruned.
    std::fs::remove_dir_all(tree.base().join("profiles/p1")).unwrap();
    let withdrawn = tree.load().unwrap();
    assert!(withdrawn.dormant.is_empty());
    let plan = genarch::converge::plan(&withdrawn, &host).unwrap();
    assert!(
        plan.changes
            .iter()
            .any(|c| matches!(c, genarch::converge::Planned::Prune { live } if live == "/etc/fstab")),
        "{plan:?}"
    );
}

#[test]
fn ceiling_classes_come_from_the_profile_layer() {
    let tree = Tree::new("ceil-data")
        .write_profile(
            "p1",
            "ceil.toml",
            "[[class]]\nname = \"fast\"\ncpus = [\"0-3\"]\nkhz = 4700000\n\
             [[class]]\nname = \"slow\"\ncpus = [\"4-7\"]\nkhz = 3200000\n",
        )
        .select("p1");
    let intent = tree.load().unwrap();
    let classes = intent.ceil.as_ref().unwrap();

    let host = tree.host();
    let policy = "/sys/devices/system/cpu/cpufreq/policy0";
    std::fs::create_dir_all(tree.dir.join("live/sys/devices/system/cpu/cpufreq/policy0")).unwrap();
    std::fs::write(tree.dir.join("live/sys/devices/system/cpu/cpufreq/policy0/related_cpus"), "0 1 2 3\n")
        .unwrap();
    assert_eq!(
        genarch::ceil::want_for_policy(&host, classes, policy).unwrap(),
        Some(4_700_000)
    );
}

#[test]
fn the_shipped_intent_loads_as_base_plus_the_selected_profile() {
    // The repository tree is what the package ships: a base with no machine facts, and one
    // profile that carries this machine's. If this stops loading, the release is broken.
    let dir = std::env::temp_dir().join(format!("genarch-shipped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("selected")).unwrap();
    std::fs::create_dir_all(dir.join("bare")).unwrap();
    std::fs::write(dir.join("selected/profile"), "profile = \"zenbook-s16\"\n").unwrap();

    let intent = Intent::load(Path::new("intent"), &dir.join("selected")).unwrap();
    let profile = intent.profile.as_ref().expect("the shipped profile must load");
    assert_eq!(profile.name, "zenbook-s16");
    assert!(profile.requires.contains_key("product"), "the profile must pin its machine");
    assert!(intent.ceil.is_some(), "the profile must declare the ceiling classes");
    assert!(intent.ryzenadj.co.all.is_some(), "the profile must carry the silicon offsets");
    assert!(intent.passes.wifi_interface.is_some(), "the profile must name the interface");
    assert_eq!(intent.units.len(), 2, "the base declares the engine's own units");
    assert_eq!(intent.smu.fast.as_deref(), Some("PPT LIMIT FAST"));

    // The machine facts must not have leaked into the base layer: those live files are what
    // make a package non-portable, so the base seen alone must not carry one.
    let base = Intent::load(Path::new("intent"), &dir.join("bare")).unwrap();
    assert!(base.profile.is_none(), "no selection means base only");
    let live: Vec<&str> = base.mirror.iter().map(|m| m.live.as_str()).collect();
    assert!(!live.contains(&"/etc/fstab"), "a disk UUID is not a base fact: {live:?}");
    assert!(
        !live.iter().any(|path| path.starts_with("/home/")),
        "a user path is not a base fact: {live:?}"
    );
    assert!(
        base.managed.iter().all(|r| !r.target.contains("/home/")),
        "a user path is not a base fact"
    );
    assert!(base.ceil.is_none(), "a ceiling is a measurement, not a base fact");
    assert!(
        base.ryzenadj.co.all.is_none(),
        "a curve offset is a measurement, not a base fact"
    );
    assert!(
        base.passes.wifi_interface.is_none(),
        "an interface name is a machine fact"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
