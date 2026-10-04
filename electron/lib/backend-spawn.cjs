"use strict";

// How main.js spawns the backend, lifted out so the options can be
// checked without spawning anything.

/**
 * spawn() options for the backend on `platform`, with `env`.
 *
 * stdin is a pipe the main process holds and never writes to or
 * closes, and ANI_GUI_PARENT_STDIN=1 tells the backend to watch it.
 * However the main process ends — a quit, a crash, SIGKILL, taskkill
 * /F — the OS closes its end, the backend reads end of file and shuts
 * itself down (backend/src/parent_watch.rs). Nothing else covers a
 * main process that dies without running its quit path.
 *
 * `detached: true` makes the backend the leader of a process group of
 * its own on POSIX, which gives the quit path a group to signal: the
 * backend and the transports it spawns per request, via
 * `process.kill(-pid, …)`. The download tools are not in that group —
 * the backend runs yt-dlp and ffmpeg in groups of their own and stops
 * them itself when it is asked to stop. Windows has no process
 * groups; the tree kill shells out to taskkill /T instead, which
 * reaches the tools directly.
 */
function backendSpawnOptions({ platform, env }) {
  return {
    stdio: ["pipe", "pipe", "pipe"],
    detached: platform !== "win32",
    env: { ...env, ANI_GUI_PARENT_STDIN: "1" },
  };
}

module.exports = { backendSpawnOptions };
