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
 * `backend` is what the handshake resolved with (`apiBase`, `lanes`,
 * and `internalSecret`, which is never sent here: a process's command
 * line is readable by other local programs, so the preload asks the
 * main process for the secret instead, lib/secret-handoff.cjs);
 * `locale` is the one config.toml names, or null. A missing locale is
 * left out rather than sent empty, so the preload falls through to
 * Paraglide's own resolution (localStorage, preferred language, base
 * locale).
 *
 * `launchFlags` is the backend's flags answer (lib/launch-flags.cjs),
 * or null. The flags go URI-encoded JSON in one argument, so no
 * character of a name or value can split it.
 */
function windowArguments(backend, locale, launchFlags = null) {
  const args = [`--ani-gui-api-base=${backend.apiBase}`];
  if (locale) args.push(`--ani-gui-locale=${locale}`);
  for (const [name, origin] of Object.entries(backend.lanes || {})) {
    if (LANE_NAME.test(name) && typeof origin === "string" && ORIGIN.test(origin)) {
      args.push(`--ani-gui-lane-${name}=${origin}`);
    }
  }
  if (launchFlags) {
    args.push(`--ani-gui-flags=${encodeURIComponent(JSON.stringify(launchFlags.flags))}`);
    args.push(`--ani-gui-flags-origin=${encodeURIComponent(launchFlags.origin)}`);
  }
  return args;
}

module.exports = { windowArguments };
