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
// Pure apart from what it is handed: no Electron here.

const { isAppDocument } = require("./app-document.cjs");

/** The IPC channel the preload asks on. */
const SECRET_CHANNEL = "ani-gui:internal-secret";

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
      const secret = event && event.sender ? held.get(event.sender) : undefined;
      const frame = senderFrameOf(event);
      if (secret === undefined) {
        logError(`[secret] refused a window it was not handed to (${frameUrl(frame)})`);
        return null;
      }
      if (!isAppMainFrame(frame, app)) {
        logError(`[secret] refused a page that is not the app's (${frameUrl(frame)})`);
        return null;
      }
      return secret;
    },
  };
}

module.exports = { SECRET_CHANNEL, secretKeeper };
