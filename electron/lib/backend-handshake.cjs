"use strict";

// The backend's startup handshake, lifted out of main.js's
// spawnBackend so its failure paths can be exercised without spawning
// anything.

// How long the boot waits for the handshake: two minutes.
//
// The deadline exists for a backend that will never answer, not for
// one that is slow. A backend that dies is reported at once, by its
// exit or by the spawn's error, so what the deadline catches is one
// that is alive and stuck — rare, and the only cost of catching it
// late is how long the user waits to be told.
//
// A slow one is not rare. Before the handshake the backend binds a
// port, opens its SQLite cache and sweeps a legacy file, and on a
// first run opening the cache means creating it and running every
// migration, each a commit the disk has to confirm. Measured on a
// hard disk with a fresh profile: about a second on an idle disk,
// about 24 s during an 8 GB sequential write. At fifteen seconds the
// boot called that a failure, where a boot with no deadline would
// just have been late. Two minutes is five times the slowest start
// measured.
const HANDSHAKE_TIMEOUT_MS = 120_000;

/**
 * Resolve with `{ apiBase, internalSecret }` once the backend has
 * printed both handshake lines (in either order); reject when it
 * cannot get there:
 *
 *   - the spawn fails ('error' — EACCES, EPERM, ENOENT; the OS refused
 *     to run the binary, so no 'exit' follows);
 *   - it exits before the handshake;
 *   - the handshake has not completed within `timeoutMs` — the
 *     backend is stopped through `stopChild` first, since it is
 *     running and nothing else would stop it.
 *
 * After the handshake, its output and exit go to `log`, and a late
 * 'error' (a failed kill, say) is logged rather than thrown.
 */
function awaitHandshake(child, { timeoutMs, stopChild, log = () => {} }) {
  return new Promise((resolve, reject) => {
    let buf = "";
    let settled = false;
    let apiBase = null;
    let internalSecret = null;

    const settle = (fn, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      fn(value);
    };

    const timer = setTimeout(() => {
      if (settled) return;
      stopChild(child);
      settle(
        reject,
        new Error(`backend did not complete its handshake within ${timeoutMs} ms`),
      );
    }, timeoutMs);

    const onLine = (line) => {
      if (settled) {
        log(`[backend] ${line}`);
        return;
      }
      const apiMatch = line.match(/^ANI_GUI_LISTENING\s+(\S+)/);
      if (apiMatch) apiBase = apiMatch[1];
      const secretMatch = line.match(/^ANI_GUI_INTERNAL_SECRET\s+(\S+)/);
      if (secretMatch) internalSecret = secretMatch[1];
      if (apiBase && internalSecret) settle(resolve, { apiBase, internalSecret });
    };

    child.stdout.on("data", (chunk) => {
      buf += chunk.toString("utf-8");
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        onLine(line);
      }
    });
    child.on("error", (err) => {
      if (settled) log(`[backend] error: ${err && err.message}`);
      else settle(reject, err);
    });
    child.on("exit", (code, signal) => {
      if (settled) {
        log(`[backend] exited (code=${code}, signal=${signal})`);
        return;
      }
      settle(
        reject,
        new Error(`backend exited before handshake (code=${code}, signal=${signal})`),
      );
    });
  });
}

module.exports = { HANDSHAKE_TIMEOUT_MS, awaitHandshake };
