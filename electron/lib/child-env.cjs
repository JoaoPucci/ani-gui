"use strict";

// The environment a program the app starts gets on Linux: the user's,
// not the one Electron and Chromium leave in this process.
//
// What they change, in Electron 33:
//
//   - before main.js runs (lib/browser/init.ts), XDG_CURRENT_DESKTOP is
//     rewritten to `Unity` on Ubuntu-style sessions, for old tray icon
//     support, with the session's value kept in
//     ORIGINAL_XDG_CURRENT_DESKTOP; and CHROME_DESKTOP is set to the app's
//     desktop file (app.setDesktopName);
//   - after main.js's top level, when Chromium starts GTK, GDK_BACKEND is
//     set to the window system the app uses (ui/gtk) and, under X11,
//     NO_AT_BRIDGE=1.
//
// A child inherits all of it. A browser started by a link then believes
// it runs on Unity: GSettings applies its Unity overrides, which Ubuntu
// still ships, and the browser draws its window buttons on the left.
// Electron's own correction before xdg-open (platform_util_linux.cc)
// looks for the kept value among the launch's overrides rather than in
// the environment, and never finds it. Electron 38 stops rewriting the
// desktop and 43 stops leaving GDK_BACKEND behind; CHROME_DESKTOP stays.
//
// So the user's environment is rebuilt from a snapshot main.js takes as
// its first statement — after Electron's early rewrite, which the kept
// value undoes, and before GTK's — whatever a later version injects in
// between: a variable added after the snapshot is dropped and one changed
// since is put back, except the app's own ANI_GUI_* variables, which
// main.js exports on purpose. The app's own process keeps Electron's
// environment throughout; only a child gets the rebuilt one.
//
// Windows and macOS are untouched: a child gets the app's environment
// itself, as it always has.

const APP_OWNED = "ANI_GUI_";
const KEPT_DESKTOP = "ORIGINAL_XDG_CURRENT_DESKTOP";
const DESKTOP = "XDG_CURRENT_DESKTOP";
// The app's desktop file, which Chromium-based browsers read as their own.
const APP_DESKTOP_FILE = "CHROME_DESKTOP";

/** A frozen copy of `env` as it is now: main.js's first statement. */
function launchSnapshot(env) {
  return Object.freeze({ ...env });
}

/**
 * The environment a child gets on `platform`, from the snapshot `launch`
 * and the app's environment `now`. Off Linux, `now` itself. Changes
 * neither.
 */
function childEnv({ platform, launch, now }) {
  if (platform !== "linux") return now;
  const env = {};
  for (const [key, value] of Object.entries(launch)) {
    if (!key.startsWith(APP_OWNED)) env[key] = value;
  }
  for (const [key, value] of Object.entries(now)) {
    if (key.startsWith(APP_OWNED)) env[key] = value;
  }
  // Electron keeps the session's value only when it may rewrite it, and
  // copies an unset one as the string "undefined" while leaving the
  // desktop itself unset: restore only a desktop that is there.
  if (KEPT_DESKTOP in launch && DESKTOP in launch) env[DESKTOP] = launch[KEPT_DESKTOP];
  delete env[KEPT_DESKTOP];
  delete env[APP_DESKTOP_FILE];
  return env;
}

/** Make `target` hold exactly `next`'s variables. */
function replaceAll(target, next) {
  for (const key of Object.keys(target)) {
    if (!Object.prototype.hasOwnProperty.call(next, key)) delete target[key];
  }
  for (const [key, value] of Object.entries(next)) {
    if (target[key] !== value) target[key] = value;
  }
}

/**
 * Run `open` — a native call that starts a program from the app's own
 * environment and takes none of its own (shell.openExternal,
 * shell.openPath) — with `target` (process.env) holding the child's
 * environment, and give `target` back exactly as it was the moment
 * `open` returns or throws. Returns what `open` returns.
 *
 * The window is the call itself: on Linux both calls start xdg-open
 * before they return, and only their promise settles later. While a
 * Wayland session hands out the activation token that lets the browser
 * come forward, the call waits in a nested loop, where other work on
 * the main thread can run and would see the child's environment.
 *
 * Off Linux `open` runs and `target` is never touched.
 */
function withUserEnv({ platform, target, launch, open }) {
  if (platform !== "linux") return open();
  const saved = { ...target };
  replaceAll(target, childEnv({ platform, launch, now: saved }));
  try {
    return open();
  } finally {
    replaceAll(target, saved);
  }
}

module.exports = { childEnv, launchSnapshot, withUserEnv };
