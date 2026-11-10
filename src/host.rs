//! The seam to the machine.
//!
//! Everything above this module works against [`Host`]; only [`RealHost`] touches the
//! real filesystem, and only [`FixtureHost`] is used by tests. That is deliberate: the
//! shell engine's whole testability rested on one path-prefix seam (`ECO_ROOT`), and a
//! rewrite that reads `/sys` or `/etc` directly from the modules would have no seam at
//! all. Two adapters make it a real seam, not a hypothetical one.
//!
//! Rules this module enforces, because the shell engine paid for them:
//!
//! - **No write escapes the root.** A path outside it is a hard error, not a skippable
//!   step (the shell's `assert_in_root`).
//! - **A write is atomic.** Content goes to a sibling temp file and is renamed, so a
//!   crash mid-apply cannot leave a half-written config (the shell's `managed_commit_tmp`).
//! - **Mode is explicit.** Source mode wins for a new target; an existing target keeps
//!   its mode unless the caller says otherwise (an exec-bit regression is invisible until
//!   a unit fails with 203/EXEC).

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use crate::exit::{Failure, Result};

/// One filesystem entry the engine can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Everything the engine needs from the machine, and nothing more.
pub trait Host {
    /// Root the host resolves paths against (`/` for the real machine).
    fn root(&self) -> &Path;

    /// Resolve an absolute live path into the host's namespace, for display only.
    /// I/O must go through [`Host::try_resolve`], which can refuse an escaping path.
    fn resolve(&self, live: &str) -> PathBuf {
        self.try_resolve(live)
            .unwrap_or_else(|_| self.root().join("__refused__"))
    }

    /// Fallible resolution. The default is the one rule every adapter must share.
    fn try_resolve(&self, live: &str) -> Result<PathBuf> {
        resolve_under(self.root(), live)
    }

    fn exists(&self, live: &str) -> bool;

    fn read(&self, live: &str) -> Result<String>;

    /// Read a file that may legitimately be absent, returning `None` instead of failing.
    fn read_opt(&self, live: &str) -> Option<String> {
        if self.exists(live) { self.read(live).ok() } else { None }
    }

    /// Write `bytes` to a live path atomically, creating parents.
    ///
    /// `mode` is applied only when the target does not exist yet.
    fn write(&self, live: &str, bytes: &[u8], mode: Option<u32>) -> Result<()>;

    /// All regular files under a live directory, recursively, sorted.
    fn walk(&self, live_dir: &str) -> Result<Vec<Entry>>;

    /// Remove a live file this engine owns (prune). A missing file is not an error: the
    /// goal is "not present", and re-running must stay idempotent.
    fn remove(&self, live: &str) -> Result<()>;

    /// Run a program and capture its output. Used for `ryzenadj` and reloads.
    fn run(&self, argv: &[String]) -> Result<Output>;
}

/// The live machine.
pub struct RealHost {
    root: PathBuf,
}

impl RealHost {
    pub fn new() -> Self {
        RealHost { root: PathBuf::from("/") }
    }

    /// Rehearsal adapter: the same code paths, aimed at a copy of the live tree.
    /// This is how a cutover plan is reviewed before it touches the machine.
    pub fn rooted(root: impl Into<PathBuf>) -> Self {
        RealHost { root: root.into() }
    }

    /// `GENARCH_ROOT` is a rehearsal override, not a test seam — tests build a
    /// [`FixtureHost`] directly so the seam cannot drift from the tests' expectations.
    pub fn from_env() -> Self {
        match std::env::var_os("GENARCH_ROOT") {
            Some(root) if !root.is_empty() => RealHost::rooted(root),
            _ => RealHost::new(),
        }
    }
}

impl Default for RealHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Join a live path onto the adapter's root.
///
/// A `..` component would escape the root and make the seam a lie, so it is refused
/// rather than normalised away: `../etc/passwd` must fail loudly, not silently become
/// something else.
fn resolve_under(root: &Path, live: &str) -> Result<PathBuf> {
    let rel = live.trim_start_matches('/');
    let rel_path = Path::new(rel);
    for component in rel_path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            other => {
                return Err(Failure::partial(format!(
                    "refusing path with a {other:?} component: {live}"
                )));
            }
        }
    }
    Ok(root.join(rel_path))
}

impl Host for RealHost {
    fn root(&self) -> &Path {
        &self.root
    }

    fn exists(&self, live: &str) -> bool {
        self.try_resolve(live).map(|p| p.exists()).unwrap_or(false)
    }

    fn read(&self, live: &str) -> Result<String> {
        let path = self.try_resolve(live)?;
        std::fs::read_to_string(&path)
            .map_err(|e| Failure::partial(format!("read {}: {e}", path.display())))
    }

    fn write(&self, live: &str, bytes: &[u8], mode: Option<u32>) -> Result<()> {
        let path = self.try_resolve(live)?;
        let existing_mode = std::fs::metadata(&path).ok().map(|m| m.permissions().mode());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Failure::partial(format!("mkdir {}: {e}", parent.display())))?;
        }
        let tmp = path.with_extension(format!(
            "{}.tmp",
            path.extension().and_then(|e| e.to_str()).unwrap_or("genarch")
        ));
        std::fs::write(&tmp, bytes)
            .map_err(|e| Failure::partial(format!("write {}: {e}", tmp.display())))?;
        let effective = existing_mode.or(mode);
        if let Some(m) = effective {
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(m))
                .map_err(|e| Failure::partial(format!("chmod {}: {e}", tmp.display())))?;
        }
        std::fs::rename(&tmp, &path)
            .map_err(|e| Failure::partial(format!("rename {}: {e}", path.display())))?;
        Ok(())
    }

    fn walk(&self, live_dir: &str) -> Result<Vec<Entry>> {
        let dir = self.try_resolve(live_dir)?;
        let mut out = Vec::new();
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let entries = match std::fs::read_dir(&d) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(Failure::partial(format!("readdir {}: {e}", d.display())));
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    stack.push(path.clone());
                }
                out.push(Entry { path, is_dir });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    fn remove(&self, live: &str) -> Result<()> {
        let path = self.try_resolve(live)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Failure::partial(format!("remove {}: {e}", path.display()))),
        }
    }

    fn run(&self, argv: &[String]) -> Result<Output> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| Failure::precondition("empty argv"))?;
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| Failure::partial(format!("run {program}: {e}")))
    }
}

