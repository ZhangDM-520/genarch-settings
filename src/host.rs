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
//! - **A write is atomic where the filesystem allows it.** Content goes to a sibling temp
//!   file (`O_EXCL|O_NOFOLLOW`, fsynced) and is renamed, so a crash mid-apply cannot leave a
//!   half-written config (the shell's `managed_commit_tmp`). `/sys` and `/proc` cannot host a
//!   temp file at all — `sysfs` creates on open and `procfs` fixes its directory — so kernel
//!   attributes are written in place; see [`kernel_fs`].
//! - **Mode is explicit.** Source mode wins for a new target; an existing target keeps
//!   its mode unless the caller says otherwise (an exec-bit regression is invisible until
//!   a unit fails with 203/EXEC).

use std::collections::BTreeMap;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use crate::exit::{Failure, Result};

/// Kernel pseudo-filesystems that must be written in place.
///
/// `sysfs` and `procfs` create files as a side effect of an open, so neither a sibling temp
/// file nor a rename exists there: an atomic-write path aimed at `/sys` fails with EPERM and
/// leaves the knob unchanged (measured 2026-11-11 on `scaling_max_freq` and
/// `/proc/sys/kernel/printk`). Those writes are inherently tiny and non-atomic by nature, so
/// they go straight to the existing file and never touch the mode.
fn kernel_fs(live: &str) -> bool {
    live.starts_with("/sys/") || live.starts_with("/proc/")
}

/// Write to an existing kernel attribute. No temp file, no chmod, no mkdir: the open itself
/// is the write permission on `sysfs`, and the file is already there.
fn write_in_place(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| Failure::partial(format!("write {}: {e}", path.display())))?;
    file.write_all(bytes)
        .map_err(|e| Failure::partial(format!("write {}: {e}", path.display())))?;
    file.flush()
        .map_err(|e| Failure::partial(format!("write {}: {e}", path.display())))
}

/// Create the sibling temp file with `O_EXCL|O_NOFOLLOW`. A stale temp from a killed run
/// carries our pid in its name, so it is ours to replace.
fn open_new(path: &Path) -> Result<std::fs::File> {
    let open = || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    };
    match open() {
        Ok(file) => Ok(file),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(path)
                .map_err(|e| Failure::partial(format!("remove stale {}: {e}", path.display())))?;
            open().map_err(|e| Failure::partial(format!("create {}: {e}", path.display())))
        }
        Err(e) => Err(Failure::partial(format!("create {}: {e}", path.display()))),
    }
}

