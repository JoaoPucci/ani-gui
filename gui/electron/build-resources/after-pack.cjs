"use strict";

const fs = require("node:fs");
const path = require("node:path");

// electron-builder afterPack hook (Linux only).
//
// We need `ELECTRON_OZONE_PLATFORM_HINT=auto` set BEFORE Electron's early Ozone
// init so the app runs natively on Wayland (and resolves back to X11 on an X11
// session — `=auto` is session-aware). There is no in-process way to do this:
// `app.commandLine.appendSwitch` lands too late, and the old `app.relaunch()`
// into `--ozone-platform-hint=auto` crashed on Wayland (the parent inits on
// XWayland, then the re-exec'd child SIGTRAPs in Ozone init — 100% on
// GNOME/ibus). The only reliable knob is the process environment, set by the
// launcher.
//
// So: rename the packed `ani-gui` ELF to `ani-gui.bin` and drop a tiny `ani-gui`
// shell wrapper in its place that exports the hint and execs the real binary.
// Every launch path goes through this one file — the .deb desktop `Exec`, the
// `/usr/local/bin/ani-gui` symlink, and the AppImage `AppRun` all exec
// `ani-gui` — so one wrapper covers them all. Idempotent and self-locating.
module.exports = async function afterPack(context) {
  if (context.electronPlatformName !== "linux") return;

  const exe = context.packager.executableName || "ani-gui";
  const dir = context.appOutDir;
  const real = path.join(dir, exe);
  const renamed = path.join(dir, `${exe}.bin`);

  if (fs.existsSync(renamed)) return; // already wrapped
  if (!fs.existsSync(real)) {
    throw new Error(`afterPack: expected packed executable not found: ${real}`);
  }

  fs.renameSync(real, renamed);

  const wrapper = `#!/bin/sh
# ani-gui launcher — select native Wayland before Electron's Ozone init.
# \`=auto\` resolves to Wayland under a Wayland session and X11 otherwise, so
# this is a no-op on X11. Setting it here (pre-init) avoids the app.relaunch()
# re-exec that crashed on Wayland. Honors a value the user already exported.
: "\${ELECTRON_OZONE_PLATFORM_HINT:=auto}"
export ELECTRON_OZONE_PLATFORM_HINT
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/${exe}.bin" "$@"
`;

  fs.writeFileSync(real, wrapper, { mode: 0o755 });
};
