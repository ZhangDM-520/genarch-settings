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
//! - **One write policy.** [`write_file`] owns temp naming, mode and owner inheritance, the
//!   `system-sleep/` staging rule and the kernel in-place branch. [`FixtureHost`] runs that
//!   same policy over [`RecordingFs`], so the gate cannot pass on a write the live apply
//!   would not perform.
//! - **Programs are not `std::process::Output`.** [`RunResult`] is constructible by both
//!   adapters, so no adapter has to fork a process to fake one.

use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
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

/// `systemd-sleep` executes every file in this directory, whatever its name.
const SYSTEM_SLEEP_DIR: &str = "/usr/lib/systemd/system-sleep";

/// Where a `system-sleep` write is staged, so an interrupted apply cannot leave an executable
/// temp the wake hook would run.
const SYSTEM_SLEEP_BACKUPS_DIR: &str = "/usr/lib/systemd/system-sleep-backups";

/// Staging directory for a live path, when it must not be the target's own parent.
/// One home for the rule: `system-sleep/` is executed wholesale, so nothing transient may sit
/// in it; every other target stages beside itself and stays atomic.
fn staging_live(live: &str) -> Option<&'static str> {
    match live.rsplit_once('/') {
        Some((dir, _)) if dir == SYSTEM_SLEEP_DIR => Some(SYSTEM_SLEEP_BACKUPS_DIR),
        _ => None,
    }
}

/// One open file, as the write policy sees it.
///
/// Mode and ownership are set here, on the descriptor, never on the path: a path-based
/// `chmod` after `close` follows a symlink planted in a user-writable directory.
pub trait FsFile {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    /// A single `write(2)`. A `sysfs` attribute's `store()` runs once per syscall and is not a
    /// stream, so a kernel write must never be split into a write plus a remainder.
    fn write_once(&mut self, bytes: &[u8]) -> std::io::Result<usize>;
    fn set_mode(&self, mode: u32) -> std::io::Result<()>;
    fn set_owner(&self, uid: u32, gid: u32) -> std::io::Result<()>;
    fn sync_all(&self) -> std::io::Result<()>;
}

/// What a target's metadata contributes to a write: the mode it keeps and the owner it keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMeta {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

/// The primitives [`write_file`] is expressed over. [`SystemFs`] is the machine;
/// [`RecordingFs`] performs the same operations for the fixture and keeps a log of them.
pub trait Fs {
    fn meta(&self, path: &Path) -> Option<FileMeta>;
    fn mkdir_all(&self, path: &Path) -> std::io::Result<()>;
    fn create_new(&self, path: &Path) -> std::io::Result<Box<dyn FsFile>>;
    fn open_existing(&self, path: &Path, truncate: bool) -> std::io::Result<Box<dyn FsFile>>;
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn remove(&self, path: &Path) -> std::io::Result<()>;
    fn sync_dir(&self, path: &Path);
    fn is_root(&self) -> bool;
}

/// The machine's primitives.
pub struct SystemFs;

struct RealFile {
    file: std::fs::File,
}

impl FsFile for RealFile {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.file.write_all(bytes)
    }

    fn write_once(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        use std::io::Write;
        self.file.write(bytes)
    }

    fn set_mode(&self, mode: u32) -> std::io::Result<()> {
        let rc = unsafe { libc::fchmod(self.file.as_raw_fd(), mode) };
        if rc == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
    }

    fn set_owner(&self, uid: u32, gid: u32) -> std::io::Result<()> {
        let rc = unsafe { libc::fchown(self.file.as_raw_fd(), uid, gid) };
        if rc == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
    }

    fn sync_all(&self) -> std::io::Result<()> {
        self.file.sync_all()
    }
}

impl Fs for SystemFs {
    fn meta(&self, path: &Path) -> Option<FileMeta> {
        std::fs::metadata(path).ok().map(|m| FileMeta {
            mode: m.permissions().mode(),
            uid: m.uid(),
            gid: m.gid(),
        })
    }

    fn mkdir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    /// `create_new` + `O_NOFOLLOW`: the temp path must not be a pre-planted symlink. Apply runs
    /// as root and writes into user-owned trees, so a symlink at a predictable name would
    /// redirect a root write.
    fn create_new(&self, path: &Path) -> std::io::Result<Box<dyn FsFile>> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        Ok(Box::new(RealFile { file }))
    }

    fn open_existing(&self, path: &Path, truncate: bool) -> std::io::Result<Box<dyn FsFile>> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(truncate)
            .open(path)?;
        Ok(Box::new(RealFile { file }))
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }

    fn sync_dir(&self, path: &Path) {
        if let Ok(dir) = std::fs::File::open(path) {
            let _ = dir.sync_all();
        }
    }

    fn is_root(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }
}

