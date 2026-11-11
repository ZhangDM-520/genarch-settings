//! The ceiling rule: declared classes decide the cap, and there is exactly one place that
//! decides it.
//!
//! This module exists because the shell engine once classified policies two different ways
//! — a topology map in the reader, `cpuinfo_max_freq` in the writer — so a low readback
//! could silently cap the fast cores at the slow ceiling while the report stayed green.
//! Reader ([`want_for_policy`], used by the CEIL row) and writer (
//! [`crate::pass::apply_ceil`]) call this module; nothing else may restate the map or a kHz
//! value.
//!
//! The classes themselves are data (`ceil.toml` in a layer), not constants: a ceiling is a
//! measurement of one SoC, and a second AMD laptop must not inherit this one's numbers.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::exit::{Failure, Result};
use crate::host::Host;

/// The declared ceiling classes, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeilConfig {
    pub classes: Vec<CeilClass>,
}

/// One class: a ceiling in kHz, and the CPU set it applies to.
///
/// An empty CPU set is a **band class**: it applies by frequency, not by core identity, for a
/// machine whose CPU numbering is unknown (any machine this repository has not booted).
/// [`CeilConfig::fallback_from_cpuinfo`] is the rule it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeilClass {
    pub name: String,
    pub khz: u32,
    pub cpus: BTreeSet<u32>,
}

impl CeilClass {
    pub fn is_band(&self) -> bool {
        self.cpus.is_empty()
    }
}

/// A plausible ceiling, kHz. The floor catches the `470000` / `4700000` unit mistake (which
/// silently caps a machine at 470 MHz); the ceiling catches the same mistake upward.
const KHZ_RANGE: std::ops::RangeInclusive<u32> = 500_000..=10_000_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    class: Vec<ClassFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassFile {
    name: String,
    /// Absent or empty means a band class (see [`CeilClass`]).
    #[serde(default)]
    cpus: Vec<String>,
    khz: u32,
}

impl CeilConfig {
    /// Parse and validate `ceil.toml`.
    ///
    /// A class with no CPU or no ceiling is a config error, not an empty class: it would
    /// silently leave a policy uncapped, which is the failure this module exists to prevent.
    pub fn parse(text: &str, path: &std::path::Path) -> Result<CeilConfig> {
        let file: File = toml::from_str(text)
            .map_err(|e| Failure::precondition(format!("parse {}: {e}", path.display())))?;
        if file.class.is_empty() {
            return Err(Failure::precondition(format!(
                "{} declares no [[class]]: a ceiling map with no classes is not a policy",
                path.display()
            )));
        }
        let mut classes: Vec<CeilClass> = Vec::new();
        let mut band_seen = false;
        for class in file.class {
            let name = class.name.trim().to_string();
            if name.is_empty() {
                return Err(Failure::precondition(format!(
                    "{}: a [[class]] has an empty name",
                    path.display()
                )));
            }
            if classes.iter().any(|c| c.name == name) {
                return Err(Failure::precondition(format!(
                    "{}: duplicate ceiling class '{name}'",
                    path.display()
                )));
            }
            if !KHZ_RANGE.contains(&class.khz) {
                return Err(Failure::precondition(format!(
                    "{}: class '{name}' declares {khz} kHz, outside the plausible range \
                     {}-{} kHz (a unit mistake, not a ceiling)",
                    path.display(),
                    KHZ_RANGE.start(),
                    KHZ_RANGE.end(),
                    khz = class.khz
                )));
            }
            let mut cpus = BTreeSet::new();
            for chunk in &class.cpus {
                cpus.extend(parse_cpu_list(chunk));
            }
            if cpus.is_empty() {
                if band_seen {
                    return Err(Failure::precondition(format!(
                        "{}: class '{name}' is a second band class — frequency bands must not \
                         overlap, so one band per machine is enough",
                        path.display()
                    )));
                }
                band_seen = true;
            }
            classes.push(CeilClass { name, khz: class.khz, cpus });
        }
        Ok(CeilConfig { classes })
    }

    /// The ceiling a policy should carry when its CPU set is known.
    ///
    /// A policy holding any core of a class takes that class's ceiling; when classes overlap,
    /// the highest ceiling wins, because capping a policy below a core's own admitted ceiling
    /// is a power cap, not a tail trim.
    pub fn want_from_cpus(&self, cpus: &[u32]) -> Option<u32> {
        self.classes
            .iter()
            .filter(|class| cpus.iter().any(|cpu| class.cpus.contains(cpu)))
            .map(|class| class.khz)
            .max()
    }

