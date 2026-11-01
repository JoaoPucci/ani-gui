"use strict";

// Every IPC handler in the main process registers through here, and
// acts only for a call from the main frame holding the app's own
// document (app-document.cjs). tests/arch/ipc_sender.sh keeps
// registrations from going around it.
//
// The navigation guard (window-guard.cjs) is meant to keep any other
// document out of the window. This is the line behind it: a document
// that got in anyway — about:blank, which Chromium commits without
// asking the guard, or a path nobody has found yet — runs the preload,
// but the bridge's calls into main are refused.
//
// It cannot take back what the preload hands out without IPC: the
// backend origins and internal secret it reads from the window's
// launch arguments. Keeping other documents out is the guard's job.

const { isAppDocument } = require("./app-document.cjs");

/** Whether `frame` (an IpcMainEvent's senderFrame) is the app's main frame. */
function isTrustedSender(frame, app) {
  try {
    return Boolean(frame) && frame.parent === null && isAppDocument(frame.url, app);
  } catch {
    // A frame disposed mid-call throws when read.
    return false;
  }
}

function senderFrameOf(event) {
  try {
    return event.senderFrame;
  } catch {
    return null;
  }
}

function describe(frame) {
  try {
    return frame ? String(frame.url) : "<no frame>";
  } catch {
    return "<disposed frame>";
  }
}

/**
 * Wrap `ipcMain` so every handler checks its caller:
 *   - handle(channel, fn)              an untrusted invoke rejects;
 *   - on(channel, fn)                  an untrusted message is dropped;
 *   - onSync(channel, fn, refusal)     an untrusted sendSync gets `refusal`
 *                                      as its answer — a sync call left
 *                                      without one hangs the renderer.
 */
function guardedIpc(ipcMain, { app, logError }) {
  const trusted = (channel, event) => {
    const frame = senderFrameOf(event);
    if (isTrustedSender(frame, app)) return true;
    logError(`[ipc] refused ${channel} from ${describe(frame)}`);
    return false;
  };
  return {
    handle(channel, fn) {
      ipcMain.handle(channel, (event, ...args) => {
        if (!trusted(channel, event)) throw new Error(`refused: ${channel}`);
        return fn(event, ...args);
      });
    },
    on(channel, fn) {
      ipcMain.on(channel, (event, ...args) => {
        if (!trusted(channel, event)) return;
        fn(event, ...args);
      });
    },
    onSync(channel, fn, refusal) {
      if (refusal === undefined) {
        throw new Error(`onSync(${channel}) needs a refusal value to answer an untrusted caller`);
      }
      ipcMain.on(channel, (event, ...args) => {
        if (!trusted(channel, event)) {
          event.returnValue = refusal;
          return;
        }
        fn(event, ...args);
      });
    },
  };
}

module.exports = { guardedIpc, isTrustedSender };