/// An exclusive, non-blocking apply lock. Dropping it releases the lock.
pub struct LockGuard {
    file: std::fs::File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn lock_at(path: &Path) -> Result<LockGuard> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Failure::precondition(format!("mkdir {}: {e}", parent.display())))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| Failure::precondition(format!("open {}: {e}", path.display())))?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let mut holder = String::new();
        let _ = std::io::Read::read_to_string(&mut file, &mut holder);
        return Err(Failure::precondition(format!(
            "apply lock held by {} ({})",
            holder.trim(),
            path.display()
        )));
    }
    let _ = file.set_len(0);
    let _ = std::io::Write::write_all(&mut file, format!("pid={} via=genarch\n", std::process::id()).as_bytes());
    Ok(LockGuard { file })
}

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
    ///
    /// Kernel attributes under `/sys` and `/proc` are written in place: those filesystems
    /// cannot host a sibling temp file or a rename, so the atomic path would fail there.
    fn write(&self, live: &str, bytes: &[u8], mode: Option<u32>) -> Result<()>;

    /// Read a live file as bytes. Mirrors may be opaque (non-UTF-8), and a pre-image must be
    /// byte-exact, so this is not a convenience wrapper around [`Host::read`].
    fn read_bytes(&self, live: &str) -> Result<Vec<u8>> {
        let path = self.try_resolve(live)?;
        std::fs::read(&path)
            .map_err(|e| Failure::partial(format!("read {}: {e}", path.display())))
    }

    /// Permission bits of an existing live file, if it exists. A pre-image copies them:
    /// an exec-bit lost in a backup is invisible until a unit fails with 203/EXEC.
    fn mode_of(&self, live: &str) -> Option<u32> {
        let path = self.try_resolve(live).ok()?;
        std::fs::metadata(&path).ok().map(|m| m.permissions().mode())
    }

    /// Exclusive, non-blocking apply lock at `live`. A second holder gets a precondition
    /// failure naming the first, never interleaved writes.
    fn lock(&self, live: &str) -> Result<LockGuard>;

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
        if kernel_fs(live) {
            return write_in_place(&path, bytes);
        }
        let existing_mode = std::fs::metadata(&path).ok().map(|m| m.permissions().mode());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Failure::partial(format!("mkdir {}: {e}", parent.display())))?;
        }
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("genarch");
        let tmp = path.with_extension(format!("{extension}.{}.tmp", std::process::id()));
        // `create_new` + `O_NOFOLLOW`: the temp path must not be a pre-planted symlink. Apply
        // runs as root and writes into user-owned trees, so a symlink at a predictable name
        // would redirect a root write.
        let mut file = open_new(&tmp)?;
        use std::io::Write;
        file.write_all(bytes)
            .map_err(|e| Failure::partial(format!("write {}: {e}", tmp.display())))?;
        // The rename is only atomic if the data is on disk before it happens.
        file.sync_all()
            .map_err(|e| Failure::partial(format!("fsync {}: {e}", tmp.display())))?;
        drop(file);
        let effective = existing_mode.or(mode);
        if let Some(m) = effective {
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(m))
                .map_err(|e| Failure::partial(format!("chmod {}: {e}", tmp.display())))?;
        }
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            Failure::partial(format!("rename {}: {e}", path.display()))
        })?;
        if let Some(parent) = path.parent()
            && let Ok(dir) = std::fs::File::open(parent)
        {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn lock(&self, live: &str) -> Result<LockGuard> {
        lock_at(&self.try_resolve(live)?)
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

    fn lock(&self, live: &str) -> Result<LockGuard> {
        lock_at(&self.try_resolve(live)?)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("genarch-host-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn kernel_attributes_are_written_in_place() {
        // sysfs/procfs refuse a sibling temp file outright, so an atomic write there fails and
        // leaves the knob unchanged (measured 2026-11-11: `touch /proc/sys/kernel/printk.tmp`
        // → ENOENT, `.tmp` under a policy dir → EACCES). The fixture reproduces the one
        // property that matters: the directory cannot accept a new entry, but the existing
        // attribute can be written.
        let dir = scratch("kernel");
        let live = "/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "4500000").unwrap();
        std::fs::set_permissions(
            target.parent().unwrap(),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();

        let host = RealHost::rooted(&dir);
        host.write(live, b"4700000", None).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "4700000");
        let _ = std::fs::set_permissions(
            target.parent().unwrap(),
            std::fs::Permissions::from_mode(0o755),
        );
        let leftovers: Vec<PathBuf> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p != &target)
            .collect();
        assert!(leftovers.is_empty(), "kernel write left a temp file: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_writes_are_atomic_and_keep_an_existing_mode() {
        let dir = scratch("atomic");
        let live = "/etc/tlp.d/01-x.conf";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "old\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();

        let host = RealHost::rooted(&dir);
        host.write(live, b"new\n", Some(0o644)).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new\n");
        let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "an existing target keeps its mode");
        let leftovers: Vec<PathBuf> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p != &target)
            .collect();
        assert!(leftovers.is_empty(), "atomic write left a temp file: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_kernel_filesystems_skip_the_atomic_path() {
        assert!(kernel_fs("/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq"));
        assert!(kernel_fs("/proc/sys/vm/swappiness"));
        assert!(!kernel_fs("/etc/sysctl.d/90-genarch.conf"));
        assert!(!kernel_fs("/var/lib/genarch/manifest.toml"));
    }
}
