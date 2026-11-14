// The routes an <img> or an EventSource loads cannot be sent a header
// from the page, and used to carry the launch's secret as `secret=` in
// their URL. A URL is written down in places a header is not: Chromium's
// disk cache keys every cached response by it, and a log line naming it
// names the secret. So the main process adds the header to those
// requests itself, for the app's own page alone, and no URL carries it.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const { appDocumentFrom } = require("./app-document.cjs");
const { secretKeeper } = require("./secret-handoff.cjs");
const {
  HEADERLESS_PATHS,
  SECRET_HEADER,
  backendOrigins,
  headerlessFilter,
  withLaunchSecret,
} = require("./secret-header.cjs");

const PACKAGED = appDocumentFrom({ isDev: false });
const SECRET = "c0ffee".repeat(10) + "abcd";
const BACKEND = {
  apiBase: "http://127.0.0.1:4000",
  lanes: {
    image: "http://127.0.0.1:4001",
    resolve: "http://127.0.0.1:4002",
    download: "http://127.0.0.1:4003",
    stream: "http://127.0.0.1:4004",
  },
};

const mainFrame = (url) => ({ url, parent: null });
const subFrame = (url) => ({ url, parent: mainFrame("app://localhost/") });

function setup() {
  const k = secretKeeper({ app: PACKAGED, logError: () => {} });
  const contents = {};
  k.hold(contents, SECRET);
  const add = (details) =>
    withLaunchSecret(
      { method: "GET", requestHeaders: { Accept: "image/*" }, ...details },
      { origins: backendOrigins(BACKEND), secretFor: (d) => k.forRequest(d) },
    );
  return { k, contents, add };
}

test("the header-less routes are the image proxy and the two progress streams", () => {
  assert.equal(SECRET_HEADER, "x-ani-gui-internal-secret");
  assert.deepEqual([...HEADERLESS_PATHS].sort(), [
    "/api/download/stream",
    "/api/image",
    "/api/play/stream",
  ]);
});

test("the backend's origins are its API's and every lane's, once each", () => {
  assert.deepEqual(backendOrigins(BACKEND).sort(), [
    "http://127.0.0.1:4000",
    "http://127.0.0.1:4001",
    "http://127.0.0.1:4002",
    "http://127.0.0.1:4003",
    "http://127.0.0.1:4004",
  ]);
  assert.deepEqual(
    backendOrigins({ apiBase: "http://127.0.0.1:4000/", lanes: { image: "http://127.0.0.1:4000", x: "not a url" } }),
    ["http://127.0.0.1:4000"],
  );
  assert.deepEqual(backendOrigins({}), []);
});

test("the request filter names each header-less route on each backend origin, and nothing else", () => {
  const { urls } = headerlessFilter(BACKEND);
  assert.equal(urls.length, 15);
  assert.ok(urls.includes("http://127.0.0.1:4001/api/image*"));
  assert.ok(urls.includes("http://127.0.0.1:4002/api/play/stream*"));
  assert.ok(urls.includes("http://127.0.0.1:4003/api/download/stream*"));
  assert.ok(urls.every((u) => HEADERLESS_PATHS.some((p) => u.endsWith(`${p}*`))));
});

test("the app's own page gets the header on each header-less route, its own headers kept", () => {
  const { contents, add } = setup();
  for (const url of [
    "http://127.0.0.1:4001/api/image?url=https%3A%2F%2Fa%2Fb.jpg",
    "http://127.0.0.1:4002/api/play/stream?title=a&episode=1",
    "http://127.0.0.1:4003/api/download/stream?title=a",
    // A lane the page fell back from reaches the route on the API's origin.
    "http://127.0.0.1:4000/api/image?url=https%3A%2F%2Fa%2Fb.jpg",
  ]) {
    assert.deepEqual(
      add({ url, webContents: contents, frame: mainFrame("app://localhost/anime/1") }),
      { Accept: "image/*", [SECRET_HEADER]: SECRET },
      url,
    );
  }
});

