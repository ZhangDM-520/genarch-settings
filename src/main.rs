//! Entry point: parse a verb, run it, translate the outcome into an exit code.
//!
//! Nothing here makes a decision — that is deliberate, so the whole engine stays callable
//! from tests without a process. The one thing `main` owns is the exit code, because that
//! is the contract with systemd and the operator's scripts.

use std::process::ExitCode;

use genarch::check::{self, Ctx};
use genarch::cli::{Cli, Verb};
use genarch::converge;
use genarch::exit::{Exit, Failure};
use genarch::host::RealHost;
use genarch::intent::Intent;
use genarch::{import, pass, report};

use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(exit) => ExitCode::from(exit.code() as u8),
        Err(failure) => {
            eprintln!("genarch: {}", failure.message);
            ExitCode::from(failure.kind.code() as u8)
        }
    }
}

fn run(cli: Cli) -> genarch::Result<Exit> {
    let host = RealHost::from_env();
    match cli.verb {
        Verb::Report { json } => {
            let intent = Intent::load_default(&host)?;
            let ctx = Ctx { host: &host, intent: &intent };
            let verdicts = check::evaluate(&ctx);
            if json {
                println!("{}", report::json(&intent, &verdicts));
            } else {
                print!("{}", report::table(&verdicts));
            }
            Ok(report::exit_code(&verdicts))
        }
        Verb::Apply { dry_run } => {
            let intent = Intent::load_default(&host)?;
            let (plan, outcome) = converge::apply(&intent, &host, dry_run)?;
            print_plan(&plan, dry_run);
            let exit = converge::apply_exit(&plan);
            if !dry_run {
                if !outcome.backups.is_empty() {
                    println!("BACKUPS {}", outcome.backups.len());
                    for backup in &outcome.backups {
                        println!("  {}", backup.display());
                    }
                }
                for pruned in &outcome.pruned {
                    println!("PRUNED {pruned}");
                }
                for reload in &outcome.reloads {
                    println!("RELOAD {reload}");
                }
                println!(
                    "SUMMARY planned={} applied={} failed=0 exit={}",
                    plan.changes.len() + plan.sysctl.len(),
                    outcome.applied.len(),
                    exit.code()
                );
            }
            Ok(exit)
        }
        Verb::Units => {
            let intent = Intent::load_default(&host)?;
            for unit in &intent.units {
                println!("{unit}");
            }
            Ok(Exit::Clean)
        }
        Verb::Harvest => {
            let intent = Intent::load_default(&host)?;
            print!("{}", converge::harvest(&intent, &host)?);
            Ok(Exit::Clean)
        }
        Verb::Converge { pass: which } => {
            let intent = Intent::load_default(&host)?;
            pass::run(&host, &intent, which.as_pass())?;
            Ok(Exit::Clean)
        }
        Verb::Import { state, profile_dir } => {
            let summary = import::run(&state, &profile_dir)?;
            println!(
                "IMPORTED profile={} mirror={} managed={} sysctl={}",
                profile_dir.display(),
                summary.mirror,
                summary.managed,
                summary.sysctl
            );
            for skipped in &summary.skipped {
                println!("  SKIPPED {skipped} (name marks a backup or editor artifact)");
            }
            for note in &summary.notes {
                println!("  NOTE {note}");
            }
            Ok(Exit::Clean)
        }
    }
}

fn print_plan(plan: &converge::Plan, dry_run: bool) {
    match &plan.profile {
        Some(name) => println!("PROFILE {name}"),
        None => println!("PROFILE none (base intent only)"),
    }
    // A mis-selected profile applies the wrong machine's facts; say so beside the plan, and
    // say it again after the apply, because that is when the operator is reading.
    for mismatch in &plan.profile_mismatch {
        eprintln!("  PROFILE-MISMATCH {mismatch}");
    }
    println!("PLAN {} change(s)", plan.changes.len() + plan.sysctl.len() + plan.anomalies.len());
    for change in &plan.changes {
        match change {
            converge::Planned::Owned { live, .. } => println!("  OWNED {live}"),
            converge::Planned::Managed { record } => {
                println!("  MANAGED {} ({})", record.target, record.name)
            }
            converge::Planned::Prune { live } => println!("  PRUNE {live} (no longer shipped)"),
        }
    }
    for drift in &plan.sysctl {
        println!(
            "  SYSCTL {}={} (live {})",
            drift.key,
            drift.want,
            drift.current.as_deref().unwrap_or("unreadable")
        );
    }
    for anomaly in &plan.anomalies {
        eprintln!("  ANOMALY {anomaly}");
    }
    println!("{}", report::scope_line(plan.profile.as_deref()));
    if dry_run {
        println!("DRY-RUN nothing written");
    }
}

/// Compile-time reminder that the failure type is the only error that reaches the exit code.
#[allow(dead_code)]
fn _assert_failure_conversion(e: std::io::Error) -> Failure {
    Failure::from(e)
}
