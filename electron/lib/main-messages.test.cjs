// The main process's own user-visible text.
//
// Nearly everything the user reads is the renderer's and goes through
// Paraglide. The main process speaks only when there is no renderer to
// speak for it — a boot that failed before a page existed — and has no
// access to the renderer's compiled messages, so it carries a small
// table of its own. That table has to cover the locales the app ships
// and pick among them the way the renderer does, or the one dialog a
// user sees on a broken install is in a language the app itself would
// not have used.

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
const text = (value) => (typeof value === "function" ? value("sample") : value);

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
    assert.notEqual(messagesFor(locale).bootFailed, base.bootFailed, locale);
  }
});

test("a locale the table does not carry falls back to the base one", () => {
  assert.equal(messagesFor("tlh"), messagesFor(shipped.baseLocale));
  assert.equal(messagesFor(null), messagesFor(shipped.baseLocale));
});

// The renderer's order: the locale saved in config.toml, then the
// system's preferred languages — full tag first, then its base
// language, each compared without regard to case — then the base
// locale. `es-MX` therefore lands on English, as it does in the
// renderer: `es` alone is not a locale the app ships.
test("the configured locale wins when the app ships it", () => {
  assert.equal(resolveLocale({ configured: "pt-BR", preferred: ["ru-RU"] }), "pt-BR");
  assert.equal(resolveLocale({ configured: "RU", preferred: [] }), "ru");
});

test("without a usable configured locale the system's preferences decide", () => {
  assert.equal(resolveLocale({ configured: null, preferred: ["ru-RU", "en-US"] }), "ru");
  assert.equal(resolveLocale({ configured: "tlh", preferred: ["es-419"] }), "es-419");
  assert.equal(resolveLocale({ configured: "", preferred: ["de-DE", "pt-br"] }), "pt-BR");
});

test("a preference the app does not ship falls through to the base locale", () => {
  assert.equal(resolveLocale({ configured: null, preferred: ["es-MX"] }), "en");
  assert.equal(resolveLocale({ configured: null, preferred: [] }), "en");
  assert.equal(resolveLocale({}), "en");
});
