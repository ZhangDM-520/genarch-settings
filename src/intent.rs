//! Intent: what the host should look like, resolved from layers.
//!
//! Hybrid by design. Anything the engine *computes* is typed TOML, so a value has one
//! home and can be validated; anything it must not *interpret* is a byte-exact mirror
//! file, so the engine cannot silently rewrite a vendor file's meaning.
//!
//! Three layers, one intent, precedence `base → profile → operator`:
//!
//! - **base** (`/usr/lib/genarch`) — what is true of any host this engine serves. No disk
//!   UUID, no user path, no silicon measurement.
//! - **profile** (`{base,overlay}/profiles/<name>`) — the machine layer, selected by
//!   `profile = "<name>"` in `/etc/genarch/profile`. Empty is legal: with no profile the
//!   engine applies the base and the rows that depend on a profile report `SKIPPED`.
//! - **operator** (`/etc/genarch`) — the last word, always.
//!
//! Merge rules are per class, because "replace the whole file" made the cheapest
//! customisation a fork: scalar maps merge per key (`sysctl`, `ryzenadj`, `passes`), lists
//! replace wholesale, mirrors and managed records are whole-file with an explicit
//! `[omit]` tombstone in a layer's `intent.toml`.
//!
//! A layer's `intent.toml` also carries `[requires]` (profile only): machine facts the
//! profile is written for. A mismatch is the report's `PROFILE` drift, never a refusal to
//! load — the operator must still be able to see the drift and fix the selection.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ceil::CeilConfig;
use crate::exit::{Failure, Result};
use crate::host::Host;
use crate::tree;

/// Schema version. Bumped when a field's meaning changes; the loader refuses an intent
/// it does not understand rather than guessing.
///
/// 3: layers and profiles replace the single-root intent, and the declared `prune` list is
/// gone (the manifest is the ownership record). A 2 intent is refused with that reason.
pub const SCHEMA_VERSION: u32 = 3;

/// A file deployed byte for byte. The engine owns it; it does not understand it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorTarget {
    /// Absolute live path, e.g. `/etc/tlp.d/01-eco.conf`.
    pub live: String,
    pub bytes: Vec<u8>,
    /// Mode applied only when the live target does not exist yet.
    pub mode: u32,
    /// Where the bytes came from, for error messages and `harvest`.
    pub origin: PathBuf,
}

/// One anchored line inside a foreign file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRecord {
    /// Derived from the file stem when absent, which is what the shell records rely on.
    #[serde(default)]
    pub name: String,
    pub target: String,
    #[serde(rename = "match")]
    pub match_expr: String,
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// Declarative ryzenadj settings: each field is a knob, not an argv fragment. The raw
/// list exists so a flag the engine has no field for is still expressible — it is applied
/// last and logged, never silently dropped.
///
/// Every field is optional because this type is also the per-layer partial: `None` means
/// "this layer says nothing", which is what lets a profile override one limit and keep the
/// rest of the table below it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RyzenadjConfig {
    #[serde(skip_serializing_if = "is_default")]
    pub co: CoConfig,
    #[serde(skip_serializing_if = "is_default")]
    pub limits: LimitsConfig,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub raw: Vec<RawArgs>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoConfig {
    /// Global curve-optimizer offset, the `--set-coall` value. `Some(0)` is a declaration;
    /// `None` means the layer does not mention it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all: Option<i32>,
    /// Per-core offsets, index = core number. A list replaces the previous one wholesale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_core: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stapm: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stapm_time: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow_time: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_saving: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawArgs {
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// What the scheduled passes do, declared once instead of hardcoded in a script.
///
/// Every list here used to be a literal inside `eco-power.sh`, then a default in this binary.
/// Keeping them in a profile means the boot pass can be reviewed, and a host whose profile
/// does not mention a knob does not get that behaviour — no fallback silently applies a
/// Zenbook's wake list to another laptop.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PassesConfig {
    /// ACPI wake sources to disarm at boot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wake_disarm: Option<Vec<String>>,
    /// USB devices to allow autosuspending, as `vid:pid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb_autosuspend: Option<Vec<String>>,
    /// cpuidle governor; `teo` predicts idle states better and sleeps deeper, more often.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_governor: Option<String>,
    /// Wireless interface for the power-save re-assert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi_interface: Option<String>,
    /// Seconds between the two SMU passes of the boot pass, so the second lands after the
    /// event handlers that reset firmware limits. Never used on resume: a sleep hook that
    /// sleeps stalls the user.slice thaw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_secs: Option<u64>,
}

