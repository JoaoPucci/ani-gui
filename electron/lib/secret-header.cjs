"use strict";

// The launch's secret, added as a header to the requests the page
// cannot add it to itself.
//
// The backend answers its API only to a request carrying the secret as
// the `x-ani-gui-internal-secret` header (backend/src/api/gate.rs). A
// `fetch` sets it. An <img> (the image proxy) and an EventSource (the
// play and download progress streams) cannot, and a secret in their URL
// would be kept where a header is not: Chromium's disk cache keys every
// response it stores by its URL, and a log line naming the URL names the
// secret. So the main process adds the header to those requests on
// their way out (session.webRequest.onBeforeSendHeaders, main.js), for
// the app's own page alone: who that is, the secret keeper decides
// (secret-handoff.cjs `forRequest`). Every other request goes out as it
// came.
//
// Pure apart from what it is handed: no Electron here.

const { SECRET_HEADER } = require("./secret-handoff.cjs");

/** The routes an <img> or an EventSource loads. */
const HEADERLESS_PATHS = Object.freeze(["/api/image", "/api/play/stream", "/api/download/stream"]);

/** The backend's origins: its API's and every lane's, once each. A
 *  page falls back to the API's origin for a lane the bridge lacks, so
 *  every one is a place the routes can be asked. */
function backendOrigins(backend) {
  const bases = [backend?.apiBase, ...Object.values(backend?.lanes ?? {})];
  const origins = new Set();
  for (const base of bases) {
    if (typeof base !== "string") continue;
    try {
      const { origin, protocol } = new URL(base);
      if (protocol === "http:") origins.add(origin);
    } catch {
      // Not a URL: nothing to match.
    }
  }
  return [...origins];
}

/** The webRequest filter: the header-less routes on the backend's
 *  origins, and nothing else, so no other request waits on the main
 *  process. */
function headerlessFilter(backend) {
  return {
    urls: backendOrigins(backend).flatMap((origin) =>
      HEADERLESS_PATHS.map((path) => `${origin}${path}*`),
    ),
  };
}

function readFrame(details) {
  try {
    return details.frame;
  } catch {
    return null;
  }
}

/**
 * The headers a request goes out with: `details.requestHeaders` with
 * the secret added when the request is a GET of a header-less route on
 * one of `origins` and `secretFor(details)` answers one; otherwise the
 * headers as they came. `secretFor` judges who is asking.
 */
function withLaunchSecret(details, { origins, secretFor }) {
  const headers = details.requestHeaders ?? {};
  let url;
  try {
    url = new URL(details.url);
  } catch {
    return headers;
  }
  if (details.method !== "GET") return headers;
  if (!origins.includes(url.origin) || !HEADERLESS_PATHS.includes(url.pathname)) return headers;
  const secret = secretFor({ webContents: details.webContents, frame: readFrame(details) });
  if (typeof secret !== "string" || secret.length === 0) return headers;
  return { ...headers, [SECRET_HEADER]: secret };
}

module.exports = {
  HEADERLESS_PATHS,
  SECRET_HEADER,
  backendOrigins,
  headerlessFilter,
  withLaunchSecret,
};
