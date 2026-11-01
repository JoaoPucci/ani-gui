"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fc = require("fast-check");

const { appDocumentFrom, isAppDocument } = require("./app-document.cjs");
const { ALLOWED_PERMISSIONS, guardPermissions, permissionVerdict } = require("./permissions.cjs");

const APP = appDocumentFrom({ isDev: false });
const DEV = appDocumentFrom({ isDev: true, devUrl: "http://localhost:5173" });

test("the app's document keeps fullscreen and clipboard writes", () => {
  // Fullscreen: the player's fullscreen button. Clipboard write: the
  // About page's copy-address button.
  assert.deepStrictEqual([...ALLOWED_PERMISSIONS].sort(), ["clipboard-sanitized-write", "fullscreen"]);
  for (const permission of ALLOWED_PERMISSIONS) {
    assert.strictEqual(permissionVerdict(permission, "app://localhost/play/1", APP), true);
    assert.strictEqual(permissionVerdict(permission, "app://localhost", APP), true);
    assert.strictEqual(permissionVerdict(permission, "http://localhost:5173/about", DEV), true);
  }
});

test("everything else is denied, to the app as to anyone", () => {
  for (const permission of [
    "notifications",
    "geolocation",
    "media",
    "midi",
    "midiSysex",
    "pointerLock",
    "openExternal",
    "clipboard-read",
    "display-capture",
    "idle-detection",
    "window-management",
    "hid",
    "serial",
    "usb",
    "storage-access",
    "fileSystem",
    "unknown",
    "",
  ]) {
    assert.strictEqual(permissionVerdict(permission, "app://localhost/", APP), false, permission);
  }
});

test("another document gets nothing, not even fullscreen", () => {
  for (const url of [
    "about:blank",
    "file:///tmp/dropped.html",
    "file:///C:/Users/me/dropped.html",
    "data:text/html,x",
    "https://example.com/",
    "app://elsewhere/",
    "",
    undefined,
    null,
  ]) {
    for (const permission of ALLOWED_PERMISSIONS) {
      assert.strictEqual(permissionVerdict(permission, url, APP), false, `${permission} ${url}`);
    }
  }
});

test("property: granted exactly when the permission is allowed and the requester is the app's document", () => {
  fc.assert(
    fc.property(
      fc.oneof(fc.constantFrom(...ALLOWED_PERMISSIONS, "notifications", "media", "geolocation"), fc.string()),
      fc.oneof(fc.webUrl(), fc.string(), fc.constantFrom("app://localhost/", "app://localhost/about", "about:blank")),
      (permission, url) => {
        assert.strictEqual(
          permissionVerdict(permission, url, APP),
          ALLOWED_PERMISSIONS.has(permission) && isAppDocument(url, APP),
        );
      },
    ),
  );
});

// ─── Installed on a session ────────────────────────────────────────

function fakeSession() {
  return {
    request: null,
    check: null,
    setPermissionRequestHandler(fn) {
      this.request = fn;
    },
    setPermissionCheckHandler(fn) {
      this.check = fn;
    },
  };
}

test("requests are answered by the verdict, from the requesting frame's URL", () => {
  const session = fakeSession();
  guardPermissions(session, { app: APP });
  const answer = (permission, details) => {
    let granted;
    session.request({}, permission, (v) => {
      granted = v;
    }, details);
    return granted;
  };
  assert.strictEqual(answer("fullscreen", { requestingUrl: "app://localhost/play/1" }), true);
  assert.strictEqual(answer("notifications", { requestingUrl: "app://localhost/" }), false);
  assert.strictEqual(answer("fullscreen", { requestingUrl: "https://example.com/" }), false);
  assert.strictEqual(answer("fullscreen", {}), false);
  assert.strictEqual(answer("fullscreen", undefined), false);
});

test("checks are answered by the verdict, falling back to the requesting origin", () => {
  const session = fakeSession();
  guardPermissions(session, { app: APP });
  assert.strictEqual(
    session.check({}, "clipboard-sanitized-write", "app://localhost", { requestingUrl: "app://localhost/about" }),
    true,
  );
  assert.strictEqual(session.check({}, "clipboard-sanitized-write", "app://localhost", {}), true);
  assert.strictEqual(session.check({}, "notifications", "app://localhost", {}), false);
  assert.strictEqual(session.check({}, "fullscreen", "file://", { requestingUrl: "file:///x.html" }), false);
  assert.strictEqual(session.check({}, "fullscreen", undefined, undefined), false);
});
