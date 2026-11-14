# Uninstall leaves the deployed /etc files, the unit files and the manifest, with no documented restore

**Severity:** medium
**Category:** doc

**Environment:** Arch guest; `genarch-settings 0.4.0-1`; staging tree `~/machine/root.e2e`
converged with `GENARCH_SETTINGS_ROOT` (a copy of the staging tree, so no real `/etc` was
touched; `root.e2e` is a copy of `root.pristine`, a byte copy of the delivered `~/machine/root`
taken before any apply).

**Intent:** Remove the package and know exactly what it leaves behind, and be able to get a
box back to how it was before I installed it.

**Steps:**

```sh
# what the package owns on the live system
pacman -Ql genarch-settings | grep '^genarch-settings /etc/'
pacman -Ql genarch-settings | grep '^genarch-settings /var/lib/'

# what an apply writes (staging copy)
GENARCH_SETTINGS_ROOT=$HOME/machine/root.e2e genarch-settings apply
python - <<'EOF'
import tomllib
d=tomllib.load(open("/home/arch/machine/root.e2e/var/lib/genarch-settings/manifest.toml","rb"))
print("owned", len(d["owned"]), "touched", len(d["touched"]))
EOF
find ~/machine/root.e2e -name '*.bak-*'
ls ~/machine/root.e2e/etc/systemd/system/genarch-settings*

# the removal hook
sed -n '/^pre_remove/,/^}/p' /var/lib/pacman/local/genarch-settings-0.4.0-1/install
```

**Expected:** `install.sh` documents the uninstall as:

> Uninstall: `systemctl disable` the units `genarch-settings units` prints, remove the binary,
> the intent directory and the documentation, then run `systemctl daemon-reload`.

For "know exactly what it leaves behind", that list should cover everything the engine wrote.

**Actual:** The package owns only `/etc/genarch-settings/profile` on the live filesystem and
nothing under `/var/lib`. A converged apply writes 40 owned targets (39 mirrors plus
`/etc/sysctl.d/90-genarch-settings.conf`), records 41 paths in the manifest (the 40 plus the
deselected `zenbook-s16` profile's dormant `98-bt-autosuspend.rules`), patches 2 foreign files
(`touched`), deploys 3 unit files under `/etc/systemd/system`, and creates 2 pre-images. None
of those are package files, so `pacman -R` cannot remove them. The removal hook only disables
the two units; it does not remove the unit files, the mirrors, the sysctl projection, the
manifest or the pre-images, and there is no verb or documented procedure to restore the
pre-images.

**Evidence:**

Package-owned live paths:

```
$ pacman -Ql genarch-settings | grep '^genarch-settings /etc/'
genarch-settings /etc/
genarch-settings /etc/genarch-settings/
genarch-settings /etc/genarch-settings/profile

$ pacman -Ql genarch-settings | grep '^genarch-settings /var/lib/'
(no output)
```

Artifacts of a converged staging apply:

```
$ python - <<'EOF'
import tomllib
p="/home/arch/machine/root.e2e/var/lib/genarch-settings/manifest.toml"
d=tomllib.load(open(p,"rb"))
print("owned", len(d["owned"]))
print("touched", len(d["touched"]))
for t in d["touched"]: print("touched:", t)
EOF
owned 41
touched 2
touched: /etc/systemd/journald.conf
touched: /usr/lib/systemd/system/power-profiles-daemon.service

$ find ~/machine/root.e2e -name '*.bak-*'
/home/arch/machine/root.e2e/etc/systemd/journald.conf.bak-20261114
/home/arch/machine/root.e2e/usr/lib/systemd/system/power-profiles-daemon.service.bak-20261114

$ ls ~/machine/root.e2e/etc/systemd/system/genarch-settings*
/home/arch/machine/root.e2e/etc/systemd/system/genarch-settings-ceil.service
/home/arch/machine/root.e2e/etc/systemd/system/genarch-settings-ceil.timer
/home/arch/machine/root.e2e/etc/systemd/system/genarch-settings.service
```

Removal hook:

```
pre_remove() {
  enabled=$(units)
  if [ -n "$enabled" ]; then
    systemctl disable $enabled 2>/dev/null || true
  fi
}
```

The package's own `install.sh` uninstall sentence says nothing about the 41 files, the
manifest, the pre-images or the deployed unit files.

**Why it matters:** on a real host, `pacman -R genarch-settings` succeeds and leaves the
machine partly tuned: 39 mirror files, a sysctl projection, three unit files (two of them
dangling, pointing at a removed binary), a patched `journald.conf`, a patched PPD unit and the
ownership record. The `.bak-YYYYMMDD` files are the only way back, and the material never says
how to use them. "Clean uninstall" is not clean, and nothing tells me what remains.

**What I could not determine:** the exact leftover set after a real-host apply, because the
review rules forbid using the real `/etc` as scratch; the set above is the staging apply's, and
those paths are provably outside the package's file list. Whether `pacman -R` saves a modified
`/etc/genarch-settings/profile` as `.pacsave` (it is a package file).
