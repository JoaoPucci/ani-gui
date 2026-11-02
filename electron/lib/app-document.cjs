"use strict";

// Which URLs are the app's own document — the one page the main window
// is meant to hold, and the only one the preload bridge is meant for.
//
// Packaged builds serve the SvelteKit bundle over the private `app://`
// scheme, host `localhost`; development serves it from the Vite dev
// server (VITE_DEV_URL). Every decision here compares the parsed
// protocol, hostname and port, and rejects credentials.
//
// Never `URL.origin`. Node — like the WHATWG URL standard it follows —
// reports the origin of `app://localhost/` as the string 'null', and
// reports the same 'null' for file:, data:, about:, blob: of an opaque
// origin and every other non-special scheme. A guard that compared
// origins would either refuse the app itself or admit all of those as
// the app.
//
// Pure: no Electron, no I/O. Windows needs nothing of its own here —
// a file URL there is `file:///C:/…` or `file://server/share/…`, and
// both are simply not the app's protocol.

/** The app's own document, from how this process was launched. */
function appDocumentFrom({ isDev, devUrl }) {
  if (!isDev) return Object.freeze({ protocol: "app:", hostname: "localhost", port: "" });
  const dev = parse(devUrl);
  // A dev URL that does not parse admits nothing rather than guessing.
  if (!dev) return Object.freeze({ protocol: null, hostname: null, port: null });
  return Object.freeze({ protocol: dev.protocol, hostname: dev.hostname, port: dev.port });
}

function parse(url) {
  if (typeof url !== "string") return null;
  try {
    return new URL(url);
  } catch {
    return null;
  }
}

function isWeb(u) {
  return u.protocol === "http:" || u.protocol === "https:";
}

/** Whether `url` is the app's own document. */
function isAppDocument(url, app) {
  const u = parse(url);
  return u !== null && sameDocument(u, app);
}

function sameDocument(u, app) {
  return (
    app.protocol !== null &&
    u.protocol === app.protocol &&
    u.hostname === app.hostname &&
    u.port === app.port &&
    u.username === "" &&
    u.password === ""
  );
}

/**
 * What the window does when something asks it to load `url`:
 *   - 'allow'    — the app's own document (a route, a reload);
 *   - 'external' — a web page: refused here, offered to the system browser;
 *   - 'deny'     — anything else (file:, data:, about:, other schemes).
 */
function navigationVerdict(url, app) {
  const u = parse(url);
  if (u === null) return "deny";
  if (sameDocument(u, app)) return "allow";
  return isWeb(u) ? "external" : "deny";
}

/**
 * What the window does when something asks for a new window on `url`.
 * The app never opens a second window: its own routes are refused (a
 * middle-click on an in-app link would otherwise spawn a window without
 * the preload), web pages go to the system browser, the rest is refused.
 */
function windowOpenVerdict(url, app) {
  return navigationVerdict(url, app) === "external" ? "external" : "deny";
}

module.exports = {
  appDocumentFrom,
  isAppDocument,
  navigationVerdict,
  windowOpenVerdict,
};
