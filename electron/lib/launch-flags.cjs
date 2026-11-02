"use strict";

// The launch's remote flags, read once from the backend before the
// app's window opens (see backend/src/flags). The backend decides them
// within two seconds of starting its refresh and never changes them in
// the session; `/api/flags` answers once they are decided. The window is
// opened with them (lib/window-args.cjs), so the renderer has its
// flags before its first page renders.

/** The longest the main process waits for `/api/flags`. The route
 *  waits at most three seconds for the decision before deciding the
 *  defaults itself (backend/src/flags/store.rs); this is the bound on
 *  a backend that does not answer at all, and is longer than the
 *  route's own so it never cuts an answer short. */
const FLAGS_WAIT_MS = 4000;

/**
 * Ask the backend at `apiBase` for the launch's flags. Resolves to
 * `{ origin, flags }`, or `null` when the backend does not answer
 * within `timeoutMs`, answers with an error, or answers with something
 * that is not a flags answer; the window then opens without flags and
 * every flag takes its default. Never rejects.
 */
async function waitForLaunchFlags(apiBase, { fetch, timeoutMs = FLAGS_WAIT_MS }) {
  const controller = new AbortController();
  let timer;
  const gaveUp = new Promise((resolve) => {
    timer = setTimeout(() => {
      controller.abort();
      resolve(null);
    }, timeoutMs);
  });
  const asked = (async () => {
    try {
      const res = await fetch(`${apiBase}/api/flags`, { signal: controller.signal });
      if (!res.ok) return null;
      const body = await res.json();
      return isFlagsAnswer(body) ? body : null;
    } catch {
      return null;
    }
  })();
  try {
    return await Promise.race([asked, gaveUp]);
  } finally {
    clearTimeout(timer);
  }
}

/** Whether `body` is a flags answer: a string origin and a flags
 *  object whose every value is a boolean or a string. */
function isFlagsAnswer(body) {
  if (!isPlainObject(body) || typeof body.origin !== "string") return false;
  if (!isPlainObject(body.flags)) return false;
  return Object.values(body.flags).every(
    (v) => typeof v === "boolean" || typeof v === "string",
  );
}

function isPlainObject(v) {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

module.exports = { FLAGS_WAIT_MS, isFlagsAnswer, waitForLaunchFlags };
