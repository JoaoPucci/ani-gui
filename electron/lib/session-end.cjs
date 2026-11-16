"use strict";

// Quitting when the desktop announces that the session is ending, on
// Linux — before the stop signals arrive.
//
// Without this the app learns of a reboot, a power-off or a logout
// only from the SIGTERM systemd sends every process in its scope at
// once, which Chromium does not survive cleanly (see
// lib/termination-signals.cjs). Chrome answers logind's announcement
// of a reboot or power-off itself
// (chrome/browser/lifetime/session_end_listener_linux.cc); Electron
// does not build that code (electron/electron#55560), so the app
// listens for it, and for GNOME's announcement of a logout too:
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

const { StringDecoder } = require("node:string_decoder");

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
 * does not outlive the app (it would keep the app's systemd scope
 * alive). A stop signal that reaches the shell (Ctrl-C, a closed
 * terminal, a stop of the app's group) ends the gdbus too: the shell
 * started it as a background job, which ignores SIGINT and SIGQUIT,
 * so it would otherwise outlive the shell. A SIGKILL to the shell
 * alone, as a kill by command line matching "ani-gui" sends it, runs no
 * trap, and gdbus reads /dev/null: setpriv asks the kernel to send
 * gdbus SIGTERM when the shell dies once gdbus has started, a SIGKILL
 * included. On a system without setpriv's --pdeathsig that one case
 * still leaves gdbus behind. Without gdbus the watcher exits at once.
 */
const SESSION_WATCHER = Object.freeze({
  cmd: "/bin/sh",
  args: [
    "-c",
    [
      "command -v gdbus >/dev/null 2>&1 || exit 0",
      // setpriv (util-linux) has gdbus sent SIGTERM when this shell
      // dies, a SIGKILL included, which no trap sees.
      "run=",
      "if setpriv --pdeathsig TERM -- true >/dev/null 2>&1; then run='setpriv --pdeathsig TERM --'; fi",
      // $! is gdbus, the only background job, from the moment it is
      // started; empty before.
      `trap '[ -n "$!" ] && kill "$!" 2>/dev/null; exit 0' INT TERM HUP QUIT`,
      '$run gdbus monitor --session --dest "$1" --object-path "$2" &',
      "read -r _",
      'kill "$!" 2>/dev/null',
    ].join("\n"),
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
  // A Buffer may end inside a multi-byte character: the decoder keeps
  // its first bytes until the rest arrive.
  const decoder = new StringDecoder("utf8");
  let pending = "";
  return (chunk) => {
    pending += typeof chunk === "string" ? chunk : decoder.write(chunk);
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

  // Electron creates its native monitor on powerMonitor's first
  // listener and, on Linux, reports whether a `shutdown` listener
  // exists before that first listener is in place — so a `shutdown`
  // listener added first is never reported, and no inhibitor is taken
  // (lib/browser/api/power-monitor.ts, 33 and later). One added after
  // any other listener is.
  powerMonitor.on("resume", () => {});
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
