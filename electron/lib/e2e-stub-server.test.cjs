// The e2e specs' fake backend answers over real HTTP, as the backend
// does: through Chromium's network stack, with the backend's gate in
// front of the API and its CORS rules on every answer. Playwright's own
// way of answering a request in place loses the answer's status and
// headers once the main process registers a webRequest listener, so
// the specs send each stubbed request here instead.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const {
  QUERY_SECRET_PATHS,
  SECRET_HEADER,
  corsHeaders,
  gateAdmits,
  isRendererOrigin,
  startStubServer,
} = require("./e2e-stub-server.cjs");

const SECRET = "ab".repeat(32);

test("the gate's names are the backend's", () => {
  assert.equal(SECRET_HEADER, "x-ani-gui-internal-secret");
  assert.deepEqual([...QUERY_SECRET_PATHS].sort(), [
    "/api/download/stream",
    "/api/image",
    "/api/play/stream",
  ]);
});

test("the gate admits the secret as the header on any API route", () => {
  for (const [method, path] of [
    ["GET", "/api/settings"],
    ["POST", "/api/sessions"],
    ["GET", "/api/image"],
  ]) {
    assert.equal(
      gateAdmits({ method, path, query: "", headers: { [SECRET_HEADER]: SECRET }, secret: SECRET }),
      true,
      path,
    );
  }
});

test("the gate admits the secret in the query on a GET of the three header-less routes only", () => {
  const query = `url=x&secret=${SECRET}`;
  for (const path of QUERY_SECRET_PATHS) {
    assert.equal(gateAdmits({ method: "GET", path, query, headers: {}, secret: SECRET }), true, path);
    assert.equal(gateAdmits({ method: "POST", path, query, headers: {}, secret: SECRET }), false, path);
    assert.equal(
      gateAdmits({ method: "GET", path: `${path}/x`, query, headers: {}, secret: SECRET }),
      false,
      path,
    );
  }
  assert.equal(
    gateAdmits({ method: "GET", path: "/api/settings", query, headers: {}, secret: SECRET }),
    false,
  );
});

test("the gate refuses a missing or wrong secret, and refuses everything when the launch's secret is unknown", () => {
  for (const headers of [{}, { [SECRET_HEADER]: "00" }, { [SECRET_HEADER]: "" }]) {
    assert.equal(
      gateAdmits({ method: "GET", path: "/api/settings", query: "", headers, secret: SECRET }),
      false,
    );
  }
  for (const secret of [null, undefined, ""]) {
    assert.equal(
      gateAdmits({ method: "GET", path: "/api/settings", query: "", headers: { [SECRET_HEADER]: "" }, secret }),
      false,
    );
  }
});

test("only the secret itself is admitted", () => {
  fc.assert(
    fc.property(fc.string(), fc.constantFrom(...QUERY_SECRET_PATHS, "/api/history"), (guess, path) => {
      fc.pre(guess !== SECRET);
      const query = new URLSearchParams({ secret: guess }).toString();
      assert.equal(
        gateAdmits({ method: "GET", path, query, headers: { [SECRET_HEADER]: guess }, secret: SECRET }),
        false,
      );
    }),
  );
});

test("the renderer's origins are the backend's: the app's scheme and loopback dev servers", () => {
  for (const origin of [
    "app://localhost",
    "http://localhost",
    "http://localhost:5173",
    "http://127.0.0.1:1",
    "http://[::1]:65535",
  ]) {
    assert.equal(isRendererOrigin(origin), true, origin);
  }
  for (const origin of [
    "null",
    "",
    undefined,
    "https://localhost",
    "app://evil",
    "http://localhost:65536",
    "http://localhost:",
    "http://localhost.evil",
    "http://127.0.0.1:123456",
    "file://",
  ]) {
    assert.equal(isRendererOrigin(origin), false, String(origin));
  }
});

test("CORS answers a renderer origin and no other", () => {
  assert.deepEqual(corsHeaders("app://localhost"), {
    "access-control-allow-origin": "app://localhost",
    "access-control-expose-headers": "*",
    vary: "origin",
  });
  assert.deepEqual(corsHeaders("https://example.com"), { vary: "origin" });
  assert.deepEqual(corsHeaders(undefined), { vary: "origin" });
});

async function withServer(fn) {
  const server = await startStubServer();
  try {
    await fn(server);
  } finally {
    await server.close();
  }
}

