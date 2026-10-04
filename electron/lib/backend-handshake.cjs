"use strict";

// The backend's startup handshake, lifted out of main.js's
// spawnBackend so its failure paths can be exercised without spawning
// anything. Behaviour is main.js's as it stood.

/**
 * Resolve with `{ apiBase, internalSecret }` once the backend has
 * printed both handshake lines; reject if it exits first.
 */
function awaitHandshake(child, { log = () => {} } = {}) {
  return new Promise((resolve, reject) => {
    let buf = "";
    let resolved = false;
    let apiBase = null;
    let internalSecret = null;

    const onLine = (line) => {
      if (resolved) {
        log(`[backend] ${line}`);
        return;
      }
      const apiMatch = line.match(/^ANI_GUI_LISTENING\s+(\S+)/);
      if (apiMatch) apiBase = apiMatch[1];
      const secretMatch = line.match(/^ANI_GUI_INTERNAL_SECRET\s+(\S+)/);
      if (secretMatch) internalSecret = secretMatch[1];
      if (apiBase && internalSecret) {
        resolved = true;
        resolve({ apiBase, internalSecret });
      }
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
    child.on("exit", (code, signal) => {
      if (!resolved) {
        reject(new Error(`backend exited before handshake (code=${code}, signal=${signal})`));
      } else {
        log(`[backend] exited (code=${code}, signal=${signal})`);
      }
    });
  });
}

module.exports = { awaitHandshake };