/// The `ryzenadj --info` field labels, declared rather than compiled in.
///
/// They are the tool's output format, not this host's fact, but a rewrite that hardcodes them
/// cannot be pointed at a different ryzenadj. A missing label means "this intent does not
/// judge that field", never a guessed string.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SmuFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stapm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow: Option<String>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

/// The selected machine layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    /// Machine facts the profile is written for, as `(fact, expected value)`.
    pub requires: BTreeMap<String, String>,
}

/// The resolved intent.
#[derive(Debug, Clone, Default)]
pub struct Intent {
    pub schema: u32,
    /// Base root the intent was loaded from (`/usr/lib/genarch`).
    pub base: PathBuf,
    /// Override root (`/etc/genarch`), whether or not it exists.
    pub overlay: PathBuf,
    /// The selected profile, when `/etc/genarch/profile` names one.
    pub profile: Option<Profile>,
    /// Runtime sysctl values, key → value. Multi-word values keep their spaces.
    pub sysctl: BTreeMap<String, String>,
    pub ryzenadj: RyzenadjConfig,
    pub passes: PassesConfig,
    /// Ceiling classes, when a layer declares them. `None` means this host declares no
    /// ceiling policy, and CEIL reports `SKIPPED` rather than guessing one.
    pub ceil: Option<CeilConfig>,
    /// Units the report expects enabled, from `[units]` in a layer.
    pub units: Vec<String>,
    /// `ryzenadj --info` labels for the SMU row, from `[smu]` in a layer.
    pub smu: SmuFields,
    pub mirror: Vec<MirrorTarget>,
    pub managed: Vec<ManagedRecord>,
    /// Live paths a profile this intent did **not** select still ships.
    ///
    /// Deselecting a profile must not unmanage its files: without this set, a host whose
    /// selection was lost would read the profile's mirrors as "no longer shipped" and prune
    /// them — including `/etc/fstab`, which is not a file to delete because a config line went
    /// missing. Ownership survives selection; only a profile that stops shipping a path hands
    /// it back.
    pub dormant: BTreeSet<String>,
}

/// One layer's small typed config (`intent.toml` inside its root).
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Meta {
    schema: Option<u32>,
    omit: Omit,
    requires: BTreeMap<String, String>,
    units: Units,
    smu: SmuFields,
}

/// `[units] enabled = [...]`: the units the report expects enabled.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Units {
    enabled: Vec<String>,
}

/// Tombstones: mirrors and managed records this layer removes from the layers below it.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Omit {
    mirror: Vec<String>,
    managed: Vec<String>,
}

struct Layer {
    root: PathBuf,
    meta: Meta,
}

/// `/etc/genarch/profile`: which machine layer to select.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Selection {
    profile: Option<String>,
}

impl Intent {
    /// Load from the standard locations, honouring `GENARCH_INTENT` for a rehearsal copy.
    pub fn load_default(host: &dyn Host) -> Result<Intent> {
        let base = std::env::var_os("GENARCH_INTENT")
            .map(PathBuf::from)
            .unwrap_or_else(|| host.resolve("/usr/lib/genarch"));
        let overlay = host.resolve("/etc/genarch");
        Intent::load(&base, &overlay)
    }

