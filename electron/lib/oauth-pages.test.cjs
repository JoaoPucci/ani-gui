// The pages the OAuth callback server answers the browser with after a
// sign-in. The user reads them in the browser, coming from the app, so
// they speak the app's language: each locale's page carries its own
// words from the main process's message table, and says which
// language it is in.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { oauthPage } = require("./oauth-pages.cjs");
const { LOCALES, messagesFor } = require("./main-messages.cjs");

const PAGES = [
  ["connected", "oauthConnectedTitle", "oauthConnectedBody"],
  ["failed", "oauthFailedTitle", "oauthFailedBody"],
];

test("each locale's page uses its own words and names its language", () => {
  for (const locale of LOCALES) {
    const table = messagesFor(locale);
    for (const [kind, title, body] of PAGES) {
      const page = oauthPage(kind, locale);
      assert.match(page, new RegExp(`<html lang="${locale}">`), `${locale} ${kind} lang`);
      assert.equal(typeof table[title], "string", `${locale}.${title}`);
      assert.ok(page.includes(`<h1>${table[title]}</h1>`), `${locale} ${kind} heading`);
      assert.ok(page.includes(`<p>${table[body]}</p>`), `${locale} ${kind} body`);
    }
  }
});

test("a page outside the base locale carries none of the English", () => {
  const en = messagesFor("en");
  for (const locale of LOCALES.filter((l) => l !== "en")) {
    for (const [kind, title, body] of PAGES) {
      const page = oauthPage(kind, locale);
      assert.ok(!page.includes(en[title]), `${locale} ${kind} heading is English`);
      assert.ok(!page.includes(en[body]), `${locale} ${kind} body is English`);
    }
  }
});

test("a locale the app does not ship gets the base locale's page", () => {
  const page = oauthPage("connected", "xx-YY");
  assert.match(page, /<html lang="en">/);
  assert.ok(page.includes(messagesFor("en").oauthConnectedTitle));
});

// The server writes no page of its own: a heading left in
// oauth-server.js is a page that cannot follow the app's language.
test("the callback server holds no page text of its own", () => {
  const server = fs.readFileSync(path.join(__dirname, "..", "oauth-server.js"), "utf8");
  for (const english of ["Connected to ani-gui", "Connection failed", "<html"]) {
    assert.ok(!server.includes(english), `oauth-server.js still has ${english}`);
  }
});
