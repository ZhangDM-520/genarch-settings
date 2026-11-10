//! Intent: what the host should look like.
//!
//! Hybrid by design. Anything the engine *computes* is typed TOML, so a value has one
//! home and can be validated; anything it must not *interpret* is a byte-exact mirror
//! file, so the engine cannot silently rewrite a vendor file's meaning.
//!
//! Two roots, one intent: the package ships defaults under `/usr/lib/genarch`, and the
//! machine overrides individual files from `/etc/genarch`. An override replaces a whole
//! file (never a merge), because a merge would make "what is the value" a question with
//! two answers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::exit::{Failure, Result};
use crate::host::Host;

/// Schema version. Bumped when a field's meaning changes; the loader refuses an intent
/// it does not understand rather than guessing.
pub const SCHEMA_VERSION: u32 = 2;

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RyzenadjConfig {
    #[serde(default)]
    pub co: CoConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub raw: Vec<RawArgs>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CoConfig {
    /// Global curve-optimizer offset, the `--set-coall` value.
    #[serde(default)]
    pub all: i32,
    /// Per-core offsets, index = core number.
    #[serde(default)]
    pub per_core: Vec<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct LimitsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stapm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stapm_time: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow_time: Option<u32>,
    #[serde(default)]
    pub power_saving: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RawArgs {
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// What the scheduled passes do, declared once instead of hardcoded in a script.
///
/// Every list here used to be a literal inside `eco-power.sh`; keeping them in the intent
/// means the boot pass can be reviewed (and a rehearsal can run against a variant) without
/// editing code.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PassesConfig {
    /// ACPI wake sources to disarm at boot. The lid and the internal keyboard are not
    /// listed on purpose: the lid is EC-handled and i8042 is absent from the ACPI table.
    #[serde(default = "default_wake_disarm")]
    pub wake_disarm: Vec<String>,
    /// USB devices to allow autosuspending, as `vid:pid`.
    #[serde(default = "default_usb_autosuspend")]
    pub usb_autosuspend: Vec<String>,
    /// cpuidle governor; `teo` predicts idle states better and sleeps deeper, more often.
    #[serde(default = "default_idle_governor")]
    pub idle_governor: String,
    /// Wireless interface for the power-save re-assert.
    #[serde(default = "default_wifi_interface")]
    pub wifi_interface: String,
    /// Seconds between the two SMU passes of the boot pass, so the second lands after the
    /// event handlers that reset firmware limits. Never used on resume: a sleep hook that
    /// sleeps stalls the user.slice thaw.
    #[serde(default = "default_settle_secs")]
    pub settle_secs: u64,
}

fn default_wake_disarm() -> Vec<String> {
    ["GPP0", "GPP1", "GPP3", "XHC0", "XHC1", "XHC3", "XHC4", "NHI0", "NHI1"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_usb_autosuspend() -> Vec<String> {
    vec!["13d3:3608".to_string()]
}

fn default_idle_governor() -> String {
    "teo".to_string()
}

fn default_wifi_interface() -> String {
    "wlan0".to_string()
}

fn default_settle_secs() -> u64 {
    5
}

impl Default for PassesConfig {
    fn default() -> Self {
        PassesConfig {
            wake_disarm: default_wake_disarm(),
            usb_autosuspend: default_usb_autosuspend(),
            idle_governor: default_idle_governor(),
            wifi_interface: default_wifi_interface(),
            settle_secs: default_settle_secs(),
        }
    }
}

/// The loaded intent.
#[derive(Debug, Clone, Default)]
pub struct Intent {
    pub schema: u32,
    /// Base root the intent was loaded from (`/usr/lib/genarch`).
    pub base: PathBuf,
    /// Override root (`/etc/genarch`), whether or not it exists.
    pub overlay: PathBuf,
    /// Runtime sysctl values, key → value. Multi-word values keep their spaces.
    pub sysctl: BTreeMap<String, String>,
    pub ryzenadj: RyzenadjConfig,
    pub passes: PassesConfig,
    pub mirror: Vec<MirrorTarget>,
    pub managed: Vec<ManagedRecord>,
    /// Live paths the engine must remove when present.
    pub prune: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Meta {
    schema: u32,
    /// Ppaths the engine must remove if present. Declared, not inferred: at cutover the
    /// manifest is empty (this engine has never applied), so "what should disappear" has to
    /// be written down somewhere reviewable — here, not in a runbook step somebody forgets.
    #[serde(default)]
    prune: Vec<String>,
}

/// Every regular file under `dir`, recursively, sorted. The intent tree is package data
/// on real paths, so this reads directly instead of going through the [`Host`] port —
/// the port exists for the *live* machine, not for the package it shipped from.
fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() => out.push(path),
                _ => {}
            }
        }
    }
    out.sort();
    out
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
        let meta_path = base.join("intent.toml");
        let meta_text = std::fs::read_to_string(&meta_path).map_err(|e| {
            Failure::precondition(format!(
                "no intent at {} ({e}) — is the package installed?",
                meta_path.display()
            ))
        })?;
        let meta: Meta = toml::from_str(&meta_text)
            .map_err(|e| Failure::precondition(format!("parse {}: {e}", meta_path.display())))?;
        if meta.schema != SCHEMA_VERSION {
            return Err(Failure::precondition(format!(
                "intent schema {} at {} but this engine speaks {SCHEMA_VERSION}",
                meta.schema,
                meta_path.display()
            )));
        }

        let mut intent = Intent {
            schema: meta.schema,
            base: base.to_path_buf(),
            overlay: overlay.to_path_buf(),
            prune: meta.prune,
            ..Intent::default()
        };

        intent.sysctl = intent.load_sysctl()?;
        intent.ryzenadj = intent.load_ryzenadj()?;
        intent.passes = intent.load_passes()?;
        intent.mirror = intent.load_mirror()?;
        intent.managed = intent.load_managed()?;
        intent.validate()?;
        Ok(intent)
    }

    /// Overlay precedence: a whole-file override replaces the base file.
    fn pick(&self, rel: &str) -> Option<PathBuf> {
        let over = self.overlay.join(rel);
        if over.is_file() {
            return Some(over);
        }
        let base = self.base.join(rel);
        base.is_file().then_some(base)
    }

    fn load_sysctl(&self) -> Result<BTreeMap<String, String>> {
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            values: BTreeMap<String, toml::Value>,
        }
        let Some(path) = self.pick("sysctl.toml") else {
            return Ok(BTreeMap::new());
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
        let file: File = toml::from_str(&text)
            .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))?;
        let mut out = BTreeMap::new();
        for (key, value) in file.values {
            // A sysctl value may be a word or a list of words; both render to one line.
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
            out.insert(key, rendered.trim().to_string());
        }
        Ok(out)
    }

    fn load_ryzenadj(&self) -> Result<RyzenadjConfig> {
        let Some(path) = self.pick("ryzenadj.toml") else {
            return Ok(RyzenadjConfig::default());
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
        toml::from_str(&text)
            .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))
    }

    fn load_passes(&self) -> Result<PassesConfig> {
        let Some(path) = self.pick("passes.toml") else {
            return Ok(PassesConfig::default());
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
        toml::from_str(&text)
            .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))
    }

    fn load_mirror(&self) -> Result<Vec<MirrorTarget>> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        // Overlay first so a base entry cannot shadow it.
        for root in [&self.overlay, &self.base] {
            let mirror_dir = root.join("mirror");
            for path in walk_files(&mirror_dir) {
                let rel = match path.strip_prefix(&mirror_dir) {
                    Ok(rel) => rel,
                    Err(_) => continue,
                };
                let live = format!("/{}", rel.display());
                if !seen.insert(live.clone()) {
                    continue;
                }
                out.push(self.mirror_target(path, live)?);
            }
        }
        out.sort_by(|a, b| a.live.cmp(&b.live));
        Ok(out)
    }

    fn mirror_target(&self, path: PathBuf, live: String) -> Result<MirrorTarget> {
        use std::os::unix::fs::PermissionsExt;
        let bytes = std::fs::read(&path)
            .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
        let mode = std::fs::metadata(&path)
            .map_err(|e| Failure::precondition(format!("stat {}: {e}", path.display())))?
            .permissions()
            .mode()
            & 0o777;
        Ok(MirrorTarget { live, bytes, mode, origin: path })
    }

    fn load_managed(&self) -> Result<Vec<ManagedRecord>> {
        let mut records: BTreeMap<String, ManagedRecord> = BTreeMap::new();
        // Base first, overlay second so an override of the same name wins.
        for root in [&self.base, &self.overlay] {
            let dir = root.join("managed");
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).map_err(|e| {
                    Failure::precondition(format!("read {}: {e}", path.display()))
                })?;
                let mut record: ManagedRecord = toml::from_str(&text).map_err(|e| {
                    Failure::precondition(format!("parse {}: {e}", path.display()))
                })?;
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                if record.name.is_empty() {
                    record.name = stem.to_string();
                }
                records.insert(record.name.clone(), record);
            }
        }
        Ok(records.into_values().collect())
    }

    fn validate(&self) -> Result<()> {
        // A path may have one *kind* of owner. Mirror + prune on the same path made `apply`
        // write the file and then delete it, leaving drift that never converges; mirror +
        // managed meant a whole-file write and a line patch fighting over the same bytes.
        // Several managed records on one target stay legal — that is how `asusd.ron` is
        // patched at five anchors.
        let mirror: std::collections::BTreeSet<&str> =
            self.mirror.iter().map(|m| m.live.as_str()).collect();
        let prune: std::collections::BTreeSet<&str> =
            self.prune.iter().map(|p| p.as_str()).collect();
        let managed: std::collections::BTreeSet<&str> =
            self.managed.iter().map(|r| r.target.as_str()).collect();
        for (a, b, a_kind, b_kind) in [
            (&mirror, &prune, "mirror", "prune"),
            (&mirror, &managed, "mirror", "managed"),
            (&prune, &managed, "prune", "managed"),
        ] {
            if let Some(path) = a.intersection(b).next() {
                return Err(Failure::precondition(format!(
                    "{path} is declared as both {a_kind} and {b_kind}: one path, one owner"
                )));
            }
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
            if !crate::converge::is_deployable_name(base) {
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
        let names: std::collections::BTreeSet<&str> =
            self.managed.iter().map(|r| r.name.as_str()).collect();
        if names.len() != self.managed.len() {
            return Err(Failure::precondition(
                "duplicate managed record name — one record would silently replace another",
            ));
        }
        Ok(())
    }

    /// Every managed record whose target is this live path.
    pub fn managed_for(&self, live: &str) -> Vec<&ManagedRecord> {
        self.managed.iter().filter(|r| r.target == live).collect()
    }
}
