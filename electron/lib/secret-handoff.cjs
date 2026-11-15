"use strict";

// How the launch's secret reaches the preload.
//
// The backend prints a per-launch secret in its handshake, and answers
// its API only to requests carrying it (backend/src/api/gate.rs). The
// renderer needs it; the renderer's command line is no place for it,
// since any local program can read a process's command line
// (/proc/<pid>/cmdline on Linux, the process list on Windows). So the
// main process keeps it, per window, and the preload asks for it once,
// synchronously, as it loads: synchronously because the bridge exposes
// it as a plain value the page reads before its first request.
//
// Main answers only the app's own page: the main frame of a window the
// secret was handed to (`hold`), holding the app's document
// (app-document.cjs). Any other page that ends up in the window runs the
// same preload and is answered null — a page the window was navigated
// to, a subframe, the splash, a frame already gone.
//
// The same keeper says which requests the main process adds the
// secret header to (`forRequest`, secret-header.cjs).
//
// Pure apart from what it is handed: no Electron here.

const { isAppDocument } = require("./app-document.cjs");

/** The IPC channel the preload asks on. */
const SECRET_CHANNEL = "ani-gui:internal-secret";

/** The header the backend reads the secret from (gate.rs). */
const SECRET_HEADER = "x-ani-gui-internal-secret";

function senderFrameOf(event) {
  try {
    return event.senderFrame;
  } catch {
    return null;
  }
}

function frameUrl(frame) {
  try {
    return frame ? String(frame.url) : "<no frame>";
  } catch {
    return "<disposed frame>";
  }
}

/** The secret held for the event's window, or undefined, including
 *  when the window cannot be read. */
function heldBy(held, event) {
  try {
    return event.sender ? held.get(event.sender) : undefined;
  } catch {
    return undefined;
  }
}

function isAppMainFrame(frame, app) {
  try {
    return Boolean(frame) && frame.parent === null && isAppDocument(frame.url, app);
  } catch {
    // A frame disposed mid-call throws when read.
    return false;
  }
}

/**
 * The windows' secrets, and who gets them. `app` is the app's document
 * (appDocumentFrom); `logError` hears every refusal, which names the
 * page refused and never the secret.
 */
function secretKeeper({ app, logError }) {
  const held = new WeakMap();
  return {
    /** Hand `secret` to the window whose contents are `contents`. */
    hold(contents, secret) {
      if (typeof secret === "string" && secret.length > 0) held.set(contents, secret);
    },
    /** The answer to an IPC `event` asking for the secret: the secret,
     *  or null. */
    answer(event) {
      // The frame first: a caller that is not the app's page is
      // refused before anything else of it is read.
      const frame = senderFrameOf(event);
      if (!isAppMainFrame(frame, app)) {
        logError(`[secret] refused a page that is not the app's (${frameUrl(frame)})`);
        return null;
      }
      const secret = heldBy(held, event);
      if (secret === undefined) {
        logError(`[secret] refused a window it was not handed to (${frameUrl(frame)})`);
        return null;
      }
      return secret;
    },
    /** The secret for a request the main process adds the header to
     *  (secret-header.cjs), judged as `answer` judges a caller: the
     *  app's main frame, in a window it was handed to. `request` is
     *  `{ webContents, frame }`; anything else is answered null, and
     *  is not logged, since a page's every image would be. */
    forRequest(request) {
      if (!isAppMainFrame(request?.frame, app)) return null;
      try {
        return (request.webContents && held.get(request.webContents)) || null;
      } catch {
        return null;
      }
    },
  };
}

module.exports = { SECRET_CHANNEL, SECRET_HEADER, secretKeeper };
