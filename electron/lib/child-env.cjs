"use strict";

// The environment a program the app starts gets on Linux: the user's,
// not the one Electron and Chromium leave in this process.
//
// What they change, in Electron 33:
//
//   - before main.js runs (lib/browser/init.ts), ORIGINAL_XDG_CURRENT_DESKTOP
//     is set to the session's XDG_CURRENT_DESKTOP (on every platform, and
//     to the string "undefined" when there is none), and
//     XDG_CURRENT_DESKTOP is rewritten to `Unity`, for old tray icon
//     support, when it matches /ubuntu/i or is one of Pantheon,
//     Unity:Unity7 and pop:GNOME; and CHROME_DESKTOP is set to the app's
//     desktop file (app.setDesktopName);
//   - after main.js's top level, when Chromium starts GTK, GDK_BACKEND is
//     set to the window system the app uses (ui/gtk) and, under X11,
//     NO_AT_BRIDGE=1; GTK takes DESKTOP_STARTUP_ID out, and Chromium's
//     Wayland platform XDG_ACTIVATION_TOKEN.
//
// A child inherits all of it. A browser started by a link then believes
// it runs on Unity: GSettings applies its Unity overrides, which Ubuntu
// still ships, and the browser draws its window buttons on the left.
// Electron's own correction before xdg-open (platform_util_linux.cc)
// looks for the kept value among the launch's overrides rather than in
// the environment, and never finds it. Electron 38 stops rewriting the
// desktop and 43 stops leaving GDK_BACKEND behind; CHROME_DESKTOP stays.
//
// So the user's environment is rebuilt from a snapshot main.js takes
// before it loads Electron's API — after Electron's early rewrite, which
// the kept value undoes, and before GTK's start-up — whatever a later
// version does in between: a variable added after the snapshot is
// dropped, one changed since is put back, and one taken out since stays
// out. The app's own ANI_GUI_* variables, which main.js exports on
// purpose, are passed on as they are now. The app's own launch's startup
// id and activation token are never passed on: they were good once, for
// the app's window. The app's own process keeps Electron's environment;
// a child gets the rebuilt one.
//
// The AppImage changes more, before main.js runs. Its runtime
// (type2-runtime, which the repack puts in place of
// electron-builder's appimage-12.0.1, scripts/repack-appimage.mjs;
// both do the same here) exports APPIMAGE, APPDIR, OWD and ARGV0, and
// in portable mode (a `<image>.home` or `<image>.config` directory
// beside the image) points HOME and XDG_CONFIG_HOME at those. The
// AppRun the image ships puts the mount ahead of PATH (the mount and
// its usr/sbin), XDG_DATA_DIRS (its usr/share/, with
// /usr/share/gnome, /usr/local/share/ and /usr/share/ appended after
// the user's), LD_LIBRARY_PATH (its usr/lib) and GSETTINGS_SCHEMA_DIR
// (its usr/share/glib-2.0/schemas). The snapshot reads all of that as
// the user's. A browser started with it would load the image's old
// libraries (libnotify, libappindicator, ...) and search its data
// first, and an Electron app started with it would read $APPIMAGE as
// its own. So when this process runs from the mount, a child gets
// those lists without what AppRun added, and a list left empty is not
// passed on at all (AppRun cannot tell a list set empty from one
// never set, and neither can this); the runtime's variables are
// dropped. A portable HOME is the account's again, and a portable
// XDG_CONFIG_HOME is dropped, since the one it replaced is not known;
// the app's own backend keeps both (`keepPortable`), because that is
// where the person chose to keep the app's data, and the external
// player and Syncplay it starts inherit them.
// A process that does not run from the mount (another AppImage leaked
// its variables into the terminal the app was started from) passes
// them on as the user launched with them.
//
// Windows and macOS are untouched: a child gets the app's environment
// itself, as it always has.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const APP_OWNED = "ANI_GUI_";
const KEPT_DESKTOP = "ORIGINAL_XDG_CURRENT_DESKTOP";
const DESKTOP = "XDG_CURRENT_DESKTOP";
const NEVER_PASSED_ON = [
  KEPT_DESKTOP,
  // The app's desktop file, which Chromium-based browsers read as their own.
  "CHROME_DESKTOP",
  // The app's own launch's, good once.
  "DESKTOP_STARTUP_ID",
  "XDG_ACTIVATION_TOKEN",
];

/** What the AppImage's runtime exports for the program it runs. */
const APPIMAGE_RUNTIME = ["APPIMAGE", "APPDIR", "OWD", "ARGV0"];
/** The lists AppRun puts the mount ahead of. */
const APPRUN_LISTS = ["PATH", "XDG_DATA_DIRS", "LD_LIBRARY_PATH", "GSETTINGS_SCHEMA_DIR"];
/** What AppRun appends to XDG_DATA_DIRS, after its own and the user's. */
const APPRUN_DATA_TAIL = ["/usr/share/gnome", "/usr/local/share/", "/usr/share/"];

const has = (env, key) => Object.prototype.hasOwnProperty.call(env, key);