    pub fn load(base: &Path, overlay: &Path) -> Result<Intent> {
        let base_meta_path = base.join("intent.toml");
        let base_meta = read_meta(&base_meta_path, true)?;
        match base_meta.schema {
            Some(SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(Failure::precondition(format!(
                    "intent schema {other} at {} but this engine speaks {SCHEMA_VERSION}",
                    base_meta_path.display()
                )));
            }
            None => {
                return Err(Failure::precondition(format!(
                    "{} declares no schema; a v2 intent's `prune` list is not read any more \
                     (the manifest owns pruning) — rewrite the intent for schema {SCHEMA_VERSION}",
                    base_meta_path.display()
                )));
            }
        }
        if !base_meta.requires.is_empty() {
            return Err(Failure::precondition(format!(
                "{}: only a profile may declare [requires]",
                base_meta_path.display()
            )));
        }

        let mut layers = vec![Layer { root: base.to_path_buf(), meta: base_meta }];
        let mut profile = None;
        if let Some(name) = selection(overlay)? {
            let mut requires = BTreeMap::new();
            let mut selected = false;
            // A profile may live in the package or be the operator's own; both are layers, so
            // an operator adding one file to a package profile does not shadow the whole tree.
            let candidates = [
                base.join("profiles").join(&name),
                overlay.join("profiles").join(&name),
            ];
            for root in candidates {
                if !root.is_dir() {
                    continue;
                }
                let meta = read_meta(&root.join("intent.toml"), false)?;
                optional_schema(&meta, &root.join("intent.toml"))?;
                requires.extend(meta.requires.iter().map(|(k, v)| (k.clone(), v.clone())));
                layers.push(Layer { root, meta });
                selected = true;
            }
            if !selected {
                return Err(Failure::precondition(format!(
                    "profile '{name}' is selected in {} but no profiles/{name} exists under {} \
                     or {}",
                    overlay.join("profile").display(),
                    base.display(),
                    overlay.display()
                )));
            }
            profile = Some(Profile { name, requires });
        }

        let overlay_meta_path = overlay.join("intent.toml");
        let overlay_meta = read_meta(&overlay_meta_path, false)?;
        optional_schema(&overlay_meta, &overlay_meta_path)?;
        if !overlay_meta.requires.is_empty() {
            return Err(Failure::precondition(format!(
                "{}: only a profile may declare [requires]",
                overlay_meta_path.display()
            )));
        }
        layers.push(Layer { root: overlay.to_path_buf(), meta: overlay_meta });

        let omit = Omit {
            mirror: layers.iter().flat_map(|l| l.meta.omit.mirror.iter().cloned()).collect(),
            managed: layers.iter().flat_map(|l| l.meta.omit.managed.iter().cloned()).collect(),
        };

        let mut intent = Intent {
            schema: SCHEMA_VERSION,
            base: base.to_path_buf(),
            overlay: overlay.to_path_buf(),
            profile,
            ..Intent::default()
        };
        intent.sysctl = load_sysctl(&layers)?;
        intent.ryzenadj = load_ryzenadj(&layers)?;
        intent.passes = load_passes(&layers)?;
        intent.ceil = load_ceil(&layers)?;
        intent.units = load_units(&layers);
        intent.smu = load_smu(&layers);
        intent.mirror = load_mirror(&layers, &omit.mirror)?;
        intent.managed = load_managed(&layers, &omit.managed)?;
        intent.dormant = dormant_mirrors(base, overlay, &layers);
        intent.validate()?;
        Ok(intent)
    }

