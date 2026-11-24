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
//   (lib/appimage-tree.cjs). An image the runtime ran extracted
//   (--appimage-extract-and-run, for a system without FUSE, which the
//   runtime takes before the app sees its arguments) is restarted with
//   the option again, and its waiter also waits (ten seconds at most)
//   for the old extraction to be removed, since the new launch extracts
//   into the same directory.
//   The waiter gets the user's environment,
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
const path = require("node:path");
const { childEnv, ownMount } = require("./child-env.cjs");

/** What the packaged Linux launchers put before the person's
 *  arguments on every launch: the wrapper script both packages run
 *  (lib/linux-launcher.cjs, written by build-resources/after-pack.cjs).
 *  The AppImage's AppRun's --no-sandbox is not one of them; it depends
 *  on the launch (lib/appimage-tree.cjs). */
const LAUNCHER_SWITCHES = ["--enable-wayland-ime"];

/** Where the AppImage runtime puts the app when it runs it extracted
 *  (type2-runtime: `/tmp/appimage_extracted_<hash>`), and the option
 *  that asks it to. */
const EXTRACTED_PREFIX = "appimage_extracted_";
const EXTRACT_AND_RUN = "--appimage-extract-and-run";

/** The extracted image's waiter: `$1` is the process to outlive, `$2`
 *  the extraction to see removed, `$3` how many tenths of a second to
 *  wait for that at most (a runtime told NO_CLEANUP keeps it), the rest
 *  the command to run. Nothing is interpolated into the script. */
const WAIT_EXTRACTION_THEN_EXEC =
  'pid=$1; dir=$2; ticks=$3; shift 3; while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done; ' +
  'n=0; while [ -e "$dir" ] && [ "$n" -lt "$ticks" ]; do sleep 0.1; n=$((n + 1)); done; exec "$@"';
/** Ten seconds. The removal starts once the runtime's own child has
 *  exited: at once off a terminal, and up to about two seconds after the
 *  app from one (the launcher outlives it, lib/linux-launcher.cjs); the
 *  removal itself takes well under a second. A restart that still meets
 *  it running can start without files the removal takes; a later launch
 *  extracts them again. */
const EXTRACTION_WAIT_TICKS = 100;

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
  const { APPDIR: mount, ...rest } = env;
  // Run extracted rather than mounted (--appimage-extract-and-run or
  // $APPIMAGE_EXTRACT_AND_RUN), the new launch is asked to run so too:
  // the runtime takes the option before the app sees its arguments, and
  // acts on the first argument that starts with `--`. The runtime
  // removes the extraction after this process has exited, and the new
  // launch extracts into the same directory (named by the image's hash),
  // so the waiter also waits, for a while, for that directory to go.
  if (path.posix.basename(mount).startsWith(EXTRACTED_PREFIX)) {
    return {
      kind: "spawn",
      command: "/bin/sh",
      args: [
        "-c",
        WAIT_EXTRACTION_THEN_EXEC,
        "ani-gui-restart",
        String(pid),
        mount,
        String(EXTRACTION_WAIT_TICKS),
        image,
        EXTRACT_AND_RUN,
        ...userArgs(argv),
      ],
      env: rest,
    };
  }
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

module.exports = {
  EXTRACTION_WAIT_TICKS,
  LAUNCHER_SWITCHES,
  WAIT_EXTRACTION_THEN_EXEC,
  WAIT_THEN_EXEC,
  appImageOf,
  relaunch,
  relaunchPlan,
  restart,
  userArgs,
};
