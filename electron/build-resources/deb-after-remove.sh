#!/bin/sh
# .deb postrm hook — runs as root after dpkg removes /opt/ani-gui/.
#
# Every file path is prefixed with $DPKG_ROOT. dpkg leaves it empty for
# a normal removal, and for `dpkg --root`, which runs this script in a
# chroot; only `--force-script-chrootless` sets it to the target root.
# Unloading the AppArmor profile acts on the running kernel either way.

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
# away (`disappear` is a package whose every file another package has
# replaced). Not on `upgrade`, where the new package's postinst
# rewrites and reloads it straight after, and unloading would only
# leave a window without it. A downgrade to a version older than the
# profile runs this script with `upgrade` and keeps it, and that
# version's own postrm will not remove it: the file then stays, granting
# user namespaces to a path nothing occupies once the package is gone,
# until the next install of this one rewrites and removes it.
case "$1" in
    remove | purge | disappear)
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

# On purge, what an administrator added for the profile goes too, as
# dh_apparmor's postrm does.
if [ "$1" = purge ]; then
    for dir in local disable force-complain; do
        rm -f "$DPKG_ROOT/etc/apparmor.d/$dir/ani-gui"
    done
fi

exit 0