    fn validate(&self) -> Result<()> {
        // A path may have one *kind* of owner. Mirror + managed on the same path meant a
        // whole-file write and a line patch fighting over the same bytes. Several managed
        // records on one target stay legal — that is how `asusd.ron` is patched at five
        // anchors.
        let mirror: BTreeSet<&str> = self.mirror.iter().map(|m| m.live.as_str()).collect();
        let managed: BTreeSet<&str> = self.managed.iter().map(|r| r.target.as_str()).collect();
        if let Some(path) = mirror.intersection(&managed).next() {
            return Err(Failure::precondition(format!(
                "{path} is declared as both mirror and managed: one path, one owner"
            )));
        }
        for target in &self.mirror {
            if !target.live.starts_with('/') {
                return Err(Failure::precondition(format!(
                    "mirror target is not absolute: {}",
                    target.live
                )));
            }
            // A mirror whose name marks it a backup is almost certainly a stray that would
            // be deployed — and in system-sleep/ executed on every resume.
            let base = target.live.rsplit('/').next().unwrap_or_default();
            if !tree::is_deployable_name(base) {
                return Err(Failure::precondition(format!(
                    "mirror holds a non-deployable file name: {}",
                    target.live
                )));
            }
        }
        for record in &self.managed {
            if record.match_expr.trim().is_empty() {
                return Err(Failure::precondition(format!(
                    "managed '{}': empty match expression",
                    record.name
                )));
            }
        }
        // Two records patching the same file is legal (asusd has five); two records with
        // the same name is not — the second would silently replace the first.
        let names: BTreeSet<&str> = self.managed.iter().map(|r| r.name.as_str()).collect();
        if names.len() != self.managed.len() {
            return Err(Failure::precondition(
                "duplicate managed record name — one record would silently replace another",
            ));
        }
        if let Some(profile) = &self.profile {
            for fact in profile.requires.keys() {
                if machine_fact_path(fact).is_none() {
                    return Err(Failure::precondition(format!(
                        "profile '{}' requires '{fact}', which is not a machine fact this engine \
                         can read ({})",
                        profile.name,
                        MACHINE_FACTS.join(", ")
                    )));
                }
            }
        }
        Ok(())
    }

    /// Every managed record whose target is this live path.
    pub fn managed_for(&self, live: &str) -> Vec<&ManagedRecord> {
        self.managed.iter().filter(|r| r.target == live).collect()
    }

    /// Machine facts the selected profile requires but the host does not match.
    ///
    /// Empty when there is no profile or it declares no requirement. This is the `PROFILE`
    /// drift and nothing else: the intent still loads and still applies.
    pub fn profile_mismatch(&self, host: &dyn Host) -> Vec<String> {
        let Some(profile) = &self.profile else { return Vec::new() };
        let mut out = Vec::new();
        for (fact, want) in &profile.requires {
            match machine_fact(host, fact) {
                Some(live) if live.trim() == want.trim() => {}
                Some(live) => {
                    out.push(format!("{fact} is {:?}, profile requires {want}", live.trim()))
                }
                None => out.push(format!("{fact} is unreadable, profile requires {want}")),
            }
        }
        out
    }
}

/// The machine facts a profile may require, mapped to the DMI attribute that carries them.
pub const MACHINE_FACTS: &[&str] = &["product", "sys_vendor", "board", "bios_version"];

fn machine_fact_path(fact: &str) -> Option<&'static str> {
    match fact {
        "product" => Some("/sys/class/dmi/id/product_name"),
        "sys_vendor" => Some("/sys/class/dmi/id/sys_vendor"),
        "board" => Some("/sys/class/dmi/id/board_name"),
        "bios_version" => Some("/sys/class/dmi/id/bios_version"),
        _ => None,
    }
}

fn machine_fact(host: &dyn Host, fact: &str) -> Option<String> {
    host.read_opt(machine_fact_path(fact)?)
}

fn read_meta(path: &Path, required: bool) -> Result<Meta> {
    if !path.is_file() {
        if required {
            return Err(Failure::precondition(format!(
                "no intent at {} — is the package installed?",
                path.display()
            )));
        }
        return Ok(Meta::default());
    }
    let text = read_text(path)?;
    parse(&text, path)
}

/// A non-base layer may omit `schema`; when it declares one it must speak ours.
fn optional_schema(meta: &Meta, path: &Path) -> Result<()> {
    match meta.schema {
        Some(SCHEMA_VERSION) | None => Ok(()),
        Some(other) => Err(Failure::precondition(format!(
            "intent schema {other} at {} but this engine speaks {SCHEMA_VERSION}",
            path.display()
        ))),
    }
}

fn selection(overlay: &Path) -> Result<Option<String>> {
    let path = overlay.join("profile");
    if !path.is_file() {
        return Ok(None);
    }
    let selection: Selection = parse(&read_text(&path)?, &path)?;
    match selection.profile {
        Some(name) if !name.trim().is_empty() => Ok(Some(name.trim().to_string())),
        _ => Err(Failure::precondition(format!(
            "{} names no profile: expected `profile = \"<name>\"`",
            path.display()
        ))),
    }
}

