"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fc = require("fast-check");

const {
  appDocumentFrom,
  isAppDocument,
  navigationVerdict,
  windowOpenVerdict,
} = require("./app-document.cjs");

const PACKAGED = appDocumentFrom({ isDev: false });
const DEV = appDocumentFrom({ isDev: true, devUrl: "http://localhost:5173" });

// ─── The app's own document ────────────────────────────────────────

test("packaged: the app scheme's own host is the app's document", () => {
  for (const url of [
    "app://localhost/",
    "app://localhost/settings",
    "app://localhost/anime/123?page=2#ep-4",
    "app://localhost/index.html",
  ]) {
    assert.strictEqual(navigationVerdict(url, PACKAGED), "allow", url);
  }
});

test("dev: the Vite origin is the app's document, and only that origin", () => {
  assert.strictEqual(navigationVerdict("http://localhost:5173/", DEV), "allow");
  assert.strictEqual(navigationVerdict("http://localhost:5173/play/1?ep=2", DEV), "allow");
  // Another port or another spelling of the host is not the dev server
  // the window was opened on.
  assert.strictEqual(navigationVerdict("http://localhost:5174/", DEV), "external");
  assert.strictEqual(navigationVerdict("http://127.0.0.1:5173/", DEV), "external");
  assert.strictEqual(navigationVerdict("https://localhost:5173/", DEV), "external");
  // The packaged scheme means nothing under dev.
  assert.strictEqual(navigationVerdict("app://localhost/", DEV), "deny");
});

test("dev: a VITE_DEV_URL that does not parse admits nothing", () => {
  const broken = appDocumentFrom({ isDev: true, devUrl: "not a url" });
  assert.strictEqual(navigationVerdict("http://localhost:5173/", broken), "external");
  assert.strictEqual(navigationVerdict("app://localhost/", broken), "deny");
  assert.strictEqual(isAppDocument("not a url", broken), false);
});

// ─── The origin trap ───────────────────────────────────────────────

test("the 'null' origin of a custom scheme admits nothing", () => {
  // Node reports `new URL('app://localhost/').origin` as the string
  // 'null' — exactly what it reports for file:, data:, about: and
  // every other non-special scheme. A guard comparing `.origin` would
  // let all of them through as the app.
  assert.strictEqual(new URL("app://localhost/").origin, "null");
  for (const url of [
    "file:///etc/hostname",
    "data:text/html,<p>x</p>",
    "about:blank",
    "about:srcdoc",
    "blob:null/0b2f",
    "javascript:alert(1)",
    "app:localhost/",
    "app:///",
    "app://localhost.evil/",
    "app://evil/",
    "app://localhost:8080/",
    "app://user@localhost/",
    "app://user:pw@localhost/",
    "chrome://gpu",
    "devtools://devtools/bundled/inspector.html",
    "ftp://localhost/",
    "ws://localhost/",
    "filesystem:app://localhost/temporary/x",
  ]) {
    assert.strictEqual(navigationVerdict(url, PACKAGED), "deny", url);
  }
});

test("Windows shapes of a local file are refused", () => {
  for (const url of [
    "file:///C:/Users/me/Downloads/page.html",
    "file:///c:/x.html",
    "file://server/share/page.html",
    "file://localhost/C:/x.html",
    // Not URLs at all; one parses with `c:` as its scheme.
    "C:\\Users\\me\\page.html",
    "\\\\server\\share\\page.html",
  ]) {
    assert.strictEqual(navigationVerdict(url, PACKAGED), "deny", url);
    assert.strictEqual(navigationVerdict(url, DEV), "deny", url);
    assert.strictEqual(windowOpenVerdict(url, PACKAGED), "deny", url);
  }
});

test("other web hosts go to the system browser", () => {
  for (const url of [
    "https://github.com/JoaoPucci/ani-gui/releases",
    "http://example.com/",
    "https://localhost/",
    "http://localhost/",
  ]) {
    assert.strictEqual(navigationVerdict(url, PACKAGED), "external", url);
  }
});

test("anything that is not a string is refused", () => {
  for (const v of [undefined, null, 42, {}, []]) {
    assert.strictEqual(navigationVerdict(v, PACKAGED), "deny");
    assert.strictEqual(windowOpenVerdict(v, PACKAGED), "deny");
    assert.strictEqual(isAppDocument(v, PACKAGED), false);
  }
});

