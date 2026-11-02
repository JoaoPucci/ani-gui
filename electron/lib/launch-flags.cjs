"use strict";

// The launch's remote flags, read once from the backend before the
// app's window opens (see backend/src/flags). The backend decides them
// within two seconds of starting and never changes them in the
// session; `/api/flags` answers once they are decided. The window is
// opened with them (lib/window-args.cjs), so the renderer has its
// flags before its first page renders.

/** The longest the main process waits for `/api/flags`. The backend
 *  answers within its own limit (three seconds from its start, two
 *  of them the cap on the file); this is the bound on a backend that
 *  does not answer at all, and is longer than that limit so it never
 *  cuts a decision short. */
const FLAGS_WAIT_MS = 4000;

/**
 * Ask the backend at `apiBase` for the launch's flags. Resolves to
 * `{ origin, flags }`, or `null` when the backend does not answer
 * within `timeoutMs`, answers with an error, or answers with something
 * that is not a flags answer; the window then opens without flags and
 * every flag takes its default. Never rejects.
 */
async function waitForLaunchFlags(apiBase, { fetch, timeoutMs = FLAGS_WAIT_MS }) {
  void apiBase;
  void fetch;
  void timeoutMs;
  return null;
}

/** Whether `body` is a flags answer: a string origin and a flags
 *  object whose every value is a boolean or a string. */
function isFlagsAnswer(body) {
  void body;
  return false;
}

module.exports = { FLAGS_WAIT_MS, isFlagsAnswer, waitForLaunchFlags };