/** Set `key` as an own property, `__proto__` included. */
function put(env, key, value) {
  Object.defineProperty(env, key, { value, enumerable: true, writable: true, configurable: true });
}

/** `p` resolved through symlinks, or as it is when it cannot be. */
function resolved(p, realpath) {
  try {
    return realpath(p);
  } catch {
    return p;
  }
}

/** The AppImage's mount ($APPDIR) when `execPath` runs from it, or null.
 *  Paths are compared resolved: the runtime sets $APPDIR as it mounted
 *  it, and the binary's path is resolved, so a symlinked /tmp tells them
 *  apart. */
function ownMount({ env, execPath, realpath = fs.realpathSync }) {
  const dir = env.APPDIR;
  if (!env.APPIMAGE || !dir || !execPath) return null;
  const inside = path.posix.relative(resolved(dir, realpath), resolved(execPath, realpath));
  if (!inside || inside.startsWith("..") || path.posix.isAbsolute(inside)) return null;
  return dir;
}

/** The account's home directory, or null when it has none on record
 *  (os.userInfo throws for a uid with no passwd entry). */
function homeOf(accountHome) {
  try {
    return accountHome();
  } catch {
    return null;
  }
}

/** Take out of `env` what the AppImage's runtime and AppRun added for
 *  `mount`, the image file `image` being where its portable directories
 *  sit. Unless `keepPortable`, a portable home becomes the account's
 *  (`accountHome`) and a portable config is dropped; when the account's
 *  home is not known the portable home stays, and its config with it. */
function withoutImage(env, { mount, image, accountHome, keepPortable }) {
  if (!keepPortable) {
    if (env.HOME === `${image}.home`) {
      const home = homeOf(accountHome);
      if (home !== null) put(env, "HOME", home);
    }
    // A config is dropped unless the portable home it goes with stays.
    if (env.XDG_CONFIG_HOME === `${image}.config` && env.HOME !== `${image}.home`) delete env.XDG_CONFIG_HOME;
  }
  const rooted = (p) => p === mount || p.startsWith(`${mount}/`);
  for (const name of APPRUN_LISTS) {
    if (!has(env, name)) continue;
    const all = env[name].split(":");
    let parts = all.filter((p) => !rooted(p));
    const tail = parts.slice(-APPRUN_DATA_TAIL.length);
    if (
      name === "XDG_DATA_DIRS" &&
      rooted(all[0]) &&
      tail.length === APPRUN_DATA_TAIL.length &&
      tail.every((p, i) => p === APPRUN_DATA_TAIL[i])
    ) {
      parts = parts.slice(0, -APPRUN_DATA_TAIL.length);
    }
    if (parts.length === 0) delete env[name];
    else put(env, name, parts.join(":"));
  }
  for (const name of APPIMAGE_RUNTIME) delete env[name];
}

/** A frozen copy of `env` as it is now: taken by main.js before it loads Electron's API. */
function launchSnapshot(env) {
  const copy = {};
  for (const [key, value] of Object.entries(env)) put(copy, key, value);
  return Object.freeze(copy);
}

/**
 * The environment a child gets on `platform`, from the snapshot `launch`
 * and the app's environment `now`, for the app running as `execPath`
 * (which says whether it runs from an AppImage's mount). Off Linux,
 * `now` itself. Changes neither.
 */
function childEnv({
  platform,
  launch,
  now,
  execPath,
  realpath,
  accountHome = () => os.userInfo().homedir,
  keepPortable = false,
}) {
  if (platform !== "linux") return now;
  const env = {};
  for (const [key, value] of Object.entries(launch)) {
    if (!key.startsWith(APP_OWNED) && has(now, key)) put(env, key, value);
  }
  for (const [key, value] of Object.entries(now)) {
    if (key.startsWith(APP_OWNED)) put(env, key, value);
  }
  // A desktop Electron did not rewrite is its own kept value; one the
  // session never had stays unset, whatever was kept for it.
  if (has(launch, KEPT_DESKTOP) && has(env, DESKTOP)) put(env, DESKTOP, launch[KEPT_DESKTOP]);
  for (const key of NEVER_PASSED_ON) delete env[key];
  const mount = ownMount({ env: launch, execPath, realpath });
  if (mount) withoutImage(env, { mount, image: launch.APPIMAGE, accountHome, keepPortable });
  return env;
}

/** Make `target` hold exactly `next`'s variables. */
function replaceAll(target, next) {
  for (const key of Object.keys(target)) {
    if (!has(next, key)) delete target[key];
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
 * the main thread can run and would see the child's environment. The
 * swap writes the process environment (setenv, unsetenv) while
 * Chromium's other threads may read it, which C does not make safe; it
 * touches only the variables that differ, the same trade VS Code makes
 * around shell.openExternal.
 *
 * Off Linux `open` runs and `target` is never touched.
 */
function withUserEnv({ platform, target, launch, execPath, open }) {
  if (platform !== "linux") return open();
  const saved = { ...target };
  try {
    replaceAll(target, childEnv({ platform, launch, now: saved, execPath }));
    return open();
  } finally {
    replaceAll(target, saved);
  }
}

module.exports = { childEnv, launchSnapshot, ownMount, withUserEnv };
