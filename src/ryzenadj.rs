//! ryzenadj: the SMU writer.
//!
//! ryzenadj is an external binary and stays one — the engine builds an argv and runs it.
//! That makes the whole module's behaviour an argv property, which is why it can be tested
//! exhaustively without touching an SMU: a fixture host records argv, and a test asserts it
//! byte for byte.
//!
//! Two packing rules are load-bearing and must not be re-derived at a call site:
//!
//! - every CO offset is sent as 16-bit two's complement (`-7` → `0xFFF9`);
//! - a per-core offset is `core_index * 2^20 + two's-complement offset`, so one word
//!   carries both the core and its value.
//!
//! Ordering matters too: the global offset is applied before the per-core ones, limits after
//! both, and the raw escape hatch last so a hand-written flag always wins.

use crate::exit::Result;
use crate::host::Host;
use crate::intent::RyzenadjConfig;

/// 16-bit two's complement of a CO offset.
pub fn co_word(offset: i32) -> u32 {
    (offset.rem_euclid(65536)) as u32
}

/// The packed value for one core's offset.
pub fn per_core_word(core: u32, offset: i32) -> u32 {
    core * (1 << 20) + co_word(offset)
}

fn hex32(value: u32) -> String {
    format!("0x{value:08X}")
}

/// The full argv plan for a config, in application order.
pub fn argv_plan(config: &RyzenadjConfig) -> Vec<Vec<String>> {
    let mut calls: Vec<Vec<String>> = Vec::new();
    let prog = |args: &[String]| {
        let mut v = vec!["ryzenadj".to_string()];
        v.extend(args.iter().cloned());
        v
    };

    // `Some(0)` is a declaration, not an absence: writing 0 clears whatever the firmware
    // left, which is a different machine state from "leave it alone" (`None`).
    if let Some(all) = config.co.all {
        calls.push(prog(&[format!("--set-coall={}", hex32(co_word(all)))]));
    }
    for (core, offset) in config.co.per_core.iter().flatten().enumerate() {
        calls.push(prog(&[format!(
            "--set-coper={}",
            hex32(per_core_word(core as u32, *offset))
        )]));
    }

    let mut limits: Vec<String> = Vec::new();
    if let Some(v) = config.limits.stapm {
        limits.push(format!("--stapm-limit={v}"));
    }
    if let Some(v) = config.limits.fast {
        limits.push(format!("--fast-limit={v}"));
    }
    if let Some(v) = config.limits.slow {
        limits.push(format!("--slow-limit={v}"));
    }
    if !limits.is_empty() {
        calls.push(prog(&limits));
    }

    let mut times: Vec<String> = Vec::new();
    if let Some(v) = config.limits.stapm_time {
        times.push(format!("--stapm-time={v}"));
    }
    if let Some(v) = config.limits.slow_time {
        times.push(format!("--slow-time={v}"));
    }
    if !times.is_empty() {
        calls.push(prog(&times));
    }

    if config.limits.power_saving == Some(true) {
        calls.push(prog(&["--power-saving".to_string()]));
    }

    for raw in &config.raw {
        calls.push(prog(&raw.args));
    }
    calls
}

/// What one ryzenadj call did, for the journal.
#[derive(Debug, Clone)]
pub struct CallResult {
    pub argv: Vec<String>,
    pub ok: bool,
}

/// Apply the settings. Every call is logged; a failure is reported but does not abort the
/// pass, because a single unsupported flag must not leave the remaining knobs unapplied.
pub fn apply(host: &dyn Host, config: &RyzenadjConfig) -> Result<Vec<CallResult>> {
    let mut out = Vec::new();
    for argv in argv_plan(config) {
        let result = host.run(&argv);
        let ok = matches!(result, Ok(ref o) if o.success);
        if let Ok(o) = &result
            && !o.success {
                let stderr = &o.stderr;
                eprintln!(
                    "genarch: ryzenadj failed: {} ({}): {}",
                    argv.join(" "),
                    o.status,
                    stderr.trim()
                );
            }
        out.push(CallResult { argv, ok });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{CoConfig, LimitsConfig, RawArgs};

    #[test]
    fn negative_offsets_are_16_bit_twos_complement() {
        assert_eq!(co_word(-7), 0xFFF9);
        assert_eq!(co_word(-24), 0xFFE8);
        assert_eq!(co_word(0), 0);
        // A positive offset stays plain: no wrap-around for the documented range.
        assert_eq!(co_word(5), 5);
    }

    #[test]
    fn per_core_packs_index_above_offset() {
        assert_eq!(per_core_word(0, -24), 0x0000_FFE8);
        assert_eq!(per_core_word(1, -24), 0x0010_FFE8);
        assert_eq!(per_core_word(3, -21), 0x0030_FFEB);
        assert_eq!(per_core_word(7, -6), 0x0070_FFFA);
    }

    #[test]
    fn an_undeclared_offset_sends_nothing_but_a_declared_zero_does() {
        // Absence and zero are different declarations: `None` leaves the firmware value,
        // `Some(0)` clears it. The row's sentinel used to be `all == 0`, which could not tell
        // the two apart.
        let config = RyzenadjConfig::default();
        assert!(argv_plan(&config).is_empty());

        let config = RyzenadjConfig {
            co: CoConfig { all: Some(0), per_core: Some(vec![0]) },
            ..Default::default()
        };
        assert_eq!(
            argv_plan(&config),
            vec![
                vec!["ryzenadj", "--set-coall=0x00000000"],
                vec!["ryzenadj", "--set-coper=0x00000000"]
            ]
        );
    }

    #[test]
    fn ordering_is_global_then_cores_then_limits_then_times_then_flags_then_raw() {
        let config = RyzenadjConfig {
            co: CoConfig { all: Some(-7), per_core: Some(vec![-24]) },
            limits: LimitsConfig {
                stapm: Some(31000),
                fast: Some(31000),
                slow: Some(31000),
                stapm_time: Some(128),
                slow_time: Some(1),
                power_saving: Some(true),
            },
            raw: vec![RawArgs { args: vec!["--future=1".into()], why: None }],
        };
        let plan = argv_plan(&config);
        assert_eq!(plan[0], vec!["ryzenadj", "--set-coall=0x0000FFF9"]);
        assert_eq!(plan[1], vec!["ryzenadj", "--set-coper=0x0000FFE8"]);
        assert_eq!(
            plan[2],
            vec![
                "ryzenadj",
                "--stapm-limit=31000",
                "--fast-limit=31000",
                "--slow-limit=31000"
            ]
        );
        assert_eq!(plan[3], vec!["ryzenadj", "--stapm-time=128", "--slow-time=1"]);
        assert_eq!(plan[4], vec!["ryzenadj", "--power-saving"]);
        assert_eq!(plan[5], vec!["ryzenadj", "--future=1"]);
    }
}
