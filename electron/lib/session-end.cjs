"use strict";

// Quitting when the desktop announces that the session is ending, on
// Linux — before the stop signals arrive.
//
// Without this the app learns of a reboot, a power-off or a logout
// only from the SIGTERM systemd sends every process in its scope at
// once, which Chromium does not survive cleanly (see
// lib/termination-signals.cjs). Chrome answers the announcements
// themselves (chrome/browser/lifetime/session_end_listener_linux.cc);
// Electron does not build that code (electron/electron#55560), so the
// app listens for the same two:
//
//   - Reboot and power-off: logind's PrepareForShutdown(true), which
//     Electron passes on as powerMonitor's `shutdown` event. While a
//     listener exists Electron holds a logind delay inhibitor; calling
//     preventDefault() keeps it held until this process exits, so the
//     shutdown waits for the quit (up to logind's InhibitDelayMaxSec).
//   - Logout on GNOME: gnome-session's SessionOver signal on the
//     session bus. Nothing in Electron reaches the session bus, so
//     `gdbus monitor` — from the GLib tools the app already runs at
//     launch (lib/reduced-motion.cjs) — prints it. A logout cannot be
//     delayed this way: the quit races the stop of the app's scope,
//     and a quit the stop overtakes ends by the signal instead
//     (lib/termination-signals.cjs).
//
// A desktop that announces a logout some other way (KDE, the Inhibit
// portal's session monitor, which needs a D-Bus connection of the
// app's own) gets no early quit; its logout reaches the app as signals.

/** gnome-session's object, and the line `gdbus monitor` prints for its
 *  SessionOver signal. */
const GNOME_SESSION = Object.freeze({
  service: "org.gnome.SessionManager",
  path: "/org/gnome/SessionManager",
});
const SESSION_OVER_LINE = `${GNOME_SESSION.path}: ${GNOME_SESSION.service}.SessionOver (`;

/**
 * The watcher: `gdbus monitor` under a shell that ends it as soon as
 * its stdin — the main process's end of a pipe — closes. The pipe
 * closes however the app ends, a SIGKILL included, so the watcher
 * never outlives the app (it would keep the app's systemd scope
 * alive). Without gdbus it exits at once.
 */
const SESSION_WATCHER = Object.freeze({
  cmd: "/bin/sh",
  args: [
    "-c",
    'command -v gdbus >/dev/null 2>&1 || exit 0; gdbus monitor --session --dest "$1" --object-path "$2" & watcher=$!; read -r _; kill "$watcher" 2>/dev/null',
    "ani-gui-session-watch",
    GNOME_SESSION.service,
    GNOME_SESSION.path,
  ],
});

/** Whether a line of `gdbus monitor` output is gnome-session's SessionOver. */
function isSessionOver(line) {
  return typeof line === "string" && line.trim().startsWith(SESSION_OVER_LINE);
}

/**
 * Split a stream's chunks into whole lines, calling `onLine` for each;
 * a partial line waits for the rest.
 */
function lineSplitter(onLine) {
  let pending = "";
  return (chunk) => {
    pending += String(chunk);
    const lines = pending.split("\n");
    pending = lines.pop();
    for (const line of lines) onLine(line);
  };
}

/**
 * Listen for the session's end. `onEnding(reason)` runs once, with
 * "shutdown" or "logout". Returns `{ stop() }`, which ends the watcher
 * (call it as the app quits). Off Linux nothing is listened for.
 *
 *   platform      process.platform
 *   powerMonitor  Electron's (only touched on Linux, after ready)
 *   spawn         child_process.spawn
 *   env           the environment the watcher runs in
 *   onEnding      (reason) => void
 *   logError      (...args) => void
 */
function watchSessionEnd({ platform, powerMonitor, spawn, env, onEnding, logError }) {
  if (platform !== "linux") return { stop() {} };
  let ended = false;
  const ending = (reason) => {
    if (ended) return;
    ended = true;
    onEnding(reason);
  };

  powerMonitor.on("shutdown", (event) => {
    // Keeps Electron's delay inhibitor held until this process exits.
    event.preventDefault();
    ending("shutdown");
  });

  let watcher = null;
  try {
    watcher = spawn(SESSION_WATCHER.cmd, SESSION_WATCHER.args, {
      env,
      stdio: ["pipe", "pipe", "ignore"],
    });
    watcher.on("error", (err) => logError("[main] session watcher:", err));
    // The pipe to a watcher that has exited: nothing to report.
    watcher.stdin.on("error", () => {});
    watcher.stdout.on(
      "data",
      lineSplitter((line) => {
        if (isSessionOver(line)) ending("logout");
      }),
    );
  } catch (err) {
    logError("[main] session watcher:", err);
    watcher = null;
  }

  return {
    stop() {
      if (watcher === null) return;
      try {
        watcher.stdin.end();
      } catch {
        // Already closed.
      }
      watcher = null;
    },
  };
}

module.exports = {
  GNOME_SESSION,
  SESSION_WATCHER,
  isSessionOver,
  lineSplitter,
  watchSessionEnd,
};