/// The write policy: one implementation, shared by the machine and the fixture.
///
/// A write is temp + fsync + rename, with mode and owner inherited from the target (or from the
/// directory a new target lands in). Kernel attributes bypass that path, and `system-sleep/`
/// stages its temp in the sibling backups directory.
pub fn write_file(
    fs: &dyn Fs,
    root: &Path,
    live: &str,
    path: &Path,
    bytes: &[u8],
    mode: Option<u32>,
) -> Result<()> {
    if kernel_fs(live) {
        return write_in_place(fs, root, path, bytes);
    }
    let existing = fs.meta(path);
    // A new target inherits the owner of the directory it lands in: the engine's mirrors
    // include files under a user's home, and a root-created file there is unusable to its
    // owner even after the content is right.
    let owner = existing
        .map(|m| (m.uid, m.gid))
        .or_else(|| path.parent().and_then(|p| fs.meta(p)).map(|m| (m.uid, m.gid)));
    if let Some(parent) = path.parent() {
        fs.mkdir_all(parent)
            .map_err(|e| Failure::partial(format!("mkdir {}: {e}", parent.display())))?;
    }
    // systemd-sleep executes every file in `system-sleep/`, whatever its name — including a
    // temp left by an interrupted apply. Stage those writes in the sibling backups directory
    // instead (same filesystem, so the rename stays atomic).
    let staging = match staging_live(live) {
        Some(dir) => {
            let dir = root.join(dir.trim_start_matches('/'));
            fs.mkdir_all(&dir)
                .map_err(|e| Failure::partial(format!("mkdir {}: {e}", dir.display())))?;
            dir
        }
        None => path.parent().map(Path::to_path_buf).unwrap_or_else(|| root.to_path_buf()),
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("genarch-settings");
    let tmp = staging.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = create_temp(fs, &tmp)?;
    file.write_all(bytes)
        .map_err(|e| Failure::partial(format!("write {}: {e}", tmp.display())))?;
    if let Some(m) = existing.map(|m| m.mode).or(mode) {
        file.set_mode(m)
            .map_err(|e| Failure::partial(format!("chmod {}: {e}", tmp.display())))?;
    }
    if let Some((uid, gid)) = owner {
        // A root run must preserve ownership or fail: silently leaving a root-owned file in a
        // user's home strips that user's ability to edit it (measured live: two
        // `~/.config/autostart` entries were root-owned after the cutover apply). An
        // unprivileged run can only chown to itself, which is already the case.
        if let Err(e) = file.set_owner(uid, gid)
            && fs.is_root()
        {
            return Err(Failure::partial(format!(
                "chown {uid}:{gid} {}: {e}",
                tmp.display()
            )));
        }
    }
    // The rename is only atomic if the data is on disk before it happens.
    file.sync_all()
        .map_err(|e| Failure::partial(format!("fsync {}: {e}", tmp.display())))?;
    drop(file);
    fs.rename(&tmp, path).map_err(|e| {
        let _ = fs.remove(&tmp);
        Failure::partial(format!("rename {}: {e}", path.display()))
    })?;
    if let Some(parent) = path.parent() {
        fs.sync_dir(parent);
    }
    Ok(())
}

/// Create the sibling temp file. A stale temp from a killed run carries our pid in its name,
/// so it is ours to replace.
fn create_temp(fs: &dyn Fs, path: &Path) -> Result<Box<dyn FsFile>> {
    match fs.create_new(path) {
        Ok(file) => Ok(file),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs.remove(path)
                .map_err(|e| Failure::partial(format!("remove stale {}: {e}", path.display())))?;
            fs.create_new(path)
                .map_err(|e| Failure::partial(format!("create {}: {e}", path.display())))
        }
        Err(e) => Err(Failure::partial(format!("create {}: {e}", path.display()))),
    }
}

