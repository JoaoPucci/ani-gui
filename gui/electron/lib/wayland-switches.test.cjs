"use strict";

const test = require("node:test");
const assert = require("node:assert");

const {
  waylandLaunchArgs,
  waylandRelaunchArgs,
} = require("./wayland-switches.cjs");

test("linux launch args carry the Ozone Wayland hint + IME flag", () => {
  // ozone-platform-hint=auto runs natively on Wayland under a Wayland session
  // (X11 otherwise); enable-wayland-ime routes text input through the Wayland
  // input method. Expressed as real CLI args, not appendSwitch objects.
  assert.deepStrictEqual(waylandLaunchArgs("linux"), [
    "--ozone-platform-hint=auto",
    "--enable-wayland-ime",
  ]);
});

test("non-linux platforms get no launch args (the flags are a no-op there)", () => {
  assert.deepStrictEqual(waylandLaunchArgs("darwin"), []);
  assert.deepStrictEqual(waylandLaunchArgs("win32"), []);
});

test("a fresh launch on a Wayland session must relaunch with the flags appended", () => {
  // ozone-platform-hint is consumed during Electron's early Ozone init, before
  // main.js runs — so appendSwitch is too late and silently ignored. The flag
  // must be in the process's real argv; when it's missing we relaunch once.
  assert.deepStrictEqual(
    waylandRelaunchArgs("linux", ["/opt/ani-gui/ani-gui", "."], {
      WAYLAND_DISPLAY: "wayland-0",
    }),
    ["--ozone-platform-hint=auto", "--enable-wayland-ime"],
  );
});

test("an X11 session (no WAYLAND_DISPLAY) never relaunches — it'd just cost a restart", () => {
  // The hint resolves to X11 under an X11 session anyway, so relaunching there
  // buys nothing but a wasted process restart. Gate on the canonical Wayland
  // signal so X11 users are entirely unaffected.
  assert.deepStrictEqual(
    waylandRelaunchArgs("linux", ["/opt/ani-gui/ani-gui", "."], {}),
    [],
  );
});

test("XDG_SESSION_TYPE=wayland also triggers the relaunch", () => {
  assert.deepStrictEqual(
    waylandRelaunchArgs("linux", ["/opt/ani-gui/ani-gui", "."], {
      XDG_SESSION_TYPE: "wayland",
    }),
    ["--ozone-platform-hint=auto", "--enable-wayland-ime"],
  );
});

test("no relaunch once the hint is already in argv (loop guard)", () => {
  // The re-exec'd process sees its own appended flag and must NOT relaunch
  // again, or it loops forever.
  assert.deepStrictEqual(
    waylandRelaunchArgs(
      "linux",
      ["/opt/ani-gui/ani-gui", ".", "--ozone-platform-hint=auto"],
      { WAYLAND_DISPLAY: "wayland-0" },
    ),
    [],
  );
});

test("no relaunch when the Electron env var already requests the hint", () => {
  // Electron honors ELECTRON_OZONE_PLATFORM_HINT natively; if the user/session
  // exported it, the backend is already on Wayland — don't relaunch.
  assert.deepStrictEqual(
    waylandRelaunchArgs("linux", ["/opt/ani-gui/ani-gui", "."], {
      WAYLAND_DISPLAY: "wayland-0",
      ELECTRON_OZONE_PLATFORM_HINT: "auto",
    }),
    [],
  );
});

test("non-linux never relaunches", () => {
  assert.deepStrictEqual(
    waylandRelaunchArgs("darwin", ["/Applications/ani-gui", "."], {}),
    [],
  );
});
