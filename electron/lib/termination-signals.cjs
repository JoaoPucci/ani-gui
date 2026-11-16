"use strict";

// How the app answers SIGTERM, SIGINT and SIGHUP on Linux.
//
// A stop signal often reaches every process of the app at once: a
// logout or shutdown stops the systemd scope the desktop launched the
// app in, which holds all of its processes (Electron does not move its
// main process into a scope of its own), `pkill ani-gui` matches by name, a closed terminal or
// `kill -- -<pgid>` signals the process group. Chromium's zygote, GPU
// and utility helpers then die while the main process is still
// quitting. Chromium relaunches the GPU process through the dead
// zygote, fails every attempt, and aborts with "GPU process isn't
// usable. Goodbye." — a SIGTRAP, which the desktop's crash reporter
// announces as a crash (electron/electron#54547). Chromium's own handler is also gone after
// the first signal, so a second one kills the app outright.
//
// So, once the app is ready, the main process takes the three signals
// over (the approach of TheOrcDev/videorc#1198):
//
//   - A signal that reached the main process alone asks for the
//     ordinary quit, every time it arrives — a second signal included.
//   - A signal that reached the helpers too ends the app at once, by
//     that same signal, before the relaunch that would abort it. First
//     `beforeEnd` runs: it stops the backend and makes the app's own
//     storage reach disk (see `persistStorage`). Then every listener
//     for the three signals is removed, so the signal's default action
//     applies, and the signal is raised again. Like the abort, this
//     ends the app without removing its profile's lock; the next
//     launch takes over a lock whose owner is gone.
//
// "Reached the helpers" means the GPU process is gone or a zombie when
// the signal arrives, or within HELPER_WATCH_WINDOW_MS of it (checked
// every HELPER_WATCH_INTERVAL_MS, and on `child-process-gone`); a
// signal that arrives after an earlier one's window opens a window of
// its own. The GPU process is the one whose death makes Chromium
// abort, and in the measurements an ordinary quit never had it exit
// inside the window. Renderers and utility processes are not counted:
// renderers exit with their windows, and utility processes exit on
// their own, cleanly, at any time.
//
// This is a race the takeover usually wins, not a guarantee: the
// watch and Chromium's relaunch of the GPU process both run on the
// main thread, and the relaunch follows the helpers' death within
// about 15 ms. The before-and-after measurements are in the pull
// request that brought this module.
//
// Nothing is installed before `ready`. Electron puts its own handler in
// place after the main script has run, and a listener added earlier
// would leave Node's watcher for the signal under it (Node creates the
// watcher on a signal's first listener only).
//
// Everything is injected, so the policy runs without Electron; main.js
// wires it to `process`, `app.getAppMetrics()` and /proc.

const TERMINATION_SIGNALS = Object.freeze(["SIGTERM", "SIGINT", "SIGHUP"]);

/** How often a signalled quit looks at the helpers again. The relaunch
 *  that aborts Chromium follows the helpers' death within about 15 ms. */
const HELPER_WATCH_INTERVAL_MS = 5;

/** How long after a signal a helper's exit still counts as that
 *  signal's doing. An ordinary quit's only helpers to exit this early
 *  are renderers, which are not counted. */
const HELPER_WATCH_WINDOW_MS = 250;

/**
 * The state letter of a /proc/<pid>/stat line, or null when it cannot
 * be read. The command name may itself contain ')', so the last one
 * ends it.
 */
function procStatState(stat) {
  if (typeof stat !== "string") return null;
  const close = stat.lastIndexOf(")");
  if (close < 0) return null;
  const state = stat.slice(close + 2, close + 3);
  return /^[A-Za-z]$/.test(state) ? state : null;
}

/**
 * Names the GPU process once it has exited, or null while it runs (or
 * there is none). `helpers` is `app.getAppMetrics()`'s shape
 * ({ pid, type }). A process not yet reaped is a zombie ("Z"), not
 * gone.
 */
function exitedGpuProcess(helpers, mainPid, readProcStat) {
  for (const helper of helpers) {
    if (helper.pid === mainPid || helper.type !== "GPU") continue;
    const state = procStatState(readProcStat(helper.pid));
    if (state === null) return `${helper.type} process ${helper.pid} is gone`;
    if (state === "Z" || state === "X" || state === "x") {
      return `${helper.type} process ${helper.pid} exited`;
    }
  }
  return null;
}

