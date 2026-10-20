"use strict";

// The flags a window is opened with, through
// `webPreferences.additionalArguments`. The preload reads them back off
// `process.argv` (preload.js); it runs sandboxed and cannot require
// this module, so the two halves are kept in step by their tests.

// A lane's name and origin travel inside one command-line argument, so
// only what survives that unquoted is sent: a lowercase name (the flag
// is `--ani-gui-lane-<name>=`) and an origin with no whitespace.
const LANE_NAME = /^[a-z]+$/;
const ORIGIN = /^\S+$/;

/**
 * `backend` is what the handshake resolved with (`apiBase`,
 * `internalSecret`, `lanes`); `locale` is the one config.toml names,
 * or null. A missing locale or secret is left out rather than sent
 * empty, so the preload falls through to its own default.
 */
function windowArguments(backend, locale) {
  const args = [`--ani-gui-api-base=${backend.apiBase}`];
  if (locale) args.push(`--ani-gui-locale=${locale}`);
  if (backend.internalSecret) {
    args.push(`--ani-gui-internal-secret=${backend.internalSecret}`);
  }
  for (const [name, origin] of Object.entries(backend.lanes || {})) {
    if (LANE_NAME.test(name) && typeof origin === "string" && ORIGIN.test(origin)) {
      args.push(`--ani-gui-lane-${name}=${origin}`);
    }
  }
  return args;
}

module.exports = { windowArguments };
