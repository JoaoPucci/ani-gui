#!/bin/sh
# .deb postinst hook — runs as root after dpkg lays down /opt/ani-gui/.
#
# Every file path is prefixed with $DPKG_ROOT. dpkg leaves it empty for
# a normal install, and for `dpkg --root`, which runs this script in a
# chroot; only `--force-script-chrootless` sets it to the target root.
# Link targets stay unprefixed, because they are read from inside that
# root. Loading the AppArmor profile acts on the running kernel either
# way, as dh_apparmor's own snippet does.

set -e

# Chromium sandboxes its renderers with user namespaces where it can
# start them, and with the setuid helper otherwise; when neither works
# it refuses to start. Two things below cover the two cases.
#
# The setuid helper: Electron's chrome-sandbox has to be owned by root
# with mode 4755 for it to attach. electron-builder's stock packaging
# stopped setting this in newer releases (the kernel trend is toward
# unprivileged user namespaces instead). Kept for every host where
# user namespaces are unavailable — disabled by sysctl, a hardened
# kernel, a container, or an AppArmor profile below that did not load.
# Without it, launching from the menu there fails with
# `setuid_sandbox_host.cc(163) … not configured correctly`.
SANDBOX="$DPKG_ROOT/opt/ani-gui/chrome-sandbox"
if [ -e "$SANDBOX" ]; then
    chown root:root "$SANDBOX"
    chmod 4755 "$SANDBOX"
fi

# The namespace sandbox on Ubuntu 23.10+: AppArmor there grants
# unprivileged user namespaces per program, and without a profile for
# the installed binary Chromium falls through to the setuid helper.
# The profile grants `userns` to /opt/ani-gui/ani-gui.bin (the ELF the
# /opt/ani-gui/ani-gui wrapper starts), and nothing else: it is
# unconfined otherwise, with the rules Ubuntu's own profiles for Chrome,
# VS Code and Obsidian carry. The path is root-owned, so no other
# program can borrow it.
#
# Written only where AppArmor ships ABI 4.0, the first that knows the
# `userns` rule: an older AppArmor (Debian 12's 3.0) could not parse
# it and would fail its boot-time load, and has no restriction to lift.
# Loaded only where AppArmor is active (as dh_apparmor's snippet
# does), and a load that fails never fails the install: the setuid
# helper above still works.
APPARMOR_D="$DPKG_ROOT/etc/apparmor.d"
PROFILE="$APPARMOR_D/ani-gui"
if [ -e "$APPARMOR_D/abi/4.0" ]; then
    cat > "$PROFILE" <<'EOF'
# AppArmor profile for ani-gui, written by its package's postinst and
# removed by its postrm. It lets Chromium's namespace sandbox start
# for the installed binary on hosts that restrict unprivileged user
# namespaces (Ubuntu 23.10 and later).
abi <abi/4.0>,
include <tunables/global>

profile ani-gui /opt/ani-gui/ani-gui.bin flags=(unconfined) {
  userns,
  @{exec_path} mr,

  # Site-specific additions and overrides. See local/README for details.
  include if exists <local/ani-gui>
}
EOF
    chmod 644 "$PROFILE"
    if aa-enabled --quiet 2>/dev/null; then
        apparmor_parser -r -T -W "$PROFILE" || true
    fi
fi

# /usr/local/bin symlink so users can launch from a terminal with
# `ani-gui`. electron-builder's stock packaging puts the launcher at
# /opt/ani-gui/ani-gui without exposing it on PATH; we add the link
# explicitly. /usr/local/bin (rather than /usr/bin) because dpkg-shipped
# binaries live in /usr/bin and shouldn't be touched by user packages.
LAUNCHER=/opt/ani-gui/ani-gui
LINK="$DPKG_ROOT/usr/local/bin/ani-gui"
if [ -x "$DPKG_ROOT$LAUNCHER" ]; then
    mkdir -p "$DPKG_ROOT/usr/local/bin"
    ln -sf "$LAUNCHER" "$LINK"
fi

exit 0