test("a held answer is served once, at its URL, with its status, headers and body", async () => {
  await withServer(async (server) => {
    const url = server.hold(
      { status: 201, headers: { "x-one": "1" }, contentType: "text/plain", body: "hi" },
      { path: "/s/abc/subtitles", query: "", secret: null },
    );
    assert.ok(url.startsWith(`${server.origin}/`), url);
    const res = await fetch(url, { headers: { origin: "app://localhost" } });
    assert.equal(res.status, 201);
    assert.equal(res.headers.get("x-one"), "1");
    assert.equal(res.headers.get("content-type"), "text/plain");
    assert.equal(res.headers.get("access-control-allow-origin"), "app://localhost");
    assert.equal(await res.text(), "hi");
    const again = await fetch(url);
    assert.equal(again.status, 404);
  });
});

test("an answer's defaults are Playwright's: status 200, and a JSON body when given json", async () => {
  await withServer(async (server) => {
    const json = await fetch(server.hold({ json: { a: [1] } }, { path: "/x", query: "", secret: null }));
    assert.equal(json.status, 200);
    assert.equal(json.headers.get("content-type"), "application/json");
    assert.deepEqual(await json.json(), { a: [1] });
    const bytes = Buffer.from([0x89, 0x50, 0x4e, 0x47]);
    const png = await fetch(
      server.hold({ contentType: "image/png", body: bytes }, { path: "/x", query: "", secret: null }),
    );
    assert.deepEqual(Buffer.from(await png.arrayBuffer()), bytes);
    const empty = await fetch(server.hold({ status: 503 }, { path: "/x", query: "", secret: null }));
    assert.equal(empty.status, 503);
    assert.equal(await empty.text(), "");
  });
});

test("an API answer goes through the backend's gate: a request without the secret gets an empty 403, with CORS", async () => {
  await withServer(async (server) => {
    const held = () =>
      server.hold({ json: { ok: true } }, { path: "/api/settings", query: "", secret: SECRET });
    const refused = await fetch(held(), { headers: { origin: "app://localhost" } });
    assert.equal(refused.status, 403);
    assert.equal(await refused.text(), "");
    assert.equal(refused.headers.get("access-control-allow-origin"), "app://localhost");
    const wrong = await fetch(held(), { headers: { [SECRET_HEADER]: "00" } });
    assert.equal(wrong.status, 403);
    const admitted = await fetch(held(), { headers: { [SECRET_HEADER]: SECRET } });
    assert.equal(admitted.status, 200);
    assert.deepEqual(await admitted.json(), { ok: true });
    const image = await fetch(
      server.hold({ status: 503 }, { path: "/api/image", query: `url=x&secret=${SECRET}`, secret: SECRET }),
    );
    assert.equal(image.status, 503);
  });
});

test("a preflight is answered as the backend's CORS layer answers it, and leaves the answer held", async () => {
  await withServer(async (server) => {
    const url = server.hold({ json: 1 }, { path: "/api/settings", query: "", secret: SECRET });
    const pre = await fetch(url, {
      method: "OPTIONS",
      headers: {
        origin: "app://localhost",
        "access-control-request-method": "GET",
        "access-control-request-headers": SECRET_HEADER,
      },
    });
    assert.equal(pre.status, 200);
    assert.equal(pre.headers.get("access-control-allow-origin"), "app://localhost");
    assert.equal(pre.headers.get("access-control-allow-methods"), "*");
    assert.equal(pre.headers.get("access-control-allow-headers"), "*");
    assert.equal(pre.headers.get("access-control-max-age"), "7200");
    const foreign = await fetch(url, {
      method: "OPTIONS",
      headers: { origin: "https://example.com", "access-control-request-method": "GET" },
    });
    assert.equal(foreign.headers.get("access-control-allow-origin"), null);
    const res = await fetch(url, { headers: { [SECRET_HEADER]: SECRET } });
    assert.equal(res.status, 200);
  });
});

test("a URL nothing was held at is a 404, and the server's URLs are its own", async () => {
  await withServer(async (server) => {
    assert.equal((await fetch(`${server.origin}/nothing`)).status, 404);
    const a = server.hold({}, { path: "/x", query: "", secret: null });
    const b = server.hold({}, { path: "/x", query: "", secret: null });
    assert.notEqual(a, b);
  });
});
