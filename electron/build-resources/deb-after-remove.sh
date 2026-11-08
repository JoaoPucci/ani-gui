#!/bin/sh
# .deb postrm hook — runs as root after dpkg removes /opt/ani-gui/.
#
# Every path is prefixed with $DPKG_ROOT, which dpkg sets to the root
# it removes from: empty for a normal removal, a directory for
# `dpkg --root`.

set -e

# Cleans up the /usr/local/bin/ani-gui symlink that the postinst
# created. Only removes if the link still points at our launcher,
# so a user who replaced it manually doesn't lose their override.
LINK="$DPKG_ROOT/usr/local/bin/ani-gui"
TARGET=/opt/ani-gui/ani-gui
if [ -L "$LINK" ]; then
    if [ "$(readlink "$LINK")" = "$TARGET" ]; then
        rm -f "$LINK"
    fi
fi

# The AppArmor profile the postinst wrote: unloaded and deleted, with
# the compiled copy `apparmor_parser -W` cached, when the package goes
# away. Not on `upgrade`, where the new package's postinst rewrites
# and reloads it straight after, and unloading would only leave a
# window without it.
case "$1" in
    remove | purge)
        PROFILE="$DPKG_ROOT/etc/apparmor.d/ani-gui"
        if [ -e "$PROFILE" ]; then
            if aa-enabled --quiet 2>/dev/null; then
                apparmor_parser -R "$PROFILE" || true
            fi
            rm -f "$PROFILE"
        fi
        rm -f "$DPKG_ROOT"/var/cache/apparmor/*/ani-gui
        ;;
esac

exit 0
