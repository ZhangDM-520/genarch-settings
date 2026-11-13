//! One-time migration: derive the initial intent from the shell engine's `state/` tree.
//!
//! Hand-writing the first intent would be a silent translation of ~60 files, and a silent
//! translation is exactly how a rewrite loses a knob. So the intent is *derived* from the
//! tree that the live host is currently converged to, and the acceptance test is a
//! round-trip property: `import → load → render` must reproduce the source bytes.
//!
//! The output is a **profile**, never a base intent: every file here is a fact about one
//! machine, and a base that shipped them would rewrite the next machine's `/etc/fstab`.
//!
//! Two translations are not byte copies and are therefore spelled out here, once:
//!
//! - `etc/sysctl.d/*.conf` collapse into `sysctl.toml` (values), because the engine applies
//!   sysctls at runtime instead of shipping five ordering-sensitive files;
//! - `hw-tune.fish` becomes `ryzenadj.toml` (knobs), because the imperative script is what
//!   the rewrite exists to replace.
//!
//! What the shell kept *in code* (`eco-power.sh`'s wake list and settle delay,
//! `lib/ceiling.sh`'s class map) is not in `state/`, so the importer reports it as a note
//! instead of inventing a value: a guessed wake list is the silent Zenbook default this
//! engine exists to delete.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::exit::{Failure, Result};
use crate::intent::RyzenadjConfig;
use crate::tree;

#[derive(Debug, Default)]
pub struct Summary {
    pub mirror: usize,
    pub managed: usize,
    pub sysctl: usize,
    pub skipped: Vec<String>,
    /// What the importer could not derive and the operator must fill in.
    pub notes: Vec<String>,
}

/// Read `state/` and write an intent tree to `out`.
///
/// Nothing is written until the whole profile has been derived in memory: a tree that is not a
/// shell state tree, a subtree this process cannot read, or an ambiguity (two `hw-tune.fish`, a
/// record that will not parse) must fail with exit 2 and leave `out` untouched. A half-derived
/// profile on disk is worse than no profile — `load` would accept it and the operator would ship
/// a tune that silently omits whatever the failure was about.
pub fn run(state: &Path, out: &Path) -> Result<Summary> {
    let owned = state.join("owned");
    if !owned.is_dir() {
        return Err(Failure::precondition(format!(
            "{} is not a directory: not a shell state tree",
            owned.display()
        )));
    }
    let mut summary = Summary::default();
    notes_for_profile(&mut summary);
    let mut profile = Profile::default();
    profile.mirror_and_notes(&owned, &mut summary)?;
    profile.managed(state)?;
    profile.sysctl(state)?;
    profile.ryzenadj(&owned, &mut summary)?;
    summary.managed = profile.managed.len();
    summary.sysctl = profile.sysctl.len();

    std::fs::create_dir_all(out)
        .map_err(|e| Failure::precondition(format!("mkdir {}: {e}", out.display())))?;
    profile.write(out)?;
    Ok(summary)
}

/// An intent tree derived from `state/`, held in memory until every part of it has been derived.
#[derive(Default)]
struct Profile {
    /// `(path relative to the mirror root, source bytes, source mode)`. The path stays a
    /// `PathBuf`: a lossy conversion would rewrite a non-UTF-8 file name, which makes two
    /// distinct source files collide on one destination.
    mirror: Vec<(PathBuf, Vec<u8>, u32)>,
    /// `(file name, rendered record)`.
    managed: Vec<(std::ffi::OsString, String)>,
    sysctl: BTreeMap<String, String>,
    ryzenadj: Option<String>,
}