/// Write a kernel attribute. No temp file, no chmod, no mkdir: the open is the write
/// permission on `sysfs`, and the file is already there.
///
/// **One `write(2)`, never `write_all`.** A `sysfs` attribute's `store()` is invoked once per
/// write syscall and is not a stream: a short write followed by the remainder would be
/// interpreted as a second, unrelated value (EINVAL, or a malformed setting).
fn write_in_place(fs: &dyn Fs, root: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    // A rehearsal root is a normal filesystem, where a shorter rewrite must not leave the tail
    // of the old value behind. Real `/sys` and `/proc` reject O_TRUNC.
    let truncate = root != Path::new("/");
    let mut file = fs
        .open_existing(path, truncate)
        .map_err(|e| Failure::partial(format!("write {}: {e}", path.display())))?;
    match file.write_once(bytes) {
        Ok(n) if n == bytes.len() => Ok(()),
        Ok(n) => Err(Failure::partial(format!(
            "write {}: short kernel write, {n} of {} bytes",
            path.display(),
            bytes.len()
        ))),
        Err(e) => Err(Failure::partial(format!("write {}: {e}", path.display()))),
    }
}

/// One primitive [`RecordingFs`] performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsOp {
    Create(PathBuf),
    Rename { from: PathBuf, to: PathBuf },
    Remove(PathBuf),
}

/// Performs every operation for real and records it, so the fixture runs exactly the policy the
/// machine runs and a test can still see where a temp was staged.
pub struct RecordingFs {
    inner: SystemFs,
    ops: std::cell::RefCell<Vec<FsOp>>,
}

impl RecordingFs {
    fn new() -> Self {
        RecordingFs { inner: SystemFs, ops: std::cell::RefCell::new(Vec::new()) }
    }

    fn recorded_ops(&self) -> Vec<FsOp> {
        self.ops.borrow().clone()
    }
}

impl Fs for RecordingFs {
    fn meta(&self, path: &Path) -> Option<FileMeta> {
        self.inner.meta(path)
    }

    fn mkdir_all(&self, path: &Path) -> std::io::Result<()> {
        self.inner.mkdir_all(path)
    }

    fn create_new(&self, path: &Path) -> std::io::Result<Box<dyn FsFile>> {
        let file = self.inner.create_new(path)?;
        self.ops.borrow_mut().push(FsOp::Create(path.to_path_buf()));
        Ok(file)
    }

    fn open_existing(&self, path: &Path, truncate: bool) -> std::io::Result<Box<dyn FsFile>> {
        // A live `/sys` and `/proc` always already contain the attribute; a fixture root is an
        // ordinary directory tree, so the file is materialised on first write. A test that
        // needs a refusing attribute puts a directory at the path, which still fails here.
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        }
        self.inner.open_existing(path, truncate)
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        self.inner.rename(from, to)?;
        self.ops.borrow_mut().push(FsOp::Rename {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        });
        Ok(())
    }

    fn remove(&self, path: &Path) -> std::io::Result<()> {
        self.inner.remove(path)?;
        self.ops.borrow_mut().push(FsOp::Remove(path.to_path_buf()));
        Ok(())
    }

    fn sync_dir(&self, path: &Path) {
        self.inner.sync_dir(path);
    }

    fn is_root(&self) -> bool {
        self.inner.is_root()
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
    let _ = std::io::Write::write_all(&mut file, format!("pid={} via=genarch-settings\n", std::process::id()).as_bytes());
    Ok(LockGuard { file })
}

