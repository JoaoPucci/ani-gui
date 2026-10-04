"use strict";

// The backend's startup handshake, lifted out of main.js's
// spawnBackend so its failure paths can be exercised without spawning
// anything.

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

module.exports = { awaitHandshake };
