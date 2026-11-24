"use strict";

// Restart, from the splash's stuck state (lib/splash-stuck.cjs): start
// the whole app again, launched the way this one was, once this
// process has exited.
//
// The new process must not start while this one still holds the
// single-instance lock (lib/single-instance.cjs): it would hand over to
// the instance that is exiting, which answers nothing while it quits,
// and quit itself — no app at all. So the relaunch waits for this
// process to be gone, on every platform:
//
// - Electron's own relaunch (`app.relaunch()`), everywhere but the
//   AppImage: the .deb, the Windows installer's install, and `pnpm dev`.
//   Electron starts a helper that waits for this process to exit (on
//   Linux it is told when its parent dies, on Windows it waits on the
//   parent's handle) and then runs this binary with this process's
//   arguments, in the environment it was asked in (below). The binary is
//   on disk for good, and the .deb's launcher environment and switches
//   carry over as they are: the new process starts on the same display
//   platform as this one. (The relaunch build-resources/after-pack.cjs
//   warns about switched platforms, XWayland to Wayland, inside one
//   launch; this one switches nothing. The end-to-end suite restarts the
//   unpacked tree this way: on X11 in CI, and on Wayland when run
//   locally under tools/headless-wayland.sh.) Under `pnpm dev` the new
//   Electron is no longer attached to the dev script's terminal, and the
//   Vite server it loads from has to be still running. Started from a
//   terminal through the .deb's launcher (lib/linux-launcher.cjs), the
//   new process is not that launcher's child: the launcher exits shortly
//   after this one, and the terminal's stops no longer reach the new
//   process.
// - The AppImage. Its binary sits on a FUSE mount that goes away the
//   moment this process exits, so neither it nor Electron's helper
//   (which runs from the same mount) can be what starts the new one.
//   A detached `/bin/sh`, outside the mount, waits for this process's
//   id to be gone and then runs the AppImage file itself ($APPIMAGE),
//   with the person's own arguments: the switch the launcher adds to
//   every launch (LAUNCHER_SWITCHES) is dropped, because the new
//   AppImage's launcher adds it again. A --no-sandbox is kept, whether
//   AppRun added it (the sandbox probe got no answer) or the person
//   passed it: the new AppRun reads it as the person's own and starts
//   without the sandbox, as this launch did, without probing again
//   (lib/appimage-tree.cjs). The waiter gets the user's environment,
//   which has none of what this image's runtime and AppRun added
//   ($APPIMAGE, $APPDIR, the mount's entries in PATH, LD_LIBRARY_PATH,
//   XDG_DATA_DIRS and GSETTINGS_SCHEMA_DIR; lib/child-env.cjs): the new
//   image's runtime and AppRun add their own.
//
// Both start the new instance from the user's environment, not the one
// Electron has rewritten in this process (lib/child-env.cjs): the
// AppImage's waiter gets the rebuilt one (`restart`, below), and
// Electron's relaunch, which starts its relauncher from this process's
// environment at the call, is asked for inside `asUser`. Started from
// Electron's, the new instance's Electron would keep Unity as the
// session's desktop and hand it to everything it opens.
//
// The AppImage is recognised by $APPIMAGE and $APPDIR together with
// this binary living under $APPDIR: another AppImage (an editor run as
// one, say) leaks both into the terminals it opens.

const fs = require("node:fs");
const { childEnv, ownMount } = require("./child-env.cjs");

/** What the packaged Linux launchers put before the person's
 *  arguments on every launch: the wrapper script both packages run
 *  (lib/linux-launcher.cjs, written by build-resources/after-pack.cjs).
 *  The AppImage's AppRun's --no-sandbox is not one of them; it depends
 *  on the launch (lib/appimage-tree.cjs). */
const LAUNCHER_SWITCHES = ["--enable-wayland-ime"];

/** The waiter: `$1` is the process to outlive, the rest the command to
 *  run once it is gone. Nothing is interpolated into the script. */
const WAIT_THEN_EXEC = 'pid=$1; shift; while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done; exec "$@"';

/** The AppImage file this process runs from, or null
 *  (lib/child-env.cjs ownMount). */
function appImageOf({ platform, env, execPath, realpath = fs.realpathSync }) {
  if (platform !== "linux") return null;
  return ownMount({ env, execPath, realpath }) ? env.APPIMAGE : null;
}

/** `argv` without its binary and without the launcher's own leading
 *  switches: what the person passed. */
function userArgs(argv) {
  const args = argv.slice(1);
  let i = 0;
  for (const s of LAUNCHER_SWITCHES) if (args[i] === s) i += 1;
  return args.slice(i);
}

/**
 * How to start the app again, from this process's environment `env`:
 * `{ kind: "relaunch" }` for Electron's own relaunch, or
 * `{ kind: "spawn", command, args, env }` for the AppImage's waiter,
 * its `env` being `env` without the dying mount's $APPDIR. `restart`
 * gives the waiter the user's environment instead.
 */
function relaunchPlan({ platform, env, execPath, argv, pid, realpath }) {
  const image = appImageOf({ platform, env, execPath, realpath });
  if (!image) return { kind: "relaunch" };
  const { APPDIR: _mount, ...rest } = env;
  return {
    kind: "spawn",
    command: "/bin/sh",
    args: ["-c", WAIT_THEN_EXEC, "ani-gui-restart", String(pid), image, ...userArgs(argv)],
    env: rest,
  };
}

/**
 * Arrange `plan`; the caller then ends this process. Electron's relaunch
 * is asked for inside `asUser` (lib/child-env.cjs withUserEnv), which
 * holds the user's environment in place for the call. Throws, before
 * anything is started, when the waiter's shell is missing, so the
 * caller can keep the app up instead of quitting into nothing. The
 * waiter is detached in a session of its own and let go: it outlives
 * this process, and a failure after it started can only be logged.
 */
function relaunch(
  plan,
  { app, spawn, exists = fs.existsSync, logError = console.error, asUser = (open) => open() },
) {
  if (plan.kind === "relaunch") {
    asUser(() => app.relaunch());
    return;
  }
  if (!exists(plan.command)) throw new Error(`cannot restart: ${plan.command} is missing`);
  const child = spawn(plan.command, plan.args, { detached: true, stdio: "ignore", env: plan.env });
  child.on("error", (err) => logError("[main] the restart's waiter failed:", err));
  child.unref();
}

/**
 * Restart, as the splash's stuck state asks for it: planned from this
 * process's environment (`now`), the AppImage's waiter given the user's,
 * rebuilt from the launch's (`launch`) and `now` by lib/child-env.cjs,
 * then arranged by `relaunch` with `deps`.
 */
function restart({ platform, launch, now, execPath, argv, pid }, deps) {
  // Whether this is the AppImage is read off this process's own
  // environment: the user's has no runtime variables in it.
  const plan = relaunchPlan({ platform, env: now, execPath, argv, pid });
  const env = childEnv({ platform, launch, now, execPath });
  relaunch(plan.kind === "spawn" ? { ...plan, env } : plan, deps);
}

module.exports = { LAUNCHER_SWITCHES, WAIT_THEN_EXEC, appImageOf, relaunch, relaunchPlan, restart, userArgs };
