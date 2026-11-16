"use strict";

// The `ani-gui` launcher the Linux packages run in place of the app's
// binary, which build-resources/after-pack.cjs renames to
// `ani-gui.bin`. Why it exists is told there.

/** The launcher for the packed executable `exe`. */
function launcherScript(exe) {
  return `#!/bin/sh
# ani-gui launcher — select native Wayland before Electron's Ozone init.
# \`=auto\` resolves to Wayland under a Wayland session and X11 otherwise, so
# this is a no-op on X11. Setting it here (pre-init) avoids the app.relaunch()
# re-exec that crashed on Wayland. Honors a value the user already exported.
# \`--enable-wayland-ime\` keeps IME composition (IBus/Fcitx — Japanese/Chinese/
# Korean) working under native Wayland; Chromium does not infer it from the
# ozone hint. It's safe here because every launch, a Restart's relaunch
# included, starts on the platform the hint picks: nothing re-execs into
# another one.
: "\${ELECTRON_OZONE_PLATFORM_HINT:=auto}"
export ELECTRON_OZONE_PLATFORM_HINT
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/${exe}.bin" --enable-wayland-ime "$@"
`;
}

module.exports = { launcherScript };
