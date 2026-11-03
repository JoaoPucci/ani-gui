"use strict";

// The OS keyring as the main process uses it: every call asynchronous,
// none of them on the thread the windows run on.
//
// On Linux the calls go to the keyring helper (lib/keyring-helper.cjs),
// a windowless run of the app's own binary started on the first call
// and kept for the rest of the session: safeStorage's first call there
// waits for the keyring however long it takes (see that file), and only
// another process can do that waiting without freezing the app. A
// token read that waits is a pending promise here and a pending IPC
// call in the renderer, which renders signed out meanwhile and takes
// the accounts when the keyring answers.
//
// Nothing here times a call out. A keyring waiting on its unlock prompt
// is waiting on the user, and a write cut off while the helper still
// holds it could land after the app reported it failed. Nothing here
// deletes or rewrites a token file either: a read that fails leaves the
// file where it was.
//
// On Windows (DPAPI) and macOS, safeStorage answers in-process without
// a prompt, and is called directly.

const { KEYRING_HELPER_SWITCH, REPLY_MARK } = require("./keyring-helper.cjs");

/**
 * The command that starts the keyring helper: this binary again, with
 * the app's path in front when it is not packaged (`electron .`). The
 * helper opens no window and loads no web content: headless Ozone, no
 * GPU, and no sandbox for renderers it never starts.
 */
function helperCommand({ execPath, isPackaged, appPath, env }) {
  const args = isPackaged ? [] : [appPath];
  args.push(
    KEYRING_HELPER_SWITCH,
    "--ozone-platform=headless",
    "--disable-gpu",
    "--no-sandbox",
  );
  const childEnv = { ...env };
  delete childEnv.ELECTRON_RUN_AS_NODE;
  return { command: execPath, args, env: childEnv };
}

/**
 * A client of the keyring helper. `request(op, fields)` resolves with
 * the helper's answer or rejects with its error; the helper is started
 * on the first request, and again on the next after it ended. Requests
 * still out when it ends reject. `stop()` kills it: a helper waiting on
 * the keyring answers no signal it could handle.
 */
function createHelperClient({ spawn, command, args, env }) {
  let current = null;
  let nextId = 1;

  function start() {
    const child = spawn(command, args, {
      stdio: ["pipe", "pipe", "ignore"],
      env,
    });
    const run = { child, pending: new Map(), ended: false };
    const end = (err) => {
      if (run.ended) return;
      run.ended = true;
      if (current === run) current = null;
      for (const { reject } of run.pending.values()) reject(err);
      run.pending.clear();
    };
    let buffered = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      buffered += chunk;
      let nl;
      while ((nl = buffered.indexOf("\n")) >= 0) {
        const line = buffered.slice(0, nl);
        buffered = buffered.slice(nl + 1);
        if (!line.startsWith(REPLY_MARK)) continue;
        let reply;
        try {
          reply = JSON.parse(line.slice(REPLY_MARK.length));
        } catch {
          continue;
        }
        const waiter = run.pending.get(reply.id);
        if (!waiter) continue;
        run.pending.delete(reply.id);
        if (reply.ok) waiter.resolve(reply);
        else
          waiter.reject(new Error(reply.message || "keyring request failed"));
      }
    });
    // A write to a helper that already ended fails with EPIPE; the
    // exit below rejects what was waiting.
    child.stdin.on("error", () => {});
    child.on("error", (err) => end(err));
    child.on("exit", (code, signal) =>
      end(new Error(`keyring helper ended (${signal || `code ${code}`})`)),
    );
    return run;
  }

  return {
    request(op, fields = {}) {
      if (!current) current = start();
      const run = current;
      const id = nextId++;
      return new Promise((resolve, reject) => {
        run.pending.set(id, { resolve, reject });
        run.child.stdin.write(`${JSON.stringify({ id, op, ...fields })}\n`);
      });
    },
    stop() {
      if (current) {
        const { child } = current;
        current = null;
        try {
          child.kill("SIGKILL");
        } catch {
          /* already gone */
        }
      }
    },
  };
}

/**
 * The keyring: `status()`, `encrypt(text)` and `decrypt(buffer)`, each
 * a promise. On Linux through `helper`; elsewhere `safeStorage` itself.
 */
function createKeyring({ platform, safeStorage, helper }) {
  if (platform === "linux") {
    return {
      status: () =>
        helper
          .request("status")
          .then((r) => ({
            available: Boolean(r.available),
            backend: r.backend ?? null,
          })),
      encrypt: (text) =>
        helper
          .request("encrypt", { text })
          .then((r) => Buffer.from(r.data, "base64")),
      decrypt: (buffer) =>
        helper
          .request("decrypt", { data: buffer.toString("base64") })
          .then((r) => r.text),
    };
  }
  return {
    status: async () => ({
      available: safeStorage.isEncryptionAvailable(),
      backend:
        typeof safeStorage.getSelectedStorageBackend === "function"
          ? safeStorage.getSelectedStorageBackend()
          : null,
    }),
    encrypt: async (text) => safeStorage.encryptString(text),
    decrypt: async (buffer) => safeStorage.decryptString(buffer),
  };
}

/**
 * Whether the keyring encrypts for real. safeStorage reports itself
 * available on Linux even when it fell back to `basic_text`, which
 * XORs the payload with a fixed string; that, and an `unknown` backend,
 * count as no encryption. A platform that names no backend (Windows,
 * macOS) has none of those fallbacks.
 */
function isRealEncryption({ available, backend }) {
  if (!available) return false;
  if (backend == null) return true;
  return backend !== "basic_text" && backend !== "unknown";
}

const message = (err) => String((err && err.message) || err);

/**
 * Read and decrypt the token file `file()` names. A missing file is
 * `not_found` and asks the keyring nothing.
 */
async function readToken({ file, fs, keyring }) {
  try {
    const p = file();
    if (!fs.existsSync(p)) return { ok: false, kind: "not_found" };
    // The file is there but the keychain that wrote it is not usable
    // (or never was): refuse rather than hand back a plaintext-fallback
    // token.
    if (!isRealEncryption(await keyring.status())) {
      return { ok: false, kind: "encryption_unavailable" };
    }
    const plain = await keyring.decrypt(fs.readFileSync(p));
    return { ok: true, payload: JSON.parse(plain) };
  } catch (err) {
    return { ok: false, kind: "decrypt_error", message: message(err) };
  }
}

/** Encrypt `payload` and write it to the token file `file()` names. */
async function writeToken({ file, fs, keyring, payload }) {
  let status;
  try {
    status = await keyring.status();
  } catch {
    status = { available: false, backend: null };
  }
  if (!isRealEncryption(status))
    return { ok: false, kind: "encryption_unavailable" };
  if (typeof payload !== "object" || payload == null)
    return { ok: false, kind: "bad_request" };
  try {
    const encrypted = await keyring.encrypt(JSON.stringify(payload));
    fs.writeFileSync(file(), encrypted);
    return { ok: true };
  } catch (err) {
    return { ok: false, kind: "io_error", message: message(err) };
  }
}

module.exports = {
  createHelperClient,
  createKeyring,
  helperCommand,
  isRealEncryption,
  readToken,
  writeToken,
};
