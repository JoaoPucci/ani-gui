"use strict";

// Every window the main process makes — the launch splash as well as
// the app's — gets the navigation guard and the permission handlers as
// it is made, before anything loads in it. The splash was once made with
// a bare `new BrowserWindow`, and nothing guarded it.

const test = require("node:test");
const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");

const { appDocumentFrom } = require("./app-document.cjs");
const { guardedWindowMaker } = require("./guarded-window.cjs");
const { splashWindowOptions } = require("./splash.cjs");

const APP = appDocumentFrom({ isDev: false });

/** A BrowserWindow stand-in that records what was installed on it. */
function fakeBrowserWindow() {
  const made = [];
  class FakeWindow {
    constructor(options) {
      this.options = options;
      const contents = new EventEmitter();
      contents.setWindowOpenHandler = (fn) => (contents.openHandler = fn);
      contents.session = {
        setPermissionRequestHandler: (fn) => (contents.session.request = fn),
        setPermissionCheckHandler: (fn) => (contents.session.check = fn),
      };
      this.webContents = contents;
      made.push(this);
    }
  }
  return { FakeWindow, made };
}

function maker() {
  const { FakeWindow, made } = fakeBrowserWindow();
  const opened = [];
  const make = guardedWindowMaker({
    BrowserWindow: FakeWindow,
    app: APP,
    openExternal: (url) => opened.push(url),
    logError: () => {},
  });
  return { make, made, opened };
}

function navigate(win, event, url) {
  let prevented = false;
  win.webContents.emit(event, { url, isMainFrame: false, preventDefault: () => (prevented = true) }, url);
  return prevented;
}

test("the splash's window is made with its options, and guarded before it is handed back", () => {
  const { make, made, opened } = maker();
  const options = splashWindowOptions();
  const win = make(options);
  assert.equal(made.length, 1);
  assert.equal(win, made[0]);
  assert.equal(win.options, options);
  // Navigation: nothing but the app's document, a web page offered to
  // the system browser instead.
  assert.equal(navigate(win, "will-navigate", "file:///tmp/dropped.html"), true);
  assert.equal(navigate(win, "will-navigate", "https://example.com/"), true);
  assert.deepEqual(opened, ["https://example.com/"]);
  assert.equal(navigate(win, "will-redirect", "data:text/html,x"), true);
  assert.equal(navigate(win, "will-frame-navigate", "file:///tmp/x.html"), true);
  assert.equal(navigate(win, "will-attach-webview", "app://localhost/"), true);
  assert.deepEqual(win.webContents.openHandler({ url: "app://localhost/" }), { action: "deny" });
  // Permissions: the splash's document is not the app's, so it gets none.
  const session = win.webContents.session;
  let granted = null;
  session.request(win.webContents, "fullscreen", (v) => (granted = v), {
    requestingUrl: "file:///opt/ani-gui/splash/index.html",
  });
  assert.equal(granted, false);
  assert.equal(
    session.check(win.webContents, "clipboard-sanitized-write", "file://", {
      requestingUrl: "file:///opt/ani-gui/splash/index.html",
    }),
    false,
  );
});

test("the app's window is guarded the same way, and its document keeps what it uses", () => {
  const { make } = maker();
  const win = make({ show: false });
  assert.equal(navigate(win, "will-navigate", "app://localhost/settings"), false);
  assert.equal(navigate(win, "will-navigate", "file:///tmp/dropped.html"), true);
  let granted = null;
  win.webContents.session.request(win.webContents, "fullscreen", (v) => (granted = v), {
    requestingUrl: "app://localhost/play/1",
  });
  assert.equal(granted, true);
});