impl Profile {
    fn mirror_and_notes(&mut self, owned: &Path, summary: &mut Summary) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        for path in tree::walk(owned)? {
            let Ok(rel) = path.strip_prefix(owned) else { continue };
            // A live path is a `String` end to end, so a non-UTF-8 name cannot be represented.
            // Refuse it here: `to_string_lossy` would rewrite it, and two distinct source files
            // would silently land on one destination.
            let Some(rel_text) = rel.to_str() else {
                return Err(Failure::precondition(format!(
                    "{}: file name is not valid UTF-8, so it has no live path",
                    path.display()
                )));
            };
            let base = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if !tree::is_deployable_name(base) {
                summary.skipped.push(format!("/{rel_text}"));
                continue;
            }
            let rel = rel.to_path_buf();
            let bytes = std::fs::read(&path)
                .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
            // Preserve the mode: an exec bit is load-bearing for deploy (service 203/EXEC).
            let mode = std::fs::metadata(&path)
                .map_err(|e| Failure::precondition(format!("stat {}: {e}", path.display())))?
                .permissions()
                .mode();
            self.mirror.push((rel, bytes, mode));
        }
        summary.mirror = self.mirror.len();
        Ok(())
    }

    fn managed(&mut self, state: &Path) -> Result<()> {
        let src = state.join("managed");
        // A missing `state/managed` is a layer that ships nothing; one that exists but cannot be
        // read is a failure, not an empty layer.
        let entries = match std::fs::read_dir(&src) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(Failure::precondition(format!("read {}: {e}", src.display())));
            }
        };
        for entry in entries {
            let entry = entry
                .map_err(|e| Failure::precondition(format!("read {}: {e}", src.display())))?;
            let path = entry.path();
            let Some(name) = path.file_name().map(|n| n.to_os_string()) else { continue };
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
            let mut record: crate::intent::ManagedRecord = toml::from_str(&text)
                .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))?;
            if record.name.is_empty() {
                record.name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string();
            }
            let rendered = toml::to_string(&record)
                .map_err(|e| Failure::precondition(format!("encode {}: {e}", path.display())))?;
            self.managed.push((name, rendered));
        }
        Ok(())
    }

    fn sysctl(&mut self, state: &Path) -> Result<()> {
        self.sysctl = collect_sysctl(state)?;
        Ok(())
    }

    fn ryzenadj(&mut self, owned: &Path, summary: &mut Summary) -> Result<()> {
        let Some(path) = discover_hw_tune(owned)? else {
            summary.notes.push(
                "no hw-tune.fish under state/owned: ryzenadj.toml was not derived (curve offsets \
                 and limits must be copied from the live intent)"
                    .into(),
            );
            return Ok(());
        };
        self.ryzenadj = derive_ryzenadj(&path)?;
        Ok(())
    }

    /// Write the already-validated tree. A write error here is an I/O failure, never a
    /// derivation failure: everything that can be refused has been refused before this point.
    fn write(self, out: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        // A profile may omit schema; what it should not omit is [requires], because that is what
        // turns "this profile is on the wrong laptop" into a reportable drift. The importer has no
        // machine to ask (it reads an offline tree), so it leaves the table to the operator.
        let meta = "# Generated by `genarch-settings import` from the shell engine's state/ tree.\n\
                    # This directory is a machine profile. Declare the machine it is for, so a\n\
                    # wrong selection is DRIFT rather than a silent wrong tune:\n\
                    #\n\
                    #   [requires]\n\
                    #   product = \"<DMI product_name>\"\n\
                    #   sys_vendor = \"<DMI sys_vendor>\"\n";
        write_file(&out.join("intent.toml"), meta.as_bytes())?;
        for (rel, bytes, mode) in &self.mirror {
            let dest = out.join("mirror").join(rel);
            write_file(&dest, bytes)?;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(*mode))
                .map_err(|e| Failure::precondition(format!("chmod {}: {e}", dest.display())))?;
        }
        if !self.managed.is_empty() {
            let dir = out.join("managed");
            std::fs::create_dir_all(&dir)
                .map_err(|e| Failure::precondition(format!("mkdir {}: {e}", dir.display())))?;
            for (name, rendered) in &self.managed {
                let dest = dir.join(name);
                write_file(&dest, rendered.as_bytes())?;
            }
        }
        if !self.sysctl.is_empty() {
            let mut text = String::from(
                "# Generated by `genarch-settings import`. One home for the value: the engine renders nothing\n\
                 # and applies these directly to /proc/sys, so ordering cannot hide a value.\n\
                 [values]\n",
            );
            for (key, value) in &self.sysctl {
                text.push_str(&format!("{} = {}\n", toml_key(key), toml_string(value)));
            }
            write_file(&out.join("sysctl.toml"), text.as_bytes())?;
        }
        if let Some(rendered) = &self.ryzenadj {
            write_file(&out.join("ryzenadj.toml"), rendered.as_bytes())?;
        }
        Ok(())
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Failure::precondition(format!("mkdir {}: {e}", parent.display())))?;
    }
    std::fs::write(path, bytes).map_err(|e| Failure::precondition(format!("write {}: {e}", path.display())))
}

fn notes_for_profile(summary: &mut Summary) {
    summary.notes.push(
        "profile intent.toml: fill in [requires] (product, sys_vendor) so a wrong machine is DRIFT"
            .into(),
    );
    summary.notes.push(
        "passes.toml and ceil.toml are not derivable from state/ (they were code in the shell \
         engine): copy them from the live intent"
            .into(),
    );
}

