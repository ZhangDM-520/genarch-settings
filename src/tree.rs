//! Package trees: walking them, deciding what may ship, and mapping a file to its live path.
//!
//! This module exists because the same two rules lived in three places: `intent` and `import`
//! each walked a tree, and `intent::validate` called `converge::is_deployable_name` while
//! `converge` imported `intent` — a cycle around the two facts that are pure functions of a
//! path. Nothing here reads live host state: a package tree is input data, not the machine.
//!
//! One home for "what may ship" matters beyond tidiness. A stray `*.bak` in `mirror/` would be
//! deployed, and in `system-sleep/` it would be *executed* on every resume, so the rule is
//! applied at load time by both the loader and the importer through [`is_deployable_name`].

use std::path::{Path, PathBuf};

/// Every regular file under `dir`, recursively, sorted.
///
/// A missing directory is empty, not an error: a layer that ships no mirrors is legal.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else { continue };
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

/// Names that must never be deployed, even if they appear in a package tree.
///
/// `*.bak-YYYYMMDD` is the pre-image convention itself, so the suffix check cannot be a plain
/// `.bak` extension test.
pub fn is_deployable_name(name: &str) -> bool {
    const BAD_SUFFIXES: &[&str] = &[".bak", "~", ".orig", ".rej", ".swp", ".tmp"];
    if name.starts_with(".#") {
        return false;
    }
    if BAD_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return false;
    }
    !name.contains(".bak-") && name != ".bak"
}

/// The live path a file under `root` deploys to, e.g. `/etc/tlp.d/01-eco.conf`.
pub fn live_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(format!("/{}", rel.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_deployable_names_deploy() {
        for name in ["01-eco.conf", "genarch-settings", "fstab", "10-user-tmp.conf"] {
            assert!(is_deployable_name(name), "{name} must deploy");
        }
        for name in [
            "01-eco.conf.bak",
            "01-eco.conf.bak-20261111",
            "fstab~",
            "fstab.orig",
            "fstab.rej",
            ".fstab.swp",
            "x.tmp",
            ".#fstab",
            ".bak",
        ] {
            assert!(!is_deployable_name(name), "{name} must never deploy");
        }
    }

    #[test]
    fn a_file_maps_to_its_absolute_live_path() {
        let root = Path::new("/pkg/mirror");
        assert_eq!(
            live_path(root, Path::new("/pkg/mirror/etc/tlp.d/01.conf")).as_deref(),
            Some("/etc/tlp.d/01.conf")
        );
        assert_eq!(live_path(root, Path::new("/somewhere/else")), None);
    }
}
