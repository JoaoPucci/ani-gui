"use strict";

// The keyring helper: the side of lib/keyring.cjs that runs in a second,
// windowless run of this app's own binary and makes every safeStorage
// call on Linux.
//
// On Linux, safeStorage reaches the OS keyring (libsecret / KWallet)
// through blocking calls on the thread that asks, and the first call of
// a process waits for the keyring's answer however long it takes: a
// locked keyring waits on its unlock prompt, a keyring that has never
// been set up on its "new keyring" prompt, an unresponsive secret
// service on the bus. Made from the main process, that wait froze the
// whole app — no first page, no window events, SIGTERM unanswered —
// because the main process's thread is the one every window runs on.
// Made here, it freezes only this process, which the app can wait on
// asynchronously and kill.
//
// The helper speaks one JSON object per line: requests on stdin,
// answers on stdout behind REPLY_MARK, so anything else the runtime
// prints there is told apart from an answer. It exits when its stdin
// closes — the app quit, or stopped wanting it.

/** The switch that starts the app's binary as the keyring helper. */
const KEYRING_HELPER_SWITCH = "--ani-gui-keyring-helper";

/** What marks a line of the helper's stdout as an answer. */
const REPLY_MARK = "ani-gui-keyring ";

/** The fields one request answers with. Throws on a failed call. */
function answer(safeStorage, req) {
  switch (req.op) {
    case "status":
      return {
        available: safeStorage.isEncryptionAvailable(),
        backend:
          typeof safeStorage.getSelectedStorageBackend === "function"
            ? safeStorage.getSelectedStorageBackend()
            : null,
      };
    case "encrypt":
      return {
        data: safeStorage.encryptString(String(req.text)).toString("base64"),
      };
    case "decrypt":
      return {
        text: safeStorage.decryptString(
          Buffer.from(String(req.data), "base64"),
        ),
      };
    default:
      throw new Error(`unknown keyring request: ${req.op}`);
  }
}

/**
 * Answer the requests arriving on `input`, one per line, in order,
 * through `write`; call `onEnd` when `input` closes. A line that is not
 * a request is ignored.
 *
 * @param {{input: NodeJS.ReadableStream, write: (line: string) => void,
 *   safeStorage: object, onEnd: () => void}} io
 */
function serveKeyring({ input, write, safeStorage, onEnd }) {
  let pending = "";
  const handle = (line) => {
    let req;
    try {
      req = JSON.parse(line);
    } catch {
      return;
    }
    if (!req || typeof req !== "object") return;
    let reply;
    try {
      reply = { id: req.id, ok: true, ...answer(safeStorage, req) };
    } catch (err) {
      reply = {
        id: req.id,
        ok: false,
        message: String((err && err.message) || err),
      };
    }
    write(`${REPLY_MARK}${JSON.stringify(reply)}\n`);
  };
  input.setEncoding("utf8");
  input.on("data", (chunk) => {
    pending += chunk;
    let nl;
    while ((nl = pending.indexOf("\n")) >= 0) {
      const line = pending.slice(0, nl);
      pending = pending.slice(nl + 1);
      if (line.trim()) handle(line);
    }
  });
  input.on("end", onEnd);
}

module.exports = { KEYRING_HELPER_SWITCH, REPLY_MARK, serveKeyring };
