//! Cross-repo contract: the report's doc anchors must resolve in the ledger.
//!
//! The ledger lives in `system-tuning` (it is the home of judgement), while the registry lives
//! here. That is a seam between two repositories, and a seam without a test is a hope. This test
//! is the cheap half of the contract: an anchor that stops resolving turns red here, not after a
//! human goes looking for a rule that moved.

use std::path::Path;

use genarch::check::{self, Class, Reach};
use genarch::host::FixtureHost;
use genarch::intent::Intent;
use genarch::report;

const LEDGER: &str = "/home/<user>/Projects/system-tuning/docs/findings.md";

fn headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|rest| rest.trim().to_string())
        .collect()
}

#[test]
fn every_registry_anchor_resolves_in_the_ledger() {
    let path = Path::new(LEDGER);
    if !path.is_file() {
        eprintln!("skipping: no ledger at {}", path.display());
        return;
    }
    let text = std::fs::read_to_string(path).unwrap();
    let heads = headings(&text);
    let mut missing = Vec::new();
    for check in check::REGISTRY {
        let (prefix, slug) = match check.doc.split_once('#') {
            Some(parts) => parts,
            None => {
                missing.push(format!("{} -> {:?} (no anchor form)", check.id, check.doc));
                continue;
            }
        };
        if prefix != "findings" {
            missing.push(format!("{} -> {} (not a findings anchor)", check.id, check.doc));
            continue;
        }
        if !heads.iter().any(|h| h == slug) {
            missing.push(format!("{} -> ## {slug}", check.id));
        }
    }
    assert!(missing.is_empty(), "anchors that do not resolve: {missing:?}");
}

#[test]
fn ids_are_unique_and_grouped_by_reach() {
    let ids = check::ids();
    let unique: std::collections::BTreeSet<&&str> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate check id in the registry: {ids:?}");

    let mut full = 0;
    let mut file_only = 0;
    let mut none = 0;
    for check in check::REGISTRY {
        match check.reach {
            Reach::Full => full += 1,
            Reach::FileOnly => file_only += 1,
            Reach::None => none += 1,
        }
    }
    // The split is reported, so it is contract: an `apply`-reachable row must be distinguishable
    // from one only a service or a firmware can move.
    assert!(full > 0 && file_only > 0 && none > 0, "{full}/{file_only}/{none}");
    assert_eq!(full + file_only + none, ids.len());
}

#[test]
fn json_lists_every_registry_row_and_carries_the_scope() {
    let dir = std::env::temp_dir().join(format!("genarch-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("intent")).unwrap();
    std::fs::write(dir.join("intent/intent.toml"), "schema = 2\n").unwrap();
    std::fs::create_dir_all(dir.join("live")).unwrap();

    let intent = Intent::load(&dir.join("intent"), &dir.join("none")).unwrap();
    let host = FixtureHost::new(dir.join("live"));
    let verdicts = check::evaluate(&check::Ctx { host: &host, intent: &intent });

    let doc = report::json(&verdicts);
    for id in check::ids() {
        assert!(doc.contains(&format!("\"id\":\"{id}\"")), "{id} missing from the json document");
    }
    assert!(doc.starts_with("{\"schema\":1,"), "{doc}");
    assert!(report::scope_line().starts_with("APPLY-SCOPE full="), "{}", report::scope_line());

    // Class vocabulary is the operator's: all five must be expressible.
    for class in [Class::Ok, Class::Accepted, Class::Warn, Class::Drift, Class::Skipped] {
        assert!(!class.label().is_empty());
    }

    let _ = std::fs::remove_dir_all(&dir);
}
