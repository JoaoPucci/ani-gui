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
 * Start the OAuth callback server. Returns a handle with:
 *
 *   - `ready`: Promise resolving when the socket is bound + accepting
 *     (the `listening` event has fired). Rejects on EADDRINUSE before
 *     bind completes. The caller MUST await this before opening the
 *     consent URL in the OS browser — otherwise an already-authorised
 *     browser profile that redirects immediately can race the bind
 *     and hit ECONNREFUSED. Codex P2 #3370057919.
 *   - `promise`: resolves with `{ code, state }` on a successful
 *     callback, or rejects with a descriptive Error on timeout /
 *     port-in-use / explicit cancel.
 *   - `stop()`: cancel before a callback lands (e.g. user closes the
 *     consent page in the OS browser).
 */
// Loopback families the callback server binds. Codex P2 #3370110077:
// `server.listen(port, "localhost")` resolves to a single address via
// Node's `dns.lookup()` — usually the OS-preferred family — but the
// browser's redirect to `localhost:53682` can choose the OTHER family
// on dual-stack systems, leaving the callback refused. Bind both
// literals so either family routes home.
const LOOPBACK_HOSTS = ["127.0.0.1", "::1"];

// errno codes that mean "this loopback family isn't configured on this
// host" — IPv6 disabled in the kernel, ::1 not bound to lo, etc.
// Codex P2 #3371498414: a hardened server with IPv6 off must still be
// able to start the OAuth flow on 127.0.0.1 alone. If the missing
// family WAS what the OS resolver would have returned for `localhost`,
// the kernel that disabled the family won't return it from getaddrinfo
// either — tolerating the bind error is symmetric with the resolver.
const FAMILY_UNAVAILABLE_CODES = new Set(["EADDRNOTAVAIL", "EAFNOSUPPORT"]);

function startOAuthServer() {
  let servers = [];
  let timer = null;
  let resolveFn = null;
  let rejectFn = null;
  let readyResolve = null;
  let readyReject = null;
  let readySettled = false;
  let pendingBinds = LOOPBACK_HOSTS.length;
  let successfulBinds = 0;
  const promise = new Promise((resolve, reject) => {
    resolveFn = resolve;
    rejectFn = reject;
  });
  const ready = new Promise((resolve, reject) => {
    readyResolve = resolve;
    readyReject = reject;
  });
  // Stash a single-fire dispatcher so the same path (bind error,
  // EADDRINUSE before listening, sync throw) settles `ready` exactly
  // once — Promise resolve/reject are idempotent but linting + the
  // mental model are cleaner with the guard.
  function settleReady(ok, err) {
    if (readySettled) return;
    readySettled = true;
    if (ok) readyResolve();
    else readyReject(err);
  }

  function shutdown() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    // Close every bound family in one pass. close() is graceful —
    // it lets the in-flight callback request finish (we've already
    // written the success page) and then the underlying socket goes
    // away. The handler-side rejectFn / resolveFn are idempotent.
    const live = servers;
    servers = [];
    for (const s of live) {
      try {
        s.close();
      } catch {
        /* already closed — ignore */
      }
    }
  }

  function handler(req, res) {
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
      // Don't shutdown — keep waiting for a clean callback on either family.
      return;
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(SUCCESS_PAGE);
    shutdown();
    resolveFn({ code, state });
  }

  function onBindError(err) {
    // Two failure modes, two policies:
    //
    //  - EADDRNOTAVAIL / EAFNOSUPPORT: the OS doesn't have this
    //    loopback family configured (IPv6 off, ::1 unbound, etc.).
    //    Per Codex P2 #3371498414, tolerate this — if the family is
    //    absent, the OS's getaddrinfo for `localhost` won't return it
    //    to the browser either, so the surviving family is what the
    //    redirect actually hits.
    //
    //  - Anything else (EADDRINUSE, permission denied, hard syscall
    //    failure): fatal. EADDRINUSE in particular means a stale
    //    instance owns the port; even if the other family binds, the
    //    browser may pick the held one and either reach the wrong
    //    process or hang. Tear everything down.
    const isFamilyUnavailable = err && FAMILY_UNAVAILABLE_CODES.has(err.code);
    if (isFamilyUnavailable) {
      pendingBinds -= 1;
      // If at least one family already bound (or will), keep the flow
      // alive; ready resolves on the survivor.
      if (pendingBinds === 0) {
        if (successfulBinds === 0) {
          // No loopback family available at all — surface as
          // server_error so the toast can show the underlying code.
          const wrapped = new Error(
            `server_error: no loopback family available (${err.code})`,
          );
          shutdown();
          settleReady(false, wrapped);
          rejectFn(wrapped);
        } else {
          settleReady(true);
        }
      }
      return;
    }
    const wrapped =
      err && err.code === "EADDRINUSE"
        ? new Error("port_busy: 53682 is held by another process")
        : new Error(`server_error: ${err && err.message ? err.message : "unknown"}`);
    shutdown();
    settleReady(false, wrapped);
    rejectFn(wrapped);
  }

  for (const host of LOOPBACK_HOSTS) {
    const s = http.createServer(handler);
    s.on("error", onBindError);
    s.on("listening", () => {
      successfulBinds += 1;
      pendingBinds -= 1;
      if (pendingBinds === 0) settleReady(true);
    });
    servers.push(s);
    try {
      s.listen(OAUTH_CALLBACK_PORT, host);
    } catch (err) {
      // Sync throw is rare (most failures arrive via 'error' event)
      // but route through the same drain so we never leak a half-bound
      // server. Don't double-report if a sibling already rejected.
      if (!readySettled) onBindError(err);
    }
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
    ready,
    stop: () => {
      shutdown();
      // If stop() fires before the listening event resolves the ready
      // promise (e.g. renderer cancels during port-busy resolution),
      // settle it as a cancel so an awaiting caller doesn't hang.
      settleReady(false, new Error("cancelled"));
      rejectFn(new Error("cancelled"));
    },
  };
}

module.exports = { startOAuthServer, OAUTH_CALLBACK_PORT };
