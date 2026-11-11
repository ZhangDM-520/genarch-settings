//! Cross-repo contract: the report's doc anchors must resolve in the ledger.
//!
//! The ledger lives in `system-tuning` (it is the home of judgement), while the registry lives
//! here. That is a seam between two repositories, and a seam without a test is a hope. This test
//! is the cheap half of the contract: an anchor that stops resolving turns red here, not after a
//! human goes looking for a rule that moved.


use genarch::check::{self, Class, Reach};
use genarch::host::FixtureHost;
use genarch::intent::{Intent, SCHEMA_VERSION};
use genarch::report;

/// The ledger repo. `SYSTEM_TUNING_ROOT` exists for a checkout somewhere else; the default is
/// the canonical path.
fn ledger_root() -> std::path::PathBuf {
    std::env::var_os("SYSTEM_TUNING_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/home/<user>/Projects/system-tuning"))
}

fn contract_rows(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let (id, anchor) = line.split_once('\t').expect("contract row is ID<TAB>anchor");
            (id.to_string(), anchor.to_string())
        })
        .collect()
}

fn headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|rest| rest.trim().to_string())
        .collect()
}

#[test]
fn every_registry_anchor_resolves_in_the_ledger() {
    // A missing ledger used to return green. That is the contract failing open: the very
    // drift this test exists to catch would have been invisible on any machine without a
    // checkout at the hardcoded path.
    let path = ledger_root().join("docs/findings.md");
    assert!(path.is_file(), "no ledger at {} — set SYSTEM_TUNING_ROOT", path.display());
    let text = std::fs::read_to_string(&path).unwrap();
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
    let mut intent = 0;
    let mut none = 0;
    for check in check::REGISTRY {
        match check.reach {
            Reach::Full => full += 1,
            Reach::FileOnly => file_only += 1,
            Reach::Intent => intent += 1,
            Reach::None => none += 1,
        }
    }
    // The split is reported, so it is contract: an `apply`-reachable row must be distinguishable
    // from one only a service or a firmware can move, and from one judged against the intent
    // alone (no live observation exists).
    assert!(full > 0 && file_only > 0 && none > 0, "{full}/{file_only}/{intent}/{none}");
    assert!(intent > 0, "an intent-only row must say so");
    assert_eq!(full + file_only + intent + none, ids.len());
}

#[test]
fn json_lists_every_registry_row_and_carries_the_scope() {
    let dir = std::env::temp_dir().join(format!("genarch-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("intent")).unwrap();
    std::fs::write(dir.join("intent/intent.toml"), format!("schema = {SCHEMA_VERSION}\n")).unwrap();
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

#[test]
fn the_pinned_contract_equals_the_registry_and_the_owner_copy() {
    // "One home, one copy, asserted equal" was the design. Without this test the pinned file
    // is decoration: the registry could gain, lose or rename an anchor and the two copies
    // would drift until somebody read both.
    let pinned = std::fs::read_to_string("tests/contract.tsv").unwrap();
    let rows = contract_rows(&pinned);
    let from_registry: Vec<(String, String)> = check::REGISTRY
        .iter()
        .map(|c| (c.id.to_string(), c.doc.to_string()))
        .collect();
    assert_eq!(rows, from_registry, "tests/contract.tsv and the registry disagree");

    let owner = ledger_root().join("docs/contract.tsv");
    assert!(owner.is_file(), "no owner copy at {} — set SYSTEM_TUNING_ROOT", owner.display());
    assert_eq!(
        std::fs::read_to_string(&owner).unwrap(),
        pinned,
        "the pinned copy and system-tuning's owner copy disagree"
    );
}