// ─── New windows ───────────────────────────────────────────────────

test("a new window never opens: the app's own routes are denied, web links go out", () => {
  assert.strictEqual(windowOpenVerdict("app://localhost/settings", PACKAGED), "deny");
  assert.strictEqual(windowOpenVerdict("http://localhost:5173/settings", DEV), "deny");
  assert.strictEqual(windowOpenVerdict("https://syncplay.pl/download/", PACKAGED), "external");
  assert.strictEqual(windowOpenVerdict("javascript:alert(1)", PACKAGED), "deny");
  assert.strictEqual(windowOpenVerdict("file:///etc/passwd", PACKAGED), "deny");
});

// ─── Properties ────────────────────────────────────────────────────

const pathArb = fc
  .array(fc.webSegment(), { maxLength: 5 })
  .map((segs) => `/${segs.join("/")}`);
const tailArb = fc.tuple(pathArb, fc.webQueryParameters(), fc.webFragments()).map(
  ([p, q, h]) => `${p}${q ? `?${q}` : ""}${h ? `#${h}` : ""}`,
);
const OTHER_SCHEMES = [
  "file",
  "data",
  "about",
  "blob",
  "javascript",
  "chrome",
  "devtools",
  "ftp",
  "ws",
  "wss",
  "filesystem",
  "mailto",
  "vbscript",
  "ani-gui",
  "apps",
  "ap",
];

test("property: every path, query and fragment on the app's document is allowed", () => {
  fc.assert(
    fc.property(tailArb, (tail) => {
      assert.strictEqual(navigationVerdict(`app://localhost${tail}`, PACKAGED), "allow");
      assert.strictEqual(navigationVerdict(`http://localhost:5173${tail}`, DEV), "allow");
    }),
  );
});

test("property: no other scheme is ever the app, whatever follows it", () => {
  fc.assert(
    fc.property(
      fc.constantFrom(...OTHER_SCHEMES),
      fc.constantFrom("", "/", "//", "///", "//localhost", "//localhost/"),
      tailArb,
      (scheme, sep, tail) => {
        const url = `${scheme}:${sep}${tail}`;
        assert.strictEqual(navigationVerdict(url, PACKAGED), "deny", url);
        assert.strictEqual(navigationVerdict(url, DEV), "deny", url);
      },
    ),
  );
});

test("property: no other host on the app scheme is the app", () => {
  fc.assert(
    fc.property(
      fc.domain().filter((h) => h !== "localhost"),
      tailArb,
      (host, tail) => {
        const url = `app://${host}${tail}`;
        assert.strictEqual(navigationVerdict(url, PACKAGED), "deny", url);
      },
    ),
  );
});

test("property: a web URL is either the dev server itself or goes out — never both, never silently allowed", () => {
  fc.assert(
    fc.property(fc.webUrl({ withQueryParameters: true, withFragments: true }), (url) => {
      const packaged = navigationVerdict(url, PACKAGED);
      assert.strictEqual(packaged, "external", url);
      const dev = navigationVerdict(url, DEV);
      const u = new URL(url);
      const same = u.protocol === "http:" && u.hostname === "localhost" && u.port === "5173" && !u.username && !u.password;
      assert.strictEqual(dev, same ? "allow" : "external", url);
    }),
  );
});

test("property: the verdict never throws and is always one of the three answers", () => {
  fc.assert(
    fc.property(fc.oneof(fc.string(), fc.fullUnicodeString(), fc.webUrl()), (s) => {
      for (const app of [PACKAGED, DEV]) {
        assert.ok(["allow", "external", "deny"].includes(navigationVerdict(s, app)));
        assert.ok(["external", "deny"].includes(windowOpenVerdict(s, app)));
      }
    }),
  );
});

test("property: whatever is allowed has the app's protocol and host — never just a matching origin string", () => {
  fc.assert(
    fc.property(fc.oneof(fc.string(), fc.webUrl(), tailArb.map((t) => `app://localhost${t}`)), (s) => {
      if (navigationVerdict(s, PACKAGED) !== "allow") return;
      const u = new URL(s);
      assert.strictEqual(u.protocol, "app:");
      assert.strictEqual(u.hostname, "localhost");
    }),
  );
});
