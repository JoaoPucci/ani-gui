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

/**
 * Spawn the backend at `bin` with `spawn` (node's, in main.js) and
 * resolve with `{ child, apiBase, internalSecret }` once `handshake`
 * — lib/backend-handshake.cjs, wired up by the caller — resolves for
 * it.
 *
 * `track` is handed the child as soon as it exists, before the
 * handshake: it is what the app's quit path and a failed boot stop,
 * and the handshake can take minutes on a first run. A quit in that
 * time has to find the backend to stop it. A handshake that fails
 * leaves it tracked, for the boot's own stop, which skips a spawn that
 * failed and a backend that has already exited (see `stoppable`).
 */
async function launchBackend({ spawn, bin, platform, env, track, handshake }) {
  const child = spawn(bin, [], backendSpawnOptions({ platform, env }));
  track(child);
  const ready = await handshake(child);
  return { child, ...ready };
}

/**
 * Whether the tree kill has a backend to stop: one that started (a
 * spawn the OS refused has no pid), that it has not stopped already,
 * and that has not exited.
 *
 * The kill acts on ids the OS hands out again. On Windows it is
 * `taskkill /T` on the backend's pid, free for reuse once the backend
 * has exited. Elsewhere it signals the backend's process group, whose
 * id is that same pid and is free once the backend and everything it
 * left in the group have exited — possibly long before the app quits.
 * Either way a kill after the exit could reach an unrelated process,
 * so an exited backend is never killed here; what it left in its group
 * is stopped when it exits (see `reapOnExit`).
 */
function stoppable(child) {
  return Boolean(
    child && !child.killed && child.pid && child.exitCode === null && child.signalCode === null,
  );
}

/**
 * When `child` exits, stop what it left in its process group: on
 * Linux and macOS, `signalGroup(pid)` once, from the 'exit' handler.
 *
 * Node reaps the backend before it reports the exit, and this runs in
 * the same turn of the event loop. A group id cannot be handed to
 * anyone else while a member is left in the group, so if the backend
 * left transports behind, the id is still theirs and the signal stops
 * them; if it left nothing, the group is gone and the signal answers
 * ESRCH. After this the id is let go: `stoppable` is false for an
 * exited child, so no later kill uses it. A quit's kill that came
 * first means a second SIGTERM to a group already told to stop, which
 * is harmless.
 *
 * Windows has no groups, so nothing is signalled; taskkill /T on a
 * live backend is what reaches its tree. A spawn that never ran has
 * no pid and nothing to stop.
 */
function reapOnExit(child, { platform, signalGroup }) {
  child.once("exit", () => {
    if (platform !== "win32" && child.pid) signalGroup(child.pid);
  });
}

module.exports = { backendSpawnOptions, launchBackend, reapOnExit, stoppable };
