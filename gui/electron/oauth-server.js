// Ephemeral 127.0.0.1:53682 HTTP server that catches OAuth redirects.
//
// Used by the account-integration feature (see
// .planning/account-integration.md §3.3): when the user clicks "Sign
// in with AniList" or "Sign in with MyAnimeList", main.js starts this
// server, opens the provider's authorize URL in the OS browser, the
// browser redirects to http://localhost:53682/callback?code=&state=,
// we capture the params and resolve the waiter.
//
// Port is fixed at 53682 because AniList only allows ONE redirect URI
// per API client (registered at https://anilist.co/settings/developer).
// If the port is held by another process at OAuth start, the caller
// surfaces a "Port 53682 in use" toast.
//
// Lifetime: started on connect, stopped on:
//   - successful callback (auto)
//   - 5-min timeout (auto)
//   - explicit stop() (renderer cancel)
//
// State (CSRF token) validation lives in the renderer, not here — the
// renderer generated the state; this server just round-trips it back
// for verification.

"use strict";

const http = require("node:http");

const OAUTH_CALLBACK_PORT = 53682;
const OAUTH_TIMEOUT_MS = 5 * 60 * 1000;

const SUCCESS_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>ani-gui</title>
<style>
  body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
         background: #0f0f10; color: #f5f5f4; display: grid; place-items: center;
         min-height: 100vh; padding: 2rem; }
  main { text-align: center; max-width: 28rem; }
  h1 { font-weight: 600; margin: 0 0 .75rem; }
  p { opacity: .7; line-height: 1.5; margin: 0; }
  .check { font-size: 3rem; margin-bottom: 1rem; }
</style></head>
<body><main>
  <div class="check">✓</div>
  <h1>Connected to ani-gui</h1>
  <p>You can close this tab and return to the app.</p>
</main></body></html>`;

const ERROR_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>ani-gui</title>
<style>
  body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
         background: #0f0f10; color: #f5f5f4; display: grid; place-items: center;
         min-height: 100vh; padding: 2rem; }
  main { text-align: center; max-width: 28rem; }
  h1 { font-weight: 600; margin: 0 0 .75rem; }
  p { opacity: .7; line-height: 1.5; margin: 0; }
  .x { font-size: 3rem; margin-bottom: 1rem; color: #c44; }
</style></head>
<body><main>
  <div class="x">×</div>
  <h1>Connection failed</h1>
  <p>The authorisation was declined or returned an error. Close this tab and try again from ani-gui.</p>
</main></body></html>`;

/**
 * Start the OAuth callback server. Returns a Promise that resolves
 * with `{ code, state }` on a successful callback, or rejects with a
 * descriptive Error on timeout / port-in-use / explicit cancel.
 *
 * The returned object also includes a `stop()` method the caller can
 * use to cancel before a callback lands (e.g. user closes the consent
 * page in the OS browser).
 */
function startOAuthServer() {
  let server = null;
  let timer = null;
  let resolveFn = null;
  let rejectFn = null;
  const promise = new Promise((resolve, reject) => {
    resolveFn = resolve;
    rejectFn = reject;
  });

  function shutdown() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (server) {
      const s = server;
      server = null;
      // close() on http.Server is graceful — it lets in-flight requests
      // finish. We've already written the success page so the request
      // is complete; closing is immediate.
      s.close();
    }
  }

  server = http.createServer((req, res) => {
    // Only the /callback path matters; everything else gets 404'd so
    // a stray favicon request doesn't trigger anything.
    const url = new URL(req.url || "/", "http://127.0.0.1");
    if (url.pathname !== "/callback") {
      res.statusCode = 404;
      res.end();
      return;
    }
    const code = url.searchParams.get("code");
    const state = url.searchParams.get("state");
    const error = url.searchParams.get("error");
    if (error) {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(ERROR_PAGE);
      shutdown();
      rejectFn(new Error(`oauth_error: ${error}`));
      return;
    }
    if (!code || !state) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end("Missing code or state");
      // Don't shutdown — keep waiting for a clean callback.
      return;
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(SUCCESS_PAGE);
    shutdown();
    resolveFn({ code, state });
  });

  server.on("error", (err) => {
    // EADDRINUSE on bind is the most common failure — the user has
    // another OAuth-running process or a leftover instance bound to
    // 53682. Surface it specifically so the toast can be actionable.
    if (err && err.code === "EADDRINUSE") {
      rejectFn(new Error("port_busy: 53682 is held by another process"));
    } else {
      rejectFn(new Error(`server_error: ${err && err.message ? err.message : "unknown"}`));
    }
    server = null;
  });

  try {
    // Bind to "localhost" so the listener picks up whichever stack
    // (IPv4 127.0.0.1 vs IPv6 ::1) the OS's DNS resolution prefers.
    // The provider redirects to the literal string `localhost:53682`
    // and the browser resolves it via the same DNS; binding only to
    // 127.0.0.1 misses ::1 on dual-stack hosts where IPv6 is
    // preferred. Codex P2 #3369941706.
    server.listen(OAUTH_CALLBACK_PORT, "localhost");
  } catch (err) {
    rejectFn(new Error(`server_error: ${err.message}`));
  }

  // Set a hard wall-clock timeout so a never-completing flow doesn't
  // leave the port held forever. The renderer's UI also shows a
  // cancel button; that path calls stop() before this fires.
  timer = setTimeout(() => {
    shutdown();
    rejectFn(new Error("timeout: no callback within 5 minutes"));
  }, OAUTH_TIMEOUT_MS);

  return {
    promise,
    stop: () => {
      shutdown();
      rejectFn(new Error("cancelled"));
    },
  };
}

module.exports = { startOAuthServer, OAUTH_CALLBACK_PORT };
