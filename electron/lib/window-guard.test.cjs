"use strict";

const test = require("node:test");
const assert = require("node:assert");
const { EventEmitter } = require("node:events");

const { appDocumentFrom } = require("./app-document.cjs");
const { guardWindow } = require("./window-guard.cjs");

const APP = appDocumentFrom({ isDev: false });

/** A webContents stand-in: the events the guard listens to, and the open handler. */
function fakeContents() {
  const contents = new EventEmitter();
  contents.openHandler = null;
  contents.setWindowOpenHandler = (fn) => {
    contents.openHandler = fn;
  };
  return contents;
}

function fakeEvent(fields = {}) {
  return {
    prevented: false,
    preventDefault() {
      this.prevented = true;
    },
    ...fields,
  };
}

function setup() {
  const contents = fakeContents();
  const opened = [];
  const errors = [];
  let reject = null;
  guardWindow(contents, {
    app: APP,
    openExternal: (url) => {
      opened.push(url);
      return reject ? Promise.reject(reject) : Promise.resolve();
    },
    logError: (...args) => errors.push(args.join(" ")),
  });
  return {
    contents,
    opened,
    errors,
    failOpens(err) {
      reject = err;
    },
  };
}

// ─── will-navigate ─────────────────────────────────────────────────

test("will-navigate: the app's own document navigates", () => {
  const { contents, opened } = setup();
  const e = fakeEvent({ url: "app://localhost/settings" });
  contents.emit("will-navigate", e, e.url);
  assert.strictEqual(e.prevented, false);
  assert.deepStrictEqual(opened, []);
});

test("will-navigate: a dropped file, a data: document and about:blank are refused", () => {
  const { contents, opened } = setup();
  for (const url of [
    "file:///tmp/dropped.html",
    "file:///C:/Users/me/dropped.html",
    "data:text/html,<p>x</p>",
    "about:blank",
    "app://evil/",
  ]) {
    const e = fakeEvent({ url });
    contents.emit("will-navigate", e, url);
    assert.strictEqual(e.prevented, true, url);
  }
  assert.deepStrictEqual(opened, []);
});

test("will-navigate: a web link is refused here and opened in the system browser", () => {
  const { contents, opened } = setup();
  const e = fakeEvent({ url: "https://github.com/JoaoPucci/ani-gui" });
  contents.emit("will-navigate", e, e.url);
  assert.strictEqual(e.prevented, true);
  assert.deepStrictEqual(opened, ["https://github.com/JoaoPucci/ani-gui"]);
});

test("will-navigate: a failed external open is logged, never left unhandled", async () => {
  const { contents, errors, failOpens } = setup();
  failOpens(new Error("no default browser"));
  const e = fakeEvent({ url: "https://example.com/" });
  contents.emit("will-navigate", e, e.url);
  await new Promise((r) => setImmediate(r));
  assert.strictEqual(e.prevented, true);
  assert.ok(errors.some((l) => l.includes("no default browser")), errors.join("\n"));
});

test("will-navigate: reads the URL off the event's own details when present", () => {
  const { contents } = setup();
  // Electron passes the URL both as the event's `url` and as the
  // deprecated second argument; the details object is authoritative.
  const e = fakeEvent({ url: "file:///etc/hostname" });
  contents.emit("will-navigate", e, "app://localhost/");
  assert.strictEqual(e.prevented, true);
});

// ─── will-redirect ─────────────────────────────────────────────────

test("will-redirect: only the app's document; a redirect out is refused, not opened", () => {
  const { contents, opened } = setup();
  const ok = fakeEvent({ url: "app://localhost/" });
  contents.emit("will-redirect", ok, ok.url);
  assert.strictEqual(ok.prevented, false);
  for (const url of ["https://example.com/", "file:///etc/hostname", "data:,x"]) {
    const e = fakeEvent({ url });
    contents.emit("will-redirect", e, url);
    assert.strictEqual(e.prevented, true, url);
  }
  assert.deepStrictEqual(opened, []);
});

// ─── will-frame-navigate ───────────────────────────────────────────

test("will-frame-navigate: a subframe leaving the app is refused, not opened", () => {
  const { contents, opened } = setup();
  for (const url of ["https://example.com/", "file:///etc/hostname", "about:blank"]) {
    const e = fakeEvent({ url, isMainFrame: false });
    contents.emit("will-frame-navigate", e);
    assert.strictEqual(e.prevented, true, url);
  }
  const inApp = fakeEvent({ url: "app://localhost/x", isMainFrame: false });
  contents.emit("will-frame-navigate", inApp);
  assert.strictEqual(inApp.prevented, false);
  assert.deepStrictEqual(opened, []);
});

test("will-frame-navigate: the main frame is left to will-navigate, so a link opens once", () => {
  const { contents, opened } = setup();
  const e = fakeEvent({ url: "https://example.com/", isMainFrame: true });
  contents.emit("will-frame-navigate", e);
  contents.emit("will-navigate", e, e.url);
  assert.deepStrictEqual(opened, ["https://example.com/"]);
});

// ─── webviews and new windows ──────────────────────────────────────

test("will-attach-webview is always refused", () => {
  const { contents } = setup();
  const e = fakeEvent();
  contents.emit("will-attach-webview", e, {}, {});
  assert.strictEqual(e.prevented, true);
});

test("new windows: denied always; web links go to the system browser, with the failure caught", async () => {
  const { contents, opened, errors, failOpens } = setup();
  assert.deepStrictEqual(contents.openHandler({ url: "app://localhost/settings" }), { action: "deny" });
  assert.deepStrictEqual(contents.openHandler({ url: "file:///etc/hostname" }), { action: "deny" });
  assert.deepStrictEqual(contents.openHandler({ url: "javascript:alert(1)" }), { action: "deny" });
  assert.deepStrictEqual(opened, []);
  failOpens(new Error("xdg-open missing"));
  assert.deepStrictEqual(contents.openHandler({ url: "https://syncplay.pl/download/" }), {
    action: "deny",
  });
  assert.deepStrictEqual(opened, ["https://syncplay.pl/download/"]);
  await new Promise((r) => setImmediate(r));
  assert.ok(errors.some((l) => l.includes("xdg-open missing")), errors.join("\n"));
});
