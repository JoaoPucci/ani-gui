// How the launch's secret reaches the preload: the main process answers
// it over IPC, privately, and only to the app's own page. It used to go
// on the renderer's command line (`additionalArguments`), where any
// local program that can list processes reads it (/proc/<pid>/cmdline
// on Linux; the process's command line on Windows).

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const { appDocumentFrom, isAppDocument } = require("./app-document.cjs");
const { SECRET_CHANNEL, secretKeeper } = require("./secret-handoff.cjs");

const PACKAGED = appDocumentFrom({ isDev: false });
const DEV = appDocumentFrom({ isDev: true, devUrl: "http://localhost:5173" });
const SECRET = "c0ffee".repeat(10) + "abcd";

const mainFrame = (url) => ({ url, parent: null });
const subFrame = (url) => ({ url, parent: mainFrame("app://localhost/") });

function keeper(app = PACKAGED) {
  const logged = [];
  const k = secretKeeper({ app, logError: (line) => logged.push(line) });
  return { k, logged };
}

test("the channel is the app's own", () => {
  assert.equal(SECRET_CHANNEL, "ani-gui:internal-secret");
});

test("the app's own page, in the window it was made for, gets the secret", () => {
  const { k } = keeper();
  const contents = {};
  k.hold(contents, SECRET);
  for (const url of ["app://localhost/", "app://localhost/settings?x=1#y"]) {
    assert.equal(k.answer({ sender: contents, senderFrame: mainFrame(url) }), SECRET, url);
  }
});

test("in development the Vite page is the app's own", () => {
  const { k } = keeper(DEV);
  const contents = {};
  k.hold(contents, SECRET);
  assert.equal(
    k.answer({ sender: contents, senderFrame: mainFrame("http://localhost:5173/anime/1") }),
    SECRET,
  );
  assert.equal(k.answer({ sender: contents, senderFrame: mainFrame("app://localhost/") }), null);
});

test("any other page in the app's window gets nothing, and the refusal is logged", () => {
  const { k, logged } = keeper();
  const contents = {};
  k.hold(contents, SECRET);
  for (const url of [
    "https://example.com/",
    "http://localhost:5173/",
    "file:///tmp/x.html",
    "data:text/html,hi",
    "about:blank",
    "app://evil/",
    "app://user:pw@localhost/",
    "",
  ]) {
    assert.equal(k.answer({ sender: contents, senderFrame: mainFrame(url) }), null, url);
  }
  assert.ok(logged.length > 0);
  for (const line of logged) assert.ok(!line.includes(SECRET), line);
});

test("a subframe, even one on the app's own document, gets nothing", () => {
  const { k } = keeper();
  const contents = {};
  k.hold(contents, SECRET);
  assert.equal(k.answer({ sender: contents, senderFrame: subFrame("app://localhost/") }), null);
});

test("a window the secret was never handed to (the splash, a stray one) gets nothing", () => {
  const { k } = keeper();
  k.hold({}, SECRET);
  assert.equal(k.answer({ sender: {}, senderFrame: mainFrame("app://localhost/") }), null);
});

test("a frame that is gone, or reads as gone, gets nothing", () => {
  const { k } = keeper();
  const contents = {};
  k.hold(contents, SECRET);
  assert.equal(k.answer({ sender: contents, senderFrame: null }), null);
  const disposed = {
    get url() {
      throw new Error("Render frame was disposed before WebFrameMain could be accessed");
    },
    parent: null,
  };
  assert.equal(k.answer({ sender: contents, senderFrame: disposed }), null);
  const throwing = {
    sender: contents,
    get senderFrame() {
      throw new Error("disposed");
    },
  };
  assert.equal(k.answer(throwing), null);
});

test("a window made with no secret holds none", () => {
  const { k } = keeper();
  const contents = {};
  k.hold(contents, null);
  k.hold(contents, "");
  assert.equal(k.answer({ sender: contents, senderFrame: mainFrame("app://localhost/") }), null);
});

test("the secret goes to a page exactly when it is the app's main frame in a window holding it", () => {
  const url = fc.oneof(
    fc.webUrl(),
    fc.constantFrom(
      "app://localhost/",
      "app://localhost/a?b#c",
      "app://localhost:1/",
      "app://other/",
      "file:///C:/x.html",
      "file://server/share/x",
      "about:blank",
      "data:text/html,x",
    ),
    fc.string(),
  );
  fc.assert(
    fc.property(url, fc.boolean(), fc.boolean(), (u, isMain, held) => {
      const { k } = keeper();
      const contents = {};
      if (held) k.hold(contents, SECRET);
      const frame = isMain ? mainFrame(u) : subFrame(u);
      const got = k.answer({ sender: contents, senderFrame: frame });
      const expected = held && isMain && isAppDocument(u, PACKAGED) ? SECRET : null;
      assert.equal(got, expected);
    }),
  );
});
