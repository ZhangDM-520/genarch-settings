# Uninstall as documented leaves everything the engine wrote: 41 owned files, the manifest, the units and the patched vendor unit

- **Severity**: medium
- **Category**: doc
- **Environment**: genarch-settings 0.4.0-1, Arch Linux guest; a converged rehearsal tree under `~/machine/root`, 2026-11-14.

## Intent

The only uninstall instruction is in `install.sh`:

> Uninstall: `systemctl disable` the units `genarch-settings units` prints, remove
> the binary, the intent directory and the documentation, then run `systemctl daemon-reload`.

An uninstall has to undo what the apply did, or at least name the files it leaves. Rollback must be findable without guessing.

## Steps

1. Converge a copy: `GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply`
2. Read the ownership record: `cat ~/machine/root/var/lib/genarch-settings/manifest.toml`
3. Check what the package owns: `pacman -Ql genarch-settings | awk '{print $2}' | grep '^/etc/'`
4. Check a representative deployed path and the patched vendor unit.
5. On the installed machine, `sudo pacman -R --noconfirm genarch-settings`, then `ls /var/lib/genarch-settings`.
6. Try to find an unapply/restore verb: `genarch-settings --help`

## Expected

The uninstall instruction should cover the manifest's `owned` and `touched` sets (or the engine should ship a verb that restores the pre-images and removes what it owns), so a user can leave the host as it was.

## Actual

After a converged apply the manifest records 41 owned paths and 2 touched paths. The package owns only `/etc/genarch-settings/profile`; none of the deployed mirrors, units or the sysctl projection is a package file, so `pacman -R` removes none of them. The instruction mentions neither `/etc` targets nor `/var/lib/genarch-settings` nor the patched `/usr/lib/systemd/system/power-profiles-daemon.service`. There is no `unapply`/`restore` verb. The dated pre-images are beside each target, but you have to know that and restore them by hand.

## Evidence

```
$ GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply
...
SUMMARY planned=... applied=... failed=0 exit=0

$ cat ~/machine/root/var/lib/genarch-settings/manifest.toml
owned = ["/etc/NetworkManager/conf.d/99-eco-dns.conf", ..., "/usr/local/lib/iw-set-regdomain"]
touched = ["/etc/systemd/journald.conf", "/usr/lib/systemd/system/power-profiles-daemon.service"]
```

(41 paths in `owned`, 2 in `touched`; the line above is elided for width.)

```
$ pacman -Ql genarch-settings | awk '{print $2}' | grep '^/etc/'
/etc/
/etc/genarch-settings/
/etc/genarch-settings/profile
```

```
$ grep -n -A2 "Uninstall" /usr/share/doc/genarch-settings/install.sh
49:reset anything.  Uninstall: `systemctl disable` the units `genarch-settings units` prints, remove
50-the binary, the intent directory and the documentation, then run `systemctl daemon-reload`.
```

The engine-created files are still there after removing the package (the example below is from a real `pacman -R` on the installed machine; because no apply had ever succeeded there, only the lock directory is present — a host where apply *did* succeed keeps the 41 paths above):

```
$ sudo pacman -R --noconfirm genarch-settings; echo $?
0
$ ls -la /var/lib/genarch-settings
drwxr-xr-x 1 root root   8 ... .
drwxr-xr-x 1 root root 206 ... ..
-rw-r--r-- 1 root root  30 ... lock
```

Representative leftovers from the converged copy:

```
$ ls ~/machine/root/etc/sysctl.d/90-genarch-settings.conf \
      ~/machine/root/etc/systemd/system/genarch-settings.service \
      ~/machine/root/var/lib/genarch-settings/manifest.toml
...
$ diff -u ~/machine/root/usr/lib/systemd/system/power-profiles-daemon.service.bak-20261114 \
          ~/machine/root/usr/lib/systemd/system/power-profiles-daemon.service | tail -5
@@ -1,3 +1,3 @@
 [Unit]
 Description=Power Profiles daemon
-Conflicts=tuned.service
+Conflicts=tuned.service auto-cpufreq.service system76-power.service
```

`genarch-settings --help` lists no restore/unapply verb.

## What could not be determined

- I did not run a successful `apply` against the real `/etc` (it is not scratch space), so the leftover set is demonstrated on the rehearsal tree plus the package file list; the manifest is the engine's own record of what would be written.
- Whether `pre_remove`'s `systemctl disable` is enough to leave a clean boot after the unit files remain (they are disabled but still on disk).