/// One filesystem entry the engine can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// The observable result of running a program.
///
/// Deliberately not `std::process::Output`: that type has no public constructor, so the fixture
/// had to fork `/bin/sh` to fabricate one. `status` is the exit code, or `-1` when the process
/// died on a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub success: bool,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl RunResult {
    fn from_output(output: Output) -> Self {
        RunResult {
            success: output.status.success(),
            status: output.status.code().unwrap_or(-1),
            // A tool's stdout is not guaranteed to be UTF-8; the callers parse text, so the
            // lossy rendering is the honest one.
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
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

    /// Run a program and capture its output. Used for `ryzenadj`, `iw`, reloads and the unit
    /// checks.
    fn run(&self, argv: &[String]) -> Result<RunResult>;
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

    /// `GENARCH_SETTINGS_ROOT` is a rehearsal override, not a test seam — tests build a
    /// [`FixtureHost`] directly so the seam cannot drift from the tests' expectations.
    pub fn from_env() -> Self {
        match std::env::var_os("GENARCH_SETTINGS_ROOT") {
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
        write_file(&SystemFs, &self.root, live, &path, bytes, mode)
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

    fn run(&self, argv: &[String]) -> Result<RunResult> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| Failure::precondition("empty argv"))?;
        Command::new(program)
            .args(args)
            .output()
            .map(RunResult::from_output)
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
    fs: RecordingFs,
    recorded: std::cell::RefCell<BTreeMap<PathBuf, Vec<u8>>>,
    runs: std::cell::RefCell<Vec<Vec<String>>>,
    processes: std::cell::RefCell<BTreeMap<String, Canned>>,
}

impl FixtureHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FixtureHost {
            root: root.into(),
            fs: RecordingFs::new(),
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

    /// Every primitive the shared write policy performed, in order: where a temp was created,
    /// what it was renamed to. That is how a test sees the staging rule, not just the result.
    pub fn recorded_ops(&self) -> Vec<FsOp> {
        self.fs.recorded_ops()
    }

    pub fn recorded_runs(&self) -> Vec<Vec<String>> {
        self.runs.borrow().clone()
    }

    /// Teach the fixture host what a program prints (e.g. `ryzenadj --info`).
    pub fn with_process(self, program: &str, stdout: &str, status: i32) -> Self {
        self.processes.borrow_mut().insert(
            program.to_string(),
            Canned { stdout: stdout.to_string(), stderr: String::new(), status },
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
        write_file(&self.fs, &self.root, live, &path, bytes, mode)?;
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

    fn run(&self, argv: &[String]) -> Result<RunResult> {
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
        let reply = self.processes.borrow().get(&program).cloned().unwrap_or_default();
        Ok(RunResult {
            success: reply.status == 0,
            status: reply.status,
            stdout: reply.stdout,
            stderr: reply.stderr,
        })
    }
}

/// A canned reply for a fixture process.
#[derive(Debug, Clone, Default)]
pub struct Canned {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("genarch-settings-host-{}-{name}", std::process::id()));
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
    fn a_rehearsal_kernel_write_truncates_the_previous_value() {
        // On a real kernel filesystem O_TRUNC is refused and irrelevant; on a GENARCH_SETTINGS_ROOT
        // rehearsal it is a normal file, where a shorter value must not leave the old tail.
        let dir = scratch("rehearsal");
        let live = "/proc/sys/vm/swappiness";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "150\n").unwrap();

        let host = RealHost::rooted(&dir);
        host.write(live, b"10", None).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "10");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sleep_hook_writes_are_staged_outside_the_executed_directory() {
        // systemd-sleep runs every file in that directory, whatever its name. A temp left by a
        // crash between chmod and rename would execute on the next resume.
        assert_eq!(
            staging_live("/usr/lib/systemd/system-sleep/genarch-settings"),
            Some("/usr/lib/systemd/system-sleep-backups")
        );
        assert_eq!(staging_live("/usr/lib/systemd/system-sleep/nested/x"), None);
        assert_eq!(staging_live("/etc/tlp.d/01-eco.conf"), None);

        let dir = scratch("sleep-stage");
        let live = "/usr/lib/systemd/system-sleep/genarch-settings";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();

        let host = RealHost::rooted(&dir);
        host.write(live, b"#!/bin/sh\ntrue\n", Some(0o755)).unwrap();

        let written: Vec<PathBuf> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(written, vec![target.clone()], "only the hook may live in system-sleep/");
        let staged = dir.join("usr/lib/systemd/system-sleep-backups");
        assert!(staged.is_dir(), "staging directory must be created");
        let leftovers: Vec<PathBuf> = std::fs::read_dir(&staged).unwrap().flatten().map(|e| e.path()).collect();
        assert!(leftovers.is_empty(), "temp was not renamed away: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fixture_stages_a_sleep_hook_outside_the_executed_directory() {
        // The fixture runs the same write policy as the machine, so the guard has to hold
        // there too: what the gate proves is then what the apply does.
        let dir = scratch("fixture-sleep");
        let host = FixtureHost::new(&dir);
        let live = "/usr/lib/systemd/system-sleep/genarch-settings";
        host.write(live, b"#!/bin/sh\ntrue\n", Some(0o755)).unwrap();

        let sleep_dir = dir.join("usr/lib/systemd/system-sleep");
        let mut created = Vec::new();
        for op in host.recorded_ops() {
            match op {
                FsOp::Create(path) => created.push(path),
                FsOp::Rename { from, .. } => created.push(from),
                FsOp::Remove(_) => {}
            }
        }
        assert!(!created.is_empty(), "the write policy created nothing to observe");
        assert!(
            created.iter().all(|p| !p.starts_with(&sleep_dir)),
            "a temp was staged inside system-sleep/: {created:?}"
        );
        assert_eq!(
            std::fs::read_to_string(sleep_dir.join("genarch-settings")).unwrap(),
            "#!/bin/sh\ntrue\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fixture_keeps_an_existing_mode_and_leaves_no_temp() {
        let dir = scratch("fixture-mode");
        let live = "/etc/tlp.d/01-x.conf";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "old\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();

        let host = FixtureHost::new(&dir);
        host.write(live, b"new\n", Some(0o644)).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new\n");
        let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the fixture must not re-apply the source mode");
        let leftovers: Vec<PathBuf> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p != &target)
            .collect();
        assert!(leftovers.is_empty(), "fixture write left a temp file: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_canned_program_reports_its_status_without_forking_a_process() {
        // `RunResult` exists so the fixture can answer a `run` without fabricating a
        // `std::process::Output`; the status and streams must survive intact.
        let dir = scratch("run");
        let host = FixtureHost::new(&dir)
            .with_process("ryzenadj", "PPT LIMIT FAST = 28000\n", 0)
            .with_process("iw", "nope\n", 7);

        let ok = host.run(&["ryzenadj".into(), "--info".into()]).unwrap();
        assert!(ok.success);
        assert_eq!(ok.status, 0);
        assert!(ok.stdout.contains("28000"));
        assert!(ok.stderr.is_empty());

        let bad = host.run(&["/usr/bin/iw".into(), "dev".into()]).unwrap();
        assert!(!bad.success);
        assert_eq!(bad.status, 7);
        assert_eq!(bad.stdout, "nope\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ownership_is_preserved_for_an_existing_target() {
        // Root-written files in a user tree became root-owned (measured live: two
        // ~/.config/autostart entries after the cutover apply). As root the temp is created
        // 0:0, so the owner has to be carried over explicitly. This test needs root to be
        // meaningful; unprivileged, the file is already ours and the assertion is a no-op.
        let dir = scratch("owner");
        let live = "/home/someone/.config/autostart/app.desktop";
        let target = dir.join(live.trim_start_matches('/'));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "old\n").unwrap();
        let owner = (1000, 1000);
        if unsafe { libc::geteuid() } == 0 {
            let rc = unsafe {
                libc::chown(
                    std::ffi::CString::new(target.to_str().unwrap()).unwrap().as_ptr(),
                    owner.0,
                    owner.1,
                )
            };
            assert_eq!(rc, 0);
        }

        let host = RealHost::rooted(&dir);
        host.write(live, b"new\n", Some(0o644)).unwrap();

        let meta = std::fs::metadata(&target).unwrap();
        if unsafe { libc::geteuid() } == 0 {
            assert_eq!((meta.uid(), meta.gid()), owner, "root write reset the file owner");
        }
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_target_inherits_the_directory_owner() {
        // The engine mirrors files under a user's home; a root-created file there must be
        // usable by that user. Root-only observation, like the preservation test above.
        let dir = scratch("new-owner");
        let live = "/home/someone/.config/app/new.conf";
        let parent = dir.join("home/someone/.config/app");
        std::fs::create_dir_all(&parent).unwrap();
        let owner = (1000, 1000);
        if unsafe { libc::geteuid() } == 0 {
            let c = std::ffi::CString::new(parent.to_str().unwrap()).unwrap();
            assert_eq!(unsafe { libc::chown(c.as_ptr(), owner.0, owner.1) }, 0);
        }

        let host = RealHost::rooted(&dir);
        host.write(live, b"x\n", Some(0o644)).unwrap();

        let meta = std::fs::metadata(parent.join("new.conf")).unwrap();
        if unsafe { libc::geteuid() } == 0 {
            assert_eq!((meta.uid(), meta.gid()), owner, "new file did not inherit the directory owner");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_kernel_filesystems_skip_the_atomic_path() {
        assert!(kernel_fs("/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq"));
        assert!(kernel_fs("/proc/sys/vm/swappiness"));
        assert!(!kernel_fs("/etc/sysctl.d/90-genarch-settings.conf"));
        assert!(!kernel_fs("/var/lib/genarch-settings/manifest.toml"));
    }
}
