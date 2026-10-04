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
 * `detached: true` puts the backend in its own process group on POSIX
 * so the quit path can kill the entire group (backend + the transport
 * it spawns per request + yt-dlp + ffmpeg) via `process.kill(-pid, …)`.
 * Without it, only the Rust process gets the signal and the download
 * grandchildren get reparented to init and keep running. Windows has
 * no process groups; the tree kill shells out to taskkill /T instead.
 */
function backendSpawnOptions({ platform, env }) {
  return {
    stdio: ["pipe", "pipe", "pipe"],
    detached: platform !== "win32",
    env: { ...env, ANI_GUI_PARENT_STDIN: "1" },
  };
}

module.exports = { backendSpawnOptions };
