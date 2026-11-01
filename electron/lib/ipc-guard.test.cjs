"use strict";

const test = require("node:test");
const assert = require("node:assert");
const { EventEmitter } = require("node:events");
const fc = require("fast-check");

const { appDocumentFrom, isAppDocument } = require("./app-document.cjs");
const { guardedIpc, isTrustedSender } = require("./ipc-guard.cjs");

const APP = appDocumentFrom({ isDev: false });
const DEV = appDocumentFrom({ isDev: true, devUrl: "http://localhost:5173" });

const mainFrame = (url) => ({ url, parent: null });
const subFrame = (url) => ({ url, parent: mainFrame("app://localhost/") });

/** ipcMain's two registration shapes, recorded. */
function fakeIpcMain() {
  const ipc = new EventEmitter();
  ipc.handlers = new Map();
  ipc.handle = (channel, fn) => {
    ipc.handlers.set(channel, fn);
  };
  return ipc;
}

function setup() {
  const ipc = fakeIpcMain();
  const errors = [];
  const guarded = guardedIpc(ipc, { app: APP, logError: (...a) => errors.push(a.join(" ")) });
  return { ipc, guarded, errors };
}

// ─── Who is trusted ────────────────────────────────────────────────

test("the app's main frame is a trusted sender", () => {
  assert.strictEqual(isTrustedSender(mainFrame("app://localhost/"), APP), true);
  assert.strictEqual(isTrustedSender(mainFrame("app://localhost/play/1?ep=2"), APP), true);
  assert.strictEqual(isTrustedSender(mainFrame("http://localhost:5173/settings"), DEV), true);
});

test("anything else is not: another document, a subframe, no frame", () => {
  for (const frame of [
    mainFrame("about:blank"),
    mainFrame("file:///tmp/dropped.html"),
    mainFrame("file:///C:/Users/me/dropped.html"),
    mainFrame("data:text/html,x"),
    mainFrame("https://example.com/"),
    mainFrame("app://elsewhere/"),
    mainFrame(""),
    subFrame("app://localhost/"),
    null,
    undefined,
    {},
  ]) {
    assert.strictEqual(isTrustedSender(frame, APP), false, JSON.stringify(frame));
  }
});

test("a frame that throws when read is not trusted", () => {
  const frame = {
    get url() {
      throw new Error("Render frame was disposed before WebFrameMain could be accessed");
    },
    parent: null,
  };
  assert.strictEqual(isTrustedSender(frame, APP), false);
});

test("property: a sender is trusted exactly when it is a main frame holding the app's document", () => {
  const urlArb = fc.oneof(
    fc.webUrl(),
    fc.string(),
    fc.constantFrom("app://localhost/", "app://localhost/x", "about:blank", "file:///x", "app://evil/"),
  );
  fc.assert(
    fc.property(urlArb, fc.boolean(), (url, isMain) => {
      const frame = isMain ? mainFrame(url) : subFrame(url);
      assert.strictEqual(isTrustedSender(frame, APP), isMain && isAppDocument(url, APP));
    }),
  );
});

// ─── The wrapper ───────────────────────────────────────────────────

test("handle: a trusted call reaches the handler with its arguments", async () => {
  const { ipc, guarded } = setup();
  guarded.handle("ch", (_e, a, b) => a + b);
  const result = await ipc.handlers.get("ch")({ senderFrame: mainFrame("app://localhost/") }, 2, 3);
  assert.strictEqual(result, 5);
});

test("handle: an untrusted call is refused before the handler runs, and logged", async () => {
  const { ipc, guarded, errors } = setup();
  let ran = false;
  guarded.handle("ani-gui:reveal-in-folder", () => {
    ran = true;
  });
  await assert.rejects(
    async () => ipc.handlers.get("ani-gui:reveal-in-folder")({ senderFrame: mainFrame("file:///tmp/x.html") }, "/"),
    /refused/,
  );
  assert.strictEqual(ran, false);
  assert.ok(errors.some((l) => l.includes("ani-gui:reveal-in-folder") && l.includes("file:///tmp/x.html")), errors.join("\n"));
});

test("on: an untrusted message is dropped", () => {
  const { ipc, guarded, errors } = setup();
  let ran = false;
  guarded.on("ani-gui:active-downloads", () => {
    ran = true;
  });
  ipc.emit("ani-gui:active-downloads", { senderFrame: mainFrame("about:blank") }, 3);
  assert.strictEqual(ran, false);
  assert.ok(errors.some((l) => l.includes("ani-gui:active-downloads")));
  ipc.emit("ani-gui:active-downloads", { senderFrame: mainFrame("app://localhost/") }, 3);
  assert.strictEqual(ran, true);
});

test("onSync: an untrusted call gets the refusal value, so the renderer never hangs", () => {
  const { ipc, guarded } = setup();
  guarded.onSync(
    "ani-gui:account:get-token",
    (event) => {
      event.returnValue = { ok: true, payload: "secret" };
    },
    { ok: false, kind: "refused" },
  );
  const refused = { senderFrame: mainFrame("https://example.com/") };
  ipc.emit("ani-gui:account:get-token", refused, "anilist");
  assert.deepStrictEqual(refused.returnValue, { ok: false, kind: "refused" });
  const trusted = { senderFrame: mainFrame("app://localhost/") };
  ipc.emit("ani-gui:account:get-token", trusted, "anilist");
  assert.deepStrictEqual(trusted.returnValue, { ok: true, payload: "secret" });
});

test("onSync: a refusal value is required — a sync channel without one is a programming error", () => {
  const { guarded } = setup();
  assert.throws(() => guarded.onSync("ch", () => {}), /refusal value/);
});

test("an event whose senderFrame cannot be read is refused", async () => {
  const { ipc, guarded } = setup();
  let ran = false;
  guarded.handle("ch", () => {
    ran = true;
  });
  const event = {
    get senderFrame() {
      throw new Error("disposed");
    },
  };
  await assert.rejects(async () => ipc.handlers.get("ch")(event));
  assert.strictEqual(ran, false);
});