    /// Whether any class applies by frequency rather than by CPU identity.
    pub fn has_band(&self) -> bool {
        self.classes.iter().any(CeilClass::is_band)
    }

    /// The ceiling a policy should carry when only `cpuinfo_max_freq` is readable.
    ///
    /// The largest declared ceiling that a policy admitting `imax` could plausibly have, and
    /// the smallest class when even that is below every ceiling. This replaces the shell
    /// engine's `> 4_400_000` threshold, which was one SoC's number compiled into the binary.
    pub fn fallback_from_cpuinfo(&self, imax: u32) -> Option<u32> {
        let below = self.classes.iter().filter(|c| c.khz <= imax).map(|c| c.khz).max();
        below.or_else(|| self.classes.iter().map(|c| c.khz).min())
    }

    /// One line naming the classes, for the CEIL verdict.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> =
            self.classes.iter().map(|c| format!("{} {}", c.name, c.khz)).collect();
        parts.sort();
        parts.join(" / ")
    }
}

/// `0-3,12-15` and `0 1 2 3` both parse: a kernel that changes the format must degrade to a
/// parse, not to a silent wrong class.
fn parse_cpu_list(raw: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for chunk in raw.split([',', ' ']).filter(|c| !c.trim().is_empty()) {
        match chunk.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    out.extend(a..=b);
                }
            }
            None => {
                if let Ok(c) = chunk.trim().parse::<u32>() {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Ceiling for one cpufreq policy directory.
///
/// `None` means no ceiling classes are declared, which is a legal host state: the caller
/// reports `SKIPPED` and the pass touches nothing.
pub fn want_for_policy(host: &dyn Host, ceil: &CeilConfig, policy_dir: &str) -> Result<Option<u32>> {
    if let Some(raw) = host.read_opt(&format!("{policy_dir}/related_cpus")) {
        let cpus = parse_cpu_list(&raw);
        if !cpus.is_empty() {
            if let Some(khz) = ceil.want_from_cpus(&cpus) {
                return Ok(Some(khz));
            }
            // A band class does not name CPUs, so an unknown numbering is expected rather than
            // a config error; the frequency band below answers instead. Without a band, a CPU
            // set that matches nothing is a real mistake in a measured profile.
            if !ceil.has_band() {
                return Err(Failure::partial(format!(
                    "{policy_dir}: CPUs {cpus:?} are not in any declared ceiling class"
                )));
            }
        }
    }
    let imax = host
        .read_opt(&format!("{policy_dir}/cpuinfo_max_freq"))
        .and_then(|s| s.trim().parse::<u32>().ok())
        .ok_or_else(|| {
            Failure::partial(format!(
                "{policy_dir}: neither related_cpus nor cpuinfo_max_freq is readable"
            ))
        })?;
    Ok(ceil.fallback_from_cpuinfo(imax))
}

/// Every cpufreq policy directory, sorted.
pub fn policies(host: &dyn Host) -> Vec<String> {
    const BASE: &str = "/sys/devices/system/cpu/cpufreq";
    let mut out: Vec<String> = host
        .walk(BASE)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.is_dir)
        .filter_map(|e| {
            let name = e.path.file_name()?.to_str()?.to_string();
            name.starts_with("policy").then_some(name)
        })
        .map(|name| format!("{BASE}/{name}"))
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_classes() -> CeilConfig {
        CeilConfig::parse(
            "[[class]]\nname = \"zen5\"\ncpus = [\"0-3\", \"12-15\"]\nkhz = 4700000\n\
             [[class]]\nname = \"zen5c\"\ncpus = [\"4-11\", \"16-23\"]\nkhz = 3200000\n",
            std::path::Path::new("ceil.toml"),
        )
        .unwrap()
    }

    #[test]
    fn class_map_is_the_top_half_of_each_pair() {
        let ceil = two_classes();
        assert_eq!(ceil.want_from_cpus(&[0, 12]), Some(4_700_000));
        assert_eq!(ceil.want_from_cpus(&[4, 16]), Some(3_200_000));
        assert_eq!(ceil.want_from_cpus(&[]), None);
        assert_eq!(ceil.want_from_cpus(&[99]), None);
    }

    #[test]
    fn parses_both_list_forms() {
        assert_eq!(parse_cpu_list("0 1 2 3"), vec![0, 1, 2, 3]);
        assert_eq!(parse_cpu_list("0-3,12-15"), vec![0, 1, 2, 3, 12, 13, 14, 15]);
    }

    #[test]
    fn a_band_class_applies_by_frequency_not_by_cpu_identity() {
        // A machine we have not booted has no known CPU numbering, so a documented profile
        // declares the clock band instead. It must never match by CPU set, only by frequency.
        let path = std::path::Path::new("ceil.toml");
        let ceil = CeilConfig::parse("[[class]]\nname = \"max\"\nkhz = 5100000\n", path).unwrap();
        assert!(ceil.has_band());
        assert_eq!(ceil.want_from_cpus(&[0, 8]), None);
        assert_eq!(ceil.fallback_from_cpuinfo(5_100_000), Some(5_100_000));

        // A second band would overlap by construction, so it is refused rather than ordered.
        assert!(
            CeilConfig::parse(
                "[[class]]\nname = \"a\"\nkhz = 5100000\n[[class]]\nname = \"b\"\nkhz = 4000000\n",
                path
            )
            .is_err()
        );
    }

    #[test]
    fn a_band_profile_reads_a_policy_with_known_cpus() {
        // related_cpus is always readable on a real machine, so the band must still answer when
        // the CPU set matches no class.
        use crate::host::FixtureHost;
        let dir = std::env::temp_dir().join(format!("genarch-ceil-band-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let policy = "/sys/devices/system/cpu/cpufreq/policy0";
        std::fs::create_dir_all(dir.join(policy.trim_start_matches('/'))).unwrap();
        std::fs::write(dir.join(policy.trim_start_matches('/')).join("related_cpus"), "0-15\n").unwrap();
        std::fs::write(
            dir.join(policy.trim_start_matches('/')).join("cpuinfo_max_freq"),
            "5100000\n",
        )
        .unwrap();
        let host = FixtureHost::new(&dir);

        let ceil = CeilConfig::parse("[[class]]\nname = \"max\"\nkhz = 5100000\n", std::path::Path::new("c"))
            .unwrap();
        assert_eq!(want_for_policy(&host, &ceil, policy).unwrap(), Some(5_100_000));

        // The same readback against explicit classes that cover none of these CPUs is a real
        // error: a measured profile that lost a core is not something to smooth over.
        let explicit = CeilConfig::parse(
            "[[class]]\nname = \"a\"\ncpus = [\"20-23\"]\nkhz = 5100000\n",
            std::path::Path::new("c"),
        )
        .unwrap();
        assert!(want_for_policy(&host, &explicit, policy).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_implausible_ceiling_is_a_unit_mistake() {
        let path = std::path::Path::new("ceil.toml");
        // 470000 is 4700000 with a digit missing.
        assert!(CeilConfig::parse("[[class]]\nname = \"a\"\nkhz = 470000\n", path).is_err());
        assert!(CeilConfig::parse("[[class]]\nname = \"a\"\nkhz = 47000000\n", path).is_err());
        assert!(CeilConfig::parse("[[class]]\nname = \"a\"\nkhz = 4700000\n", path).is_ok());
    }

    #[test]
    fn cpuinfo_fallback_never_caps_below_an_admitted_ceiling() {
        let ceil = two_classes();
        assert_eq!(ceil.fallback_from_cpuinfo(4_700_000), Some(4_700_000));
        assert_eq!(ceil.fallback_from_cpuinfo(3_200_000), Some(3_200_000));
        // Below every declared ceiling: the smallest class, never zero and never a guess.
        assert_eq!(ceil.fallback_from_cpuinfo(1_000), Some(3_200_000));
    }

    #[test]
    fn an_empty_or_duplicate_class_map_is_refused() {
        let path = std::path::Path::new("ceil.toml");
        assert!(CeilConfig::parse("", path).is_err(), "no classes must be refused");
        for text in [
            "[[class]]\nname = \"a\"\ncpus = [\"0\"]\nkhz = 0\n",
            "[[class]]\nname = \"a\"\ncpus = []\nkhz = 1\n",
            "[[class]]\nname = \"\"\ncpus = [\"0\"]\nkhz = 1\n",
            "[[class]]\nname = \"a\"\ncpus = [\"0\"]\nkhz = 1\n[[class]]\nname = \"a\"\ncpus = [\"1\"]\nkhz = 2\n",
        ] {
            assert!(CeilConfig::parse(text, path).is_err(), "{text:?} must be refused");
        }
    }
}
