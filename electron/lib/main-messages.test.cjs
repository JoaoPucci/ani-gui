// The main process's own user-visible text.
//
// Nearly everything the user reads is the renderer's and goes through
// Paraglide. The main process shows two dialogs itself — a boot that
// failed before a page existed, and the prompt on a quit with
// downloads running — and has no access to the renderer's compiled
// messages, so it carries a small table of its own. That table has
// to cover the locales the app ships and pick among them the way the
// renderer does, or the dialog a user sees on a broken install is in
// a language the app itself would not have used.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { LOCALES, messagesFor, resolveLocale } = require("./main-messages.cjs");

const shipped = JSON.parse(
  fs.readFileSync(
    path.join(__dirname, "..", "..", "frontend", "project.inlang", "settings.json"),
    "utf8",
  ),
);

test("the table covers exactly the locales the renderer ships", () => {
  assert.deepEqual([...LOCALES].sort(), [...shipped.locales].sort());
});

/** A message's text, whatever its shape: called with a sample
 *  argument when it takes one. */
const text = (value) => (typeof value === "function" ? value(3) : value);

test("every locale says everything the base locale says", () => {
  const base = messagesFor(shipped.baseLocale);
  for (const locale of shipped.locales) {
    const table = messagesFor(locale);
    assert.deepEqual(Object.keys(table).sort(), Object.keys(base).sort(), locale);
    for (const [key, value] of Object.entries(table)) {
      assert.equal(typeof value, typeof base[key], `${locale}.${key}`);
      assert.ok(String(text(value)).trim().length > 0, `${locale}.${key}`);
    }
  }
});

test("a locale other than the base one is actually translated", () => {
  const base = messagesFor(shipped.baseLocale);
  for (const locale of shipped.locales.filter((l) => l !== shipped.baseLocale)) {
    for (const [key, value] of Object.entries(messagesFor(locale))) {
      assert.notEqual(text(value), text(base[key]), `${locale}.${key}`);
    }
  }
});

test("a locale the table does not carry falls back to the base one", () => {
  assert.equal(messagesFor("tlh"), messagesFor(shipped.baseLocale));
  assert.equal(messagesFor(null), messagesFor(shipped.baseLocale));
});

// The renderer's order, which this has to reproduce or the dialog is
// in a language the interface is not:
//
//   1. the locale saved in config.toml, when it is exactly one the app
//      ships — the renderer compares it as written, so `RU` is not
//      `ru` and falls through;
//   2. `navigator.languages`, negotiated by full tag and then by base
//      language, without regard to case. In Electron that list is the
//      application locale followed by the system's preferred
//      languages — and the application locale is Chromium's own pick
//      among the locales it ships, which maps every Latin American
//      Spanish to `es-419`. Measured on the packaged app under
//      LANG=es_MX: app.getLocale() is `es-419`,
//      getPreferredSystemLanguages() is [`es-MX`, `es`], and
//      navigator.languages is [`es-419`, `es-MX`, `es`];
//   3. the base locale.
test("the configured locale wins when it is exactly one the app ships", () => {
  assert.equal(
    resolveLocale({ configured: "pt-BR", appLocale: "ru", preferred: ["ru-RU"] }),
    "pt-BR",
  );
});

test("a configured locale in the wrong case falls through, as it does in the renderer", () => {
  assert.equal(resolveLocale({ configured: "RU", appLocale: "en-US", preferred: [] }), "en");
  assert.equal(resolveLocale({ configured: "pt-br", appLocale: "es-419", preferred: [] }), "es-419");
});

test("the application locale comes first among the system's languages", () => {
  // The audience the Spanish bundle exists for: a Mexican, Argentine
  // or Chilean system, which no entry of the preferred list matches.
  assert.equal(
    resolveLocale({ configured: null, appLocale: "es-419", preferred: ["es-MX", "es"] }),
    "es-419",
  );
  assert.equal(
    resolveLocale({ configured: null, appLocale: "ru", preferred: ["ru-RU", "ru"] }),
    "ru",
  );
});

test("without a usable configured locale the system's languages decide", () => {
  assert.equal(
    resolveLocale({ configured: null, appLocale: "de", preferred: ["de-DE", "ru-RU", "en-US"] }),
    "ru",
  );
  assert.equal(
    resolveLocale({ configured: "tlh", appLocale: "en-US", preferred: ["es-419"] }),
    "en",
  );
  assert.equal(
    resolveLocale({ configured: "", appLocale: "fr", preferred: ["de-DE", "pt-br"] }),
    "pt-BR",
  );
});

test("languages the app does not ship fall through to the base locale", () => {
  assert.equal(
    resolveLocale({ configured: null, appLocale: "pt-PT", preferred: ["pt-PT", "pt"] }),
    "en",
  );
  assert.equal(resolveLocale({ configured: null, appLocale: "", preferred: [] }), "en");
  assert.equal(resolveLocale({}), "en");
});