fn layer_file(layer: &Layer, name: &str) -> Option<PathBuf> {
    let path = layer.root.join(name);
    path.is_file().then_some(path)
}

fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str, path: &Path) -> Result<T> {
    toml::from_str(text)
        .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))
}

fn load_sysctl(layers: &[Layer]) -> Result<BTreeMap<String, String>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        #[serde(default)]
        values: BTreeMap<String, toml::Value>,
    }
    let mut out = BTreeMap::new();
    for layer in layers {
        let Some(path) = layer_file(layer, "sysctl.toml") else { continue };
        let file: File = parse(&read_text(&path)?, &path)?;
        for (key, value) in file.values {
            let rendered = render_sysctl_value(&key, value, &path)?;
            out.insert(key, rendered);
        }
    }
    Ok(out)
}

/// A sysctl value may be a word, an integer or a list of words; all render to one line.
fn render_sysctl_value(key: &str, value: toml::Value, path: &Path) -> Result<String> {
    let rendered = match value {
        toml::Value::String(s) => s,
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Array(items) => items
            .iter()
            .map(|v| match v {
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::String(s) => s.clone(),
                other => other.to_string().trim_matches('"').to_string(),
            })
            .collect::<Vec<_>>()
            .join(" "),
        toml::Value::Table(_) => {
            return Err(Failure::precondition(format!(
                "sysctl {key}: a dotted key became a table in {} — quote it, \
                 e.g. \"kernel.printk\" = \"3 3 3 3\"",
                path.display()
            )));
        }
        _other => {
            return Err(Failure::precondition(format!(
                "sysctl {key}: unsupported value type in {}",
                path.display()
            )));
        }
    };
    if key.trim().is_empty() {
        return Err(Failure::precondition(format!(
            "sysctl: empty key in {}",
            path.display()
        )));
    }
    Ok(rendered.trim().to_string())
}

fn load_ryzenadj(layers: &[Layer]) -> Result<RyzenadjConfig> {
    let mut out = RyzenadjConfig::default();
    for layer in layers {
        let Some(path) = layer_file(layer, "ryzenadj.toml") else { continue };
        let one: RyzenadjConfig = parse(&read_text(&path)?, &path)?;
        // Scalars merge per key; a list replaces the layer below wholesale.
        out.co.all = one.co.all.or(out.co.all);
        out.co.per_core = one.co.per_core.or(out.co.per_core);
        out.limits.stapm = one.limits.stapm.or(out.limits.stapm);
        out.limits.fast = one.limits.fast.or(out.limits.fast);
        out.limits.slow = one.limits.slow.or(out.limits.slow);
        out.limits.stapm_time = one.limits.stapm_time.or(out.limits.stapm_time);
        out.limits.slow_time = one.limits.slow_time.or(out.limits.slow_time);
        out.limits.power_saving = one.limits.power_saving.or(out.limits.power_saving);
        if !one.raw.is_empty() {
            out.raw = one.raw;
        }
    }
    Ok(out)
}

fn load_passes(layers: &[Layer]) -> Result<PassesConfig> {
    let mut out = PassesConfig::default();
    for layer in layers {
        let Some(path) = layer_file(layer, "passes.toml") else { continue };
        let one: PassesConfig = parse(&read_text(&path)?, &path)?;
        out.wake_disarm = one.wake_disarm.or(out.wake_disarm);
        out.usb_autosuspend = one.usb_autosuspend.or(out.usb_autosuspend);
        out.idle_governor = one.idle_governor.or(out.idle_governor);
        out.wifi_interface = one.wifi_interface.or(out.wifi_interface);
        out.settle_secs = one.settle_secs.or(out.settle_secs);
    }
    Ok(out)
}

/// Units are unioned rather than replaced: a profile adds the units it expects enabled and
/// cannot accidentally drop the engine's own pair from the report.
fn load_units(layers: &[Layer]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for unit in layers.iter().flat_map(|l| l.meta.units.enabled.iter()) {
        if seen.insert(unit.clone()) {
            out.push(unit.clone());
        }
    }
    out
}