/// A host that lives entirely under a directory: the fixture tree the gate runs on.
///
/// It refuses anything the real host would refuse, but never touches the machine — and
/// in particular it can record writes instead of performing them, which is how a plan is
/// reviewed without a dry-run flag threaded through every writer.
pub struct FixtureHost {
    root: PathBuf,
    recorded: std::cell::RefCell<BTreeMap<PathBuf, Vec<u8>>>,
    runs: std::cell::RefCell<Vec<Vec<String>>>,
    processes: std::cell::RefCell<BTreeMap<String, Canned>>,
}

impl FixtureHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FixtureHost {
            root: root.into(),
            recorded: std::cell::RefCell::new(BTreeMap::new()),
            runs: std::cell::RefCell::new(Vec::new()),
            processes: std::cell::RefCell::new(BTreeMap::new()),
        }
    }

    /// Every write the engine attempted, in path order. Tests assert on this instead of
    /// scraping a directory, so a write that should not have happened is visible even if
    /// it happened to be byte-identical.
    pub fn recorded_writes(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.recorded.borrow().clone()
    }

    pub fn recorded_runs(&self) -> Vec<Vec<String>> {
        self.runs.borrow().clone()
    }

    /// Teach the fixture host what a program prints (e.g. `ryzenadj --info`).
    pub fn with_process(self, program: &str, stdout: &str, status: i32) -> Self {
        self.processes.borrow_mut().insert(
            program.to_string(),
            Canned { stdout: stdout.to_string(), stdout_err: String::new(), status },
        );
        self
    }
}

impl Host for FixtureHost {
    fn root(&self) -> &Path {
        &self.root
    }

    fn exists(&self, live: &str) -> bool {
        self.try_resolve(live).map(|p| p.exists()).unwrap_or(false)
    }

    fn read(&self, live: &str) -> Result<String> {
        let path = self.try_resolve(live)?;
        std::fs::read_to_string(&path)
            .map_err(|e| Failure::partial(format!("read {}: {e}", path.display())))
    }

    fn write(&self, live: &str, bytes: &[u8], mode: Option<u32>) -> Result<()> {
        let path = self.try_resolve(live)?;
        if !path.starts_with(&self.root) {
            return Err(Failure::partial(format!(
                "refusing write outside the host root: {}",
                path.display()
            )));
        }
        let existing_mode = std::fs::metadata(&path).ok().map(|m| m.permissions().mode());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Failure::partial(format!("mkdir {}: {e}", parent.display())))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| Failure::partial(format!("write {}: {e}", path.display())))?;
        let effective = existing_mode.or(mode);
        if let Some(m) = effective {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(m))
                .map_err(|e| Failure::partial(format!("chmod {}: {e}", path.display())))?;
        }
        self.recorded.borrow_mut().insert(path, bytes.to_vec());
        Ok(())
    }

    fn walk(&self, live_dir: &str) -> Result<Vec<Entry>> {
        let dir = self.try_resolve(live_dir)?;
        let mut out = Vec::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            let entries = match std::fs::read_dir(&d) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(Failure::partial(format!("readdir {}: {e}", d.display())));
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    stack.push(path.clone());
                }
                out.push(Entry { path, is_dir });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    fn remove(&self, live: &str) -> Result<()> {
        let path = self.try_resolve(live)?;
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound {
                return Err(Failure::partial(format!("remove {}: {e}", path.display())));
            }
        self.recorded.borrow_mut().remove(&path);
        Ok(())
    }

    fn run(&self, argv: &[String]) -> Result<Output> {
        // Nothing is executed. The intent is recorded so a test can assert argv (the
        // ryzenadj packing is exactly an argv property), and the reply comes from a
        // canned table so a caller that parses stdout still works.
        self.runs.borrow_mut().push(argv.to_vec());
        let program = argv
            .first()
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
                    .to_string()
            })
            .unwrap_or_default();
        let reply = self.processes.borrow().get(&program).cloned().unwrap_or(Canned {
            stdout: String::new(),
            stdout_err: String::new(),
            status: 0,
        });
        Ok(canned_output(reply))
    }
}

/// A canned reply for a fixture process.
#[derive(Debug, Clone, Default)]
pub struct Canned {
    pub stdout: String,
    pub stdout_err: String,
    pub status: i32,
}

fn canned_output(canned: Canned) -> Output {
    // `Output` has no public constructor, so it is produced by running a trivial process.
    // The cost is one short fork per canned call in tests; the alternative is leaking a
    // private type through the trait, which would make the seam depend on the test helper.
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(if canned.status == 0 {
        "cat"
    } else {
        "cat >&2; exit 1"
    });
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn canned process");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        let _ = stdin.write_all(canned.stdout.as_bytes());
    }
    child.wait_with_output().expect("canned process output")
}
