"use strict";

// The pages the OAuth callback server (oauth-server.js) answers the
// browser with after a sign-in. The user arrives on them from the app,
// so they speak the app's language: their words come from the main
// process's message table (lib/main-messages.cjs), in the locale the
// main process resolves the way the renderer does.

const { messagesFor, shippedLocale } = require("./main-messages.cjs");

const STYLE = `
  body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
         background: #0f0f10; color: #f5f5f4; display: grid; place-items: center;
         min-height: 100vh; padding: 2rem; }
  main { text-align: center; max-width: 28rem; }
  h1 { font-weight: 600; margin: 0 0 .75rem; }
  p { opacity: .7; line-height: 1.5; margin: 0; }
  .mark { font-size: 3rem; margin-bottom: 1rem; }
  .failed { color: #c44; }
`;

const KINDS = {
  connected: { mark: "✓", markClass: "mark", title: "oauthConnectedTitle", body: "oauthConnectedBody" },
  failed: { mark: "×", markClass: "mark failed", title: "oauthFailedTitle", body: "oauthFailedBody" },
};

/**
 * The page for a sign-in that `kind` ("connected" or "failed")
 * describes, in `locale`. A locale the app does not ship gets the base
 * locale's page, as the main process's dialogs do.
 */
function oauthPage(kind, locale) {
  const page = KINDS[kind];
  const lang = shippedLocale(locale) || "en";
  const table = messagesFor(lang);
  return `<!doctype html>
<html lang="${lang}"><head><meta charset="utf-8"><title>ani-gui</title>
<style>${STYLE}</style></head>
<body><main>
  <div class="${page.markClass}">${page.mark}</div>
  <h1>${table[page.title]}</h1>
  <p>${table[page.body]}</p>
</main></body></html>`;
}

module.exports = { oauthPage };
