"use strict";

// The e2e specs' fake backend, answering over real HTTP.
//
// A spec stubs the backend by intercepting the renderer's requests. It
// used to answer them in place, inside Playwright. That answer reaches
// the page without its status or headers whenever the main process
// has a webRequest listener on the window's session: Electron then
// sends every request through its own loader, and an answer made
// up in place arrives with status 0. So a spec holds its answer here
// and sends the request on to the URL `hold` returns (electron/e2e/
// stubs.ts does both). The request then travels through Chromium's
// network stack as a request to the backend does, and is answered as
// the backend would answer it: the backend's gate in front of every API
// path (backend/src/api/gate.rs) and its CORS rules on the answer
// (backend/src/renderer_origin.rs). A stub that the real backend would
// refuse is refused here too.
//
// Test infrastructure only; nothing in the app requires it.

const http = require("node:http");
const crypto = require("node:crypto");

/** The header the backend reads the launch's secret from. */
const SECRET_HEADER = "x-ani-gui-internal-secret";

/** The routes whose callers cannot set a header, and so may carry the
 *  secret as the `secret` query parameter. `GET` only. */
const QUERY_SECRET_PATHS = Object.freeze(["/api/image", "/api/play/stream", "/api/download/stream"]);

/** How long a browser may reuse a preflight's answer, as the backend
 *  says (two hours). */
const PREFLIGHT_MAX_AGE_SECS = 7200;

function sameSecret(got, secret) {
  if (typeof got !== "string" || typeof secret !== "string" || secret.length === 0) return false;
  const a = Buffer.from(got);
  const b = Buffer.from(secret);
  return a.length === b.length && crypto.timingSafeEqual(a, b);
}

/**
 * Whether the backend's gate lets a request through: the secret as the
 * header on any route, or as the `secret` query parameter on a GET of
 * one of QUERY_SECRET_PATHS. `headers` has lowercase names, as Node's
 * do; `secret` is the launch's, and nothing is admitted without it.
 */
function gateAdmits({ method, path, query, headers, secret }) {
  if (sameSecret(headers?.[SECRET_HEADER], secret)) return true;
  if (method !== "GET" || !QUERY_SECRET_PATHS.includes(path)) return false;
  return new URLSearchParams(query ?? "").getAll("secret").some((v) => sameSecret(v, secret));
}

/** Whether `origin` is one the app's renderer runs at: the packaged
 *  app's scheme, or a loopback dev server on any port. */
function isRendererOrigin(origin) {
  if (typeof origin !== "string") return false;
  if (origin === "app://localhost") return true;
  const m = /^http:\/\/(localhost|127\.0\.0\.1|\[::1\])(?::(\d{1,5}))?$/.exec(origin);
  return Boolean(m) && (m[2] === undefined || Number(m[2]) <= 65535);
}

/** What every answer varies on, as the backend's CORS layer says. */
const VARY = "origin, access-control-request-method, access-control-request-headers";

/** The CORS headers on an answer to a request from `origin`. */
function corsHeaders(origin) {
  if (!isRendererOrigin(origin)) return { vary: VARY };
  return {
    "access-control-allow-origin": origin,
    "access-control-expose-headers": "*",
    vary: VARY,
  };
}

/** A stub's own headers, less any CORS header: CORS is the backend's
 *  rules alone. */
function withoutCors(headers) {
  return Object.fromEntries(
    Object.entries(headers ?? {}).filter(
      ([name]) => !name.toLowerCase().startsWith("access-control-") && name.toLowerCase() !== "vary",
    ),
  );
}

function bodyOf(answer) {
  if (answer.json !== undefined) return Buffer.from(JSON.stringify(answer.json));
  if (answer.body === undefined || answer.body === null) return Buffer.alloc(0);
  return Buffer.isBuffer(answer.body) ? answer.body : Buffer.from(String(answer.body));
}

function contentTypeOf(answer) {
  if (answer.contentType) return answer.contentType;
  if (answer.json !== undefined) return "application/json";
  return undefined;
}

/**
 * Starts the server on a loopback port. `hold(answer, request)` keeps
 * `answer` — `{ status, headers, contentType, body, json }`, the shape a
 * spec's stub returns — for one request and returns the URL to send the
 * request to. `request` says what the renderer asked: its `path` and
 * `query`, and the launch's `secret`, which the gate checks for an API
 * path. `close()` stops the server.
 */
async function startStubServer() {
  const held = new Map();
  const server = http.createServer((req, res) => {
    const origin = req.headers.origin;
    const cors = corsHeaders(origin);
    // Any OPTIONS is a preflight to the backend's CORS layer, which
    // names the methods, headers and age whatever the origin, and the
    // origin only when it is the renderer's. In the e2e specs
    // Playwright answers a preflight itself before it reaches here, so
    // what a spec meets from these rules is the answer's own CORS.
    if (req.method === "OPTIONS") {
      res
        .writeHead(200, {
          ...cors,
          "access-control-allow-methods": "*",
          "access-control-allow-headers": "*",
          "access-control-max-age": String(PREFLIGHT_MAX_AGE_SECS),
        })
        .end();
      return;
    }
    const token = new URL(req.url ?? "/", "http://stub").pathname.slice(1);
    const entry = held.get(token);
    if (!entry) {
      res.writeHead(404, cors).end();
      return;
    }
    held.delete(token);
    const { answer, request } = entry;
    if (
      request.path.startsWith("/api/") &&
      !gateAdmits({
        method: req.method,
        path: request.path,
        query: request.query,
        headers: req.headers,
        secret: request.secret,
      })
    ) {
      res.writeHead(403, cors).end();
      return;
    }
    const body = bodyOf(answer);
    const headers = { ...withoutCors(answer.headers), ...cors, "content-length": String(body.length) };
    const type = contentTypeOf(answer);
    if (type) {
      for (const name of Object.keys(headers)) {
        if (name.toLowerCase() === "content-type") delete headers[name];
      }
      headers["content-type"] = type;
    }
    res.writeHead(answer.status ?? 200, headers).end(body);
  });
  // A spec worker keeps it for its whole run; it must not keep the
  // worker alive once the run is done.
  server.unref();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  return {
    origin,
    hold(answer, request) {
      const token = crypto.randomUUID();
      held.set(token, { answer: answer ?? {}, request });
      return `${origin}/${token}`;
    },
    close() {
      server.closeAllConnections?.();
      return new Promise((resolve) => server.close(() => resolve()));
    },
  };
}

module.exports = {
  QUERY_SECRET_PATHS,
  SECRET_HEADER,
  corsHeaders,
  gateAdmits,
  isRendererOrigin,
  startStubServer,
};
