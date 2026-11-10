//! The ceiling rule: topology decides the class, and there is exactly one place that
//! decides it.
//!
//! This module exists because the shell engine once classified policies two different ways
//! — a topology map in the reader, `cpuinfo_max_freq` in the writer — so a low readback
//! could silently cap the fast cores at the slow ceiling while the report stayed green.
//! Reader and writer call [`want_for_policy`] here; nothing else may restate the map or the
//! kHz values.

use crate::exit::{Failure, Result};
use crate::host::Host;

/// Ceiling for the Zen 5 cores, kHz. Owner ruling: 4.7 GHz.
pub const ZEN5_KHZ: u32 = 4_700_000;
/// Ceiling for the Zen 5c cores, kHz. Owner ruling: 3.2 GHz.
pub const ZEN5C_KHZ: u32 = 3_200_000;

/// Class map: Zen 5 is CPUs 0-3 and 12-15 (the top half of each Zen 5 / Zen 5c pair).
///
/// The map is a property of this SoC, not of a policy index, which is why it is a CPU set
/// and not "policies 0 and 2". `related_cpus` is the truth; `cpuinfo_max_freq` is only a
/// fallback for when the topology file is unreadable.
pub fn is_zen5_cpu(cpu: u32) -> bool {
    (0..=3).contains(&cpu) || (12..=15).contains(&cpu)
}

/// The ceiling a policy should carry, in kHz.
pub fn want_from_cpus(cpus: &[u32]) -> u32 {
    // A policy holding any Zen 5 core takes the Zen 5 ceiling: capping it lower would bind
    // those cores below their own admitted ceiling, which is a power cap, not a tail trim.
    if cpus.iter().any(|c| is_zen5_cpu(*c)) {
        ZEN5_KHZ
    } else {
        ZEN5C_KHZ
    }
}

fn parse_cpu_list(raw: &str) -> Vec<u32> {
    // `related_cpus` is a space-separated list, but tolerate the `0-3,12-15` form too: a
    // kernel that changes the format must degrade to a parse, not to a silent wrong class.
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
pub fn want_for_policy(host: &dyn Host, policy_dir: &str) -> Result<u32> {
    if let Some(raw) = host.read_opt(&format!("{policy_dir}/related_cpus")) {
        let cpus = parse_cpu_list(&raw);
        if !cpus.is_empty() {
            return Ok(want_from_cpus(&cpus));
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
    // The fallback threshold is deliberately above the Zen 5c ceiling: a policy that admits
    // more than 4.4 GHz is a fast-core policy even when the topology file is missing.
    Ok(if imax > 4_400_000 { ZEN5_KHZ } else { ZEN5C_KHZ })
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

    #[test]
    fn class_map_is_the_top_half_of_each_pair() {
        for cpu in 0..=3 {
            assert!(is_zen5_cpu(cpu), "{cpu} should be Zen 5");
        }
        for cpu in 4..=11 {
            assert!(!is_zen5_cpu(cpu), "{cpu} should be Zen 5c");
        }
        for cpu in 12..=15 {
            assert!(is_zen5_cpu(cpu), "{cpu} should be Zen 5");
        }
        for cpu in 16..=23 {
            assert!(!is_zen5_cpu(cpu), "{cpu} should be Zen 5c");
        }
    }

    #[test]
    fn any_fast_core_decides_the_policy() {
        assert_eq!(want_from_cpus(&[0, 12]), ZEN5_KHZ);
        assert_eq!(want_from_cpus(&[4, 16]), ZEN5C_KHZ);
    }

    #[test]
    fn parses_both_list_forms() {
        assert_eq!(parse_cpu_list("0 1 2 3"), vec![0, 1, 2, 3]);
        assert_eq!(parse_cpu_list("0-3,12-15"), vec![0, 1, 2, 3, 12, 13, 14, 15]);
    }
}