fn load_smu(layers: &[Layer]) -> SmuFields {
    let mut out = SmuFields::default();
    for layer in layers {
        out.stapm = layer.meta.smu.stapm.clone().or(out.stapm);
        out.fast = layer.meta.smu.fast.clone().or(out.fast);
        out.slow = layer.meta.smu.slow.clone().or(out.slow);
    }
    out
}

fn load_ceil(layers: &[Layer]) -> Result<Option<CeilConfig>> {
    let mut out = None;
    for layer in layers {
        let Some(path) = layer_file(layer, "ceil.toml") else { continue };
        // A class list replaces the one below: a profile that declares ceilings owns the rule.
        out = Some(CeilConfig::parse(&read_text(&path)?, &path)?);
    }
    Ok(out)
}

/// Mirror paths shipped by profile layers that are not selected.
///
/// A profile is a directory under `profiles/`; the selected ones are already layers, so
/// everything else with a `mirror/` under it is dormant ownership, not a dropped target.
fn dormant_mirrors(base: &Path, overlay: &Path, layers: &[Layer]) -> BTreeSet<String> {
    let selected: BTreeSet<&Path> = layers.iter().map(|l| l.root.as_path()).collect();
    let mut out = BTreeSet::new();
    for profiles in [base.join("profiles"), overlay.join("profiles")] {
        let Ok(entries) = std::fs::read_dir(&profiles) else { continue };
        for entry in entries.flatten() {
            let root = entry.path();
            if !root.is_dir() || selected.contains(root.as_path()) {
                continue;
            }
            let mirror = root.join("mirror");
            for path in tree::walk(&mirror) {
                if let Some(live) = tree::live_path(&mirror, &path) {
                    out.insert(live);
                }
            }
        }
    }
    out
}

fn load_mirror(layers: &[Layer], omit: &[String]) -> Result<Vec<MirrorTarget>> {
    let mut shipped: BTreeSet<String> = BTreeSet::new();
    let mut by_live: BTreeMap<String, MirrorTarget> = BTreeMap::new();
    // Later layers win, so the walk is in precedence order and insert overwrites.
    for layer in layers {
        let dir = layer.root.join("mirror");
        for path in tree::walk(&dir) {
            let Some(live) = tree::live_path(&dir, &path) else { continue };
            shipped.insert(live.clone());
            by_live.insert(live.clone(), mirror_target(&path, live)?);
        }
    }
    for live in omit {
        // A tombstone that names nothing is a typo or a stale override. Either way the
        // operator asked for something the engine cannot do, so it is refused, not ignored.
        if !shipped.contains(live) {
            return Err(Failure::precondition(format!(
                "[omit] mirror names {live}, which no layer ships"
            )));
        }
        by_live.remove(live);
    }
    Ok(by_live.into_values().collect())
}

fn mirror_target(path: &Path, live: String) -> Result<MirrorTarget> {
    use std::os::unix::fs::PermissionsExt;
    let bytes = std::fs::read(path)
        .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
    let mode = std::fs::metadata(path)
        .map_err(|e| Failure::precondition(format!("stat {}: {e}", path.display())))?
        .permissions()
        .mode()
        & 0o777;
    Ok(MirrorTarget { live, bytes, mode, origin: path.to_path_buf() })
}

fn load_managed(layers: &[Layer], omit: &[String]) -> Result<Vec<ManagedRecord>> {
    let mut records: BTreeMap<String, ManagedRecord> = BTreeMap::new();
    for layer in layers {
        let dir = layer.root.join("managed");
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let mut record: ManagedRecord = parse(&read_text(&path)?, &path)?;
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if record.name.is_empty() {
                record.name = stem.to_string();
            }
            records.insert(record.name.clone(), record);
        }
    }
    for name in omit {
        if records.remove(name).is_none() {
            return Err(Failure::precondition(format!(
                "[omit] managed names '{name}', which no layer declares"
            )));
        }
    }
    Ok(records.into_values().collect())
}
