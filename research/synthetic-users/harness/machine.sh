#!/usr/bin/env bash
# machine.sh — a synthetic machine for a rehearsal root.
#
# The VM is not the laptop the package ships a profile for, and it has no SMU or
# cpufreq policies. This builds a *complete fake live tree* under ~/machine/root
# and a matching operator profile, so `GENARCH_SETTINGS_ROOT=~/machine/root
# genarch-settings ...` exercises the real engine: layers, plan, apply, pre-images,
# re-plan, prune and the sysctl/ceil/mirror/managed writers. Nothing here touches
# the real VM.
#
# Usage: ./machine.sh [DEST]     (default ~/machine)
set -euo pipefail

DEST=${1:-$HOME/machine}
BARE=${2:-}
ROOT="$DEST/root"
PROFILE=vm-generic
# The fake machine carries THIS VM's DMI, or the profile cannot match it.
PRODUCT=$(cat /sys/class/dmi/id/product_name)
VENDOR=$(cat /sys/class/dmi/id/sys_vendor)

rm -rf "$DEST"
mkdir -p "$ROOT"

# The package's base layer, byte for byte.
mkdir -p "$ROOT/usr/lib"
cp -a /usr/lib/genarch-settings "$ROOT/usr/lib/genarch-settings"

# --- the fake machine's identity -------------------------------------------
mkdir -p "$ROOT/sys/class/dmi/id"
printf '%s\n' "$PRODUCT" > "$ROOT/sys/class/dmi/id/product_name"
printf '%s\n' "$VENDOR"  > "$ROOT/sys/class/dmi/id/sys_vendor"
printf '%s\n' "QEMU Virtual Machine" > "$ROOT/sys/class/dmi/id/product_version"
printf '%s\n' "1.0" > "$ROOT/sys/class/dmi/id/bios_version"

# Two cpufreq policies, one core each. scaling_max_freq starts above the
# profile's declared ceiling so the plan has something to do.
for p in 0 1; do
    d="$ROOT/sys/devices/system/cpu/cpufreq/policy$p"
    mkdir -p "$d"
    printf '%s\n' "$p"    > "$d/affected_cpus"
    printf '%s\n' "$p"    > "$d/related_cpus"
    printf '4000000\n'    > "$d/scaling_max_freq"
    printf '400000\n'     > "$d/scaling_min_freq"
    printf '3500000\n'    > "$d/cpuinfo_max_freq"
    printf '400000\n'     > "$d/cpuinfo_min_freq"
    printf 'powersave\n'  > "$d/scaling_governor"
    printf 'amd-pstate-epp\n' > "$d/scaling_driver"
done

# The runtime sysctl surface the base intent asserts, deliberately divergent.
put_sysctl() {
    local f="$ROOT/proc/sys/$1" v="$2"
    mkdir -p "$(dirname "$f")"
    printf '%s\n' "$v" > "$f"
}
put_sysctl fs/file-max 9223372036854775807
put_sysctl kernel/hung_task_panic 0
put_sysctl kernel/hung_task_timeout_secs 120
put_sysctl kernel/kptr_restrict 0
put_sysctl kernel/nmi_watchdog 1
put_sysctl kernel/printk "4 4 1 7"
put_sysctl kernel/sysrq 16
put_sysctl kernel/unprivileged_userns_clone 1
put_sysctl net/core/netdev_max_backlog 1000
put_sysctl vm/dirty_background_bytes 0
put_sysctl vm/dirty_bytes 0
put_sysctl vm/dirty_expire_centisecs 3000
put_sysctl vm/dirty_writeback_centisecs 500
put_sysctl vm/page-cluster 3
put_sysctl vm/swappiness 60
put_sysctl vm/vfs_cache_pressure 100

# --- files the managed records patch ---------------------------------------
mkdir -p "$ROOT/etc/systemd" "$ROOT/usr/lib/systemd/system"
cat > "$ROOT/etc/systemd/journald.conf" <<'EOF'
[Journal]
#SystemMaxUse=
Storage=auto
EOF
cat > "$ROOT/usr/lib/systemd/system/power-profiles-daemon.service" <<'EOF'
[Unit]
Description=Power Profiles daemon
Conflicts=tuned.service
EOF

mkdir -p "$ROOT/var/lib/genarch-settings" "$ROOT/etc/systemd/system" \
         "$ROOT/etc/udev/rules.d" "$ROOT/etc/sysctl.d" "$ROOT/usr/lib/systemd/system-sleep"

# --- the operator layer: a profile for THIS machine ------------------------
OP="$ROOT/etc/genarch-settings"
if [ "$BARE" = "--bare" ]; then
    mkdir -p "$OP"
    cat > "$DEST/README" <<'EOF'
This is a staging box, not the laptop the shipped profile is for.

`root/` is a rehearsal tree: a fake live filesystem with this VM's own DMI
strings, two cpufreq policies and the usual /etc targets a machine has. The
engine's shipped documentation describes `GENARCH_SETTINGS_ROOT` for exactly
this: point every live path at a copy. Nothing under `root/` is the real
machine, so writes there are safe and need no privilege.

There is no operator profile in this tree yet; the only one the package knows
is the retail laptop's.

    GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report
EOF
    echo "synthetic machine (bare) at $DEST"
    exit 0
fi
mkdir -p "$OP/profiles/$PROFILE"
printf 'profile = "%s"\n' "$PROFILE" > "$OP/profile"

cat > "$OP/profiles/$PROFILE/intent.toml" <<EOF
# A profile for the rehearsal machine, not for a retail laptop: the DMI strings
# are QEMU's, and every value below describes this fake box.
status = "measured"

[requires]
product = "$PRODUCT"
sys_vendor = "$VENDOR"
EOF

cat > "$OP/profiles/$PROFILE/ceil.toml" <<'EOF'
[[class]]
name = "vm"
cpus = ["0-1"]
khz = 3000000
EOF

cat > "$OP/profiles/$PROFILE/ryzenadj.toml" <<'EOF'
raw = []

[co]
all = -3

[limits]
power_saving = true
EOF

cat > "$OP/profiles/$PROFILE/passes.toml" <<'EOF'
wake_disarm = []
idle_governor = "teo"
wifi_interface = "wlan0"
settle_secs = 0
EOF

cat > "$DEST/README" <<'EOF'
This is a staging box, not the laptop the shipped profile is for.

`root/` is a rehearsal tree: a fake live filesystem with a QEMU DMI, two
cpufreq policies and a `vm-generic` operator profile that describes it. The
engine's shipped documentation describes `GENARCH_SETTINGS_ROOT` for exactly
this: point every live path at a copy. Nothing under `root/` is the real
machine, so writes there are safe and need no privilege.

    GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings report
    GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply --dry-run
    GENARCH_SETTINGS_ROOT=$HOME/machine/root genarch-settings apply

The SMU tooling in this VM cannot talk to an AMD SMU, and there is no wireless
interface: rows that need that hardware will say SKIPPED.
EOF

echo "synthetic machine at $DEST"
