"use strict";

// How main.js spawns the backend, lifted out so the options can be
// checked without spawning anything. Behaviour is main.js's as it
// stood.

/** spawn() options for the backend on `platform`, with `env`. */
function backendSpawnOptions({ platform, env }) {
  return {
    stdio: ["ignore", "pipe", "pipe"],
    detached: platform !== "win32",
    env,
  };
}

module.exports = { backendSpawnOptions };