/**
 * Take the three signals over. Returns `{ childProcessGone(details) }`,
 * to be fed every `child-process-gone` event.
 *
 * Options:
 *   platform      process.platform: off Linux nothing is taken over
 *   source        `process`: on(signal, fn), removeAllListeners(signal)
 *   mainPid       this process's pid
 *   listHelpers   () => [{ pid, type }]  (app.getAppMetrics())
 *   readProcStat  (pid) => text of /proc/<pid>/stat, or null when gone
 *   requestQuit   (signal) => void: the ordinary quit
 *   beforeEnd     (signal, reason) => void: runs once, before the raise
 *   raise         (signal) => void: sends `signal` to this process
 *   setInterval, clearInterval, now: the clock
 */
function installTerminationSignals(options) {
  if (options.platform !== "linux") return { childProcessGone() {} };
  let quitSignal = null;
  let quitSignalAt = 0;
  let watch = null;
  let ended = false;

  const stopWatching = () => {
    if (watch !== null) {
      options.clearInterval(watch);
      watch = null;
    }
  };
  const watching = () =>
    watch !== null && options.now() - quitSignalAt <= HELPER_WATCH_WINDOW_MS;

  const helperGone = () => {
    try {
      return exitedGpuProcess(options.listHelpers(), options.mainPid, options.readProcStat);
    } catch {
      return null;
    }
  };

  const end = (signal, reason) => {
    if (ended) return;
    ended = true;
    stopWatching();
    try {
      options.beforeEnd(signal, reason);
    } finally {
      // Node restores a signal's default action only once it has no
      // listener left, so every listener goes, not only this module's.
      for (const each of TERMINATION_SIGNALS) options.source.removeAllListeners(each);
      options.raise(signal);
    }
  };

  const onSignal = (signal) => {
    if (ended) return;
    const gone = helperGone();
    if (gone) {
      end(signal, gone);
      return;
    }
    if (!watching()) {
      stopWatching();
      quitSignal = signal;
      quitSignalAt = options.now();
      watch = options.setInterval(() => {
        if (!watching()) {
          stopWatching();
          return;
        }
        const helper = helperGone();
        if (helper) end(quitSignal, helper);
      }, HELPER_WATCH_INTERVAL_MS);
    }
    options.requestQuit(signal);
  };

  for (const signal of TERMINATION_SIGNALS) options.source.on(signal, () => onSignal(signal));

  return {
    childProcessGone(details) {
      if (
        quitSignal !== null &&
        !ended &&
        watching() &&
        details.type === "GPU"
      ) {
        end(quitSignal, `${details.type} process ${details.reason}`);
      }
    },
  };
}

/**
 * How long an ending by signal holds the main process after asking
 * Chromium to write its storage out. The write takes a few
 * milliseconds; this is the margin. Holding the main thread also
 * holds back the GPU relaunch that would abort the app, which runs on
 * that same thread.
 */
const STORAGE_COMMIT_WAIT_MS = 100;

/**
 * Make what the renderer saved in localStorage — resume positions,
 * recent searches, caption and audio choices — reach disk before the
 * app ends by a signal. Chromium commits localStorage to disk in
 * batches, some time after each write, and on an ordinary quit; a
 * process killed by a signal does neither. `flush` asks for the commit
 * now (session.flushStorageData()), which runs off the main thread
 * without a completion callback; `block` then holds the main thread
 * for `waitMs`.
 */
function persistStorage({ flush, block, waitMs = STORAGE_COMMIT_WAIT_MS }) {
  try {
    flush();
  } finally {
    block(waitMs);
  }
}

/** Hold the calling thread for `ms` milliseconds without spinning. */
function blockFor(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

module.exports = {
  HELPER_WATCH_INTERVAL_MS,
  HELPER_WATCH_WINDOW_MS,
  STORAGE_COMMIT_WAIT_MS,
  TERMINATION_SIGNALS,
  blockFor,
  exitedGpuProcess,
  installTerminationSignals,
  persistStorage,
  procStatState,
};