/// Collapse the shell's `sysctl.d` files into one value table.
///
/// Ordering is irrelevant now: the engine applies values directly, so a later file cannot
/// silently win by filename. Values from a later-sorting file still override, so the
/// derivation matches what the live kernel ended up with.
fn collect_sysctl(state: &Path) -> Result<BTreeMap<String, String>> {
    let dir = state.join("owned/etc/sysctl.d");
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut files: Vec<PathBuf> = tree::walk(&dir)?
        .into_iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("conf"))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let key = key.trim();
            let value = value.split('#').next().unwrap_or("").trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            values.insert(key.to_string(), value.to_string());
        }
    }
    Ok(values)
}

/// The shell engine's `hw-tune.fish`, wherever the owning user's home is.
///
/// The path is `owned/home/<user>/.reserve/hw-tune.fish` in the source tree, and `<user>` is a
/// host fact: naming it here made `import` work for exactly one operator. The file is found by
/// name instead. Several matches are a hard error, as with anchor matching: choosing one would
/// derive this machine's knobs from an arbitrary user's script.
fn discover_hw_tune(owned: &Path) -> Result<Option<PathBuf>> {
    let mut found: Vec<PathBuf> = tree::walk(owned)?
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("hw-tune.fish"))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        _ => Err(Failure::precondition(format!(
            "several hw-tune.fish under {}: {} — keep the one this machine converged to",
            owned.display(),
            found.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// Translate `hw-tune.fish` into declarative knobs.
fn derive_ryzenadj(path: &Path) -> Result<Option<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Failure::precondition(format!("read {}: {e}", path.display())))?;
    let mut config = RyzenadjConfig::default();

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("set -l coall ") {
            config.co.all = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("set -l per_core_offsets ") {
            config.co.per_core = Some(
                rest.split_whitespace()
                    .filter_map(|v| v.parse::<i32>().ok())
                    .collect(),
            );
        } else if line.starts_with("ryzenadj ") {
            apply_ryzenadj_line(&mut config, line);
        }
    }
    if config == RyzenadjConfig::default() {
        return Ok(None);
    }
    let mut rendered = String::from(
        "# Generated by `genarch-settings import` from hw-tune.fish. Knobs, not argv: the packing\n\
         # arithmetic lives in the engine so a config edit cannot corrupt it.\n",
    );
    rendered.push_str(
        &toml::to_string(&config)
            .map_err(|e| Failure::precondition(format!("encode ryzenadj.toml: {e}")))?,
    );
    Ok(Some(rendered))
}

fn apply_ryzenadj_line(config: &mut RyzenadjConfig, line: &str) {
    // The CO writes are derived from the `set -l` knobs above, and their fish syntax embeds
    // `(printf …)` command substitution. Carrying those tokens as raw args would replay
    // garbage argv; the knobs are the whole truth for them.
    if line.contains("--set-coall") || line.contains("--set-coper") {
        return;
    }
    for arg in line.split_whitespace().skip(1) {
        // A token with a shell construct is not a literal argument.
        if arg.contains('(') || arg.contains('$') || arg.contains('"') || arg.contains("'") {
            continue;
        }
        let (flag, value) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v)),
            None => (arg, None),
        };
        match (flag, value) {
            ("--stapm-limit", Some(v)) => config.limits.stapm = v.parse().ok(),
            ("--fast-limit", Some(v)) => config.limits.fast = v.parse().ok(),
            ("--slow-limit", Some(v)) => config.limits.slow = v.parse().ok(),
            ("--stapm-time", Some(v)) => config.limits.stapm_time = v.parse().ok(),
            ("--slow-time", Some(v)) => config.limits.slow_time = v.parse().ok(),
            ("--power-saving", _) => config.limits.power_saving = Some(true),
            // Anything the engine has no field for is carried as a raw argument rather than
            // dropped: a silent loss here is a tuning that disappears at cutover.
            (flag, Some(v)) => config.raw.push(crate::intent::RawArgs {
                args: vec![format!("{flag}={v}")],
                why: Some("carried verbatim from hw-tune.fish".into()),
            }),
            (flag, None) => config.raw.push(crate::intent::RawArgs {
                args: vec![flag.to_string()],
                why: Some("carried verbatim from hw-tune.fish".into()),
            }),
        }
    }
}

fn toml_key(key: &str) -> String {
    // A bare dotted key would be parsed as a table path (`kernel = { printk = … }`), which
    // silently changes the meaning of every sysctl name. Quote it.
    if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        key.to_string()
    } else {
        format!("{key:?}")
    }
}

fn toml_string(value: &str) -> String {
    format!("{value:?}")
}
