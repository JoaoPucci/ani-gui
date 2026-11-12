"use strict";

// Keeps a window on the app's own document (see app-document.cjs).
//
// Every document a window loads runs the preload, and the preload hands
// it the bridge: the backend's internal secret, the OAuth token reads,
// revealInFolder. So the window may hold the app's document and nothing
// else. Installed on each window as it is created.
//
//   will-navigate        main frame, renderer-initiated: the app's own
//                        document passes; a web page is refused and
//                        offered to the system browser; the rest is
//                        refused.
//   will-redirect        a redirect out of the app is refused, never
//                        opened — nobody asked for the target.
//   will-frame-navigate  subframes (the app has none): the same as a
//                        redirect. The main frame is left to
//                        will-navigate, so a link opens once.
//   will-attach-webview  always refused.
//   window open          never a new window; web links go to the system
//                        browser.
//
// What does not pass through here, by design: client-side routing,
// hash and query changes (same-document, no navigation), reloads, and
// the main process's own loadURL.

const { navigationVerdict, windowOpenVerdict } = require("./app-document.cjs");

/** The URL a navigation event is about: the details object, else the legacy argument. */
function eventUrl(event, legacyUrl) {
  return event && typeof event.url === "string" ? event.url : legacyUrl;
}

function guardWindow(contents, { app, openExternal, logError }) {
  // shell.openExternal rejects on a host with no default browser (or no
  // xdg-open / portal); left alone that is an unhandled rejection.
  const failed = (url) => (err) =>
    logError(`[window] could not open ${url} externally:`, String((err && err.message) || err));
  const openOutside = (url) => {
    let opening;
    try {
      opening = openExternal(url);
    } catch (err) {
      failed(url)(err);
      return;
    }
    Promise.resolve(opening).catch(failed(url));
  };

  contents.on("will-navigate", (event, legacyUrl) => {
    const url = eventUrl(event, legacyUrl);
    const verdict = navigationVerdict(url, app);
    if (verdict === "allow") return;
    event.preventDefault();
    if (verdict === "external") openOutside(url);
  });

  contents.on("will-redirect", (event, legacyUrl) => {
    if (navigationVerdict(eventUrl(event, legacyUrl), app) !== "allow") event.preventDefault();
  });

  contents.on("will-frame-navigate", (event) => {
    if (event.isMainFrame) return;
    if (navigationVerdict(event.url, app) !== "allow") event.preventDefault();
  });

  contents.on("will-attach-webview", (event) => {
    event.preventDefault();
  });

  contents.setWindowOpenHandler(({ url }) => {
    if (windowOpenVerdict(url, app) === "external") openOutside(url);
    return { action: "deny" };
  });
}

module.exports = { guardWindow };