test("a frame that is not the app's page never gets the header", () => {
  const { contents, add } = setup();
  const url = "http://127.0.0.1:4001/api/image?url=x";
  const disposed = {
    get url() {
      throw new Error("Render frame was disposed before WebFrameMain could be accessed");
    },
    parent: null,
  };
  for (const frame of [
    subFrame("app://localhost/"),
    subFrame("about:srcdoc"),
    mainFrame("file:///tmp/x.html"),
    mainFrame("https://example.com/"),
    mainFrame("about:blank"),
    null,
    undefined,
    disposed,
  ]) {
    assert.deepEqual(add({ url, webContents: contents, frame }), { Accept: "image/*" });
  }
  const k = secretKeeper({ app: PACKAGED, logError: () => {} });
  k.hold(contents, SECRET);
  const throwing = {
    url,
    method: "GET",
    requestHeaders: { Accept: "image/*" },
    webContents: contents,
    get frame() {
      throw new Error("disposed");
    },
  };
  assert.deepEqual(
    withLaunchSecret(throwing, {
      origins: backendOrigins(BACKEND),
      secretFor: (d) => k.forRequest(d),
    }),
    { Accept: "image/*" },
  );
});

test("a window the secret was not handed to, or no window, never gets the header", () => {
  const { add } = setup();
  const url = "http://127.0.0.1:4001/api/image?url=x";
  for (const webContents of [{}, null, undefined]) {
    assert.deepEqual(add({ url, webContents, frame: mainFrame("app://localhost/") }), {
      Accept: "image/*",
    });
  }
});

test("any other route, origin or method goes out as it came", () => {
  const { contents, add } = setup();
  const frame = mainFrame("app://localhost/");
  for (const url of [
    "http://127.0.0.1:4001/api/history",
    "http://127.0.0.1:4001/api/imagex?url=x",
    "http://127.0.0.1:4001/api/image/x",
    "http://127.0.0.1:4001/s/abc/api/image",
    "http://127.0.0.1:4999/api/image?url=x",
    "http://localhost:4001/api/image?url=x",
    "https://127.0.0.1:4001/api/image?url=x",
    "https://media.kitsu.app/api/image?url=x",
    "not a url",
  ]) {
    assert.deepEqual(add({ url, webContents: contents, frame }), { Accept: "image/*" }, url);
  }
  assert.deepEqual(
    add({ url: "http://127.0.0.1:4001/api/image?url=x", method: "POST", webContents: contents, frame }),
    { Accept: "image/*" },
  );
});

test("the header is added exactly when the app's page asks a header-less route of the backend", () => {
  const origin = fc.constantFrom(...backendOrigins(BACKEND), "http://127.0.0.1:9", "https://e.example");
  const path = fc.constantFrom(...HEADERLESS_PATHS, "/api/history", "/api/image/x", "/s/a");
  const page = fc.constantFrom("app://localhost/", "app://localhost/x?y", "file:///x", "https://e.example/");
  fc.assert(
    fc.property(origin, path, page, fc.boolean(), fc.boolean(), fc.string(), (o, p, u, isMain, held, q) => {
      const k = secretKeeper({ app: PACKAGED, logError: () => {} });
      const contents = {};
      if (held) k.hold(contents, SECRET);
      const out = withLaunchSecret(
        {
          url: `${o}${p}?q=${encodeURIComponent(q)}`,
          method: "GET",
          requestHeaders: {},
          webContents: contents,
          frame: isMain ? mainFrame(u) : subFrame(u),
        },
        { origins: backendOrigins(BACKEND), secretFor: (d) => k.forRequest(d) },
      );
      const expected =
        held &&
        isMain &&
        u.startsWith("app://localhost/") &&
        backendOrigins(BACKEND).includes(o) &&
        HEADERLESS_PATHS.includes(p);
      assert.deepEqual(out, expected ? { [SECRET_HEADER]: SECRET } : {});
    }),
  );
});
