// The e2e specs' fake backend answers over real HTTP, as the backend
// does: through Chromium's network stack, with the backend's gate in
// front of the API and its CORS rules on every answer. Playwright's own
// way of answering a request in place loses the answer's status and
// headers once the main process registers a webRequest listener, so
// the specs send each stubbed request here instead.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");
const fs = require("node:fs");
const path = require("node:path");

const {
  SECRET_HEADER,
  corsHeaders,
  gateAdmits,
  isRendererOrigin,
  startStubServer,
} = require("./e2e-stub-server.cjs");

const SECRET = "ab".repeat(32);
const stubServer = require("./e2e-stub-server.cjs");
// The routes an <img> or an EventSource loads, which once took the
// secret in their query.
const HEADERLESS = ["/api/image", "/api/play/stream", "/api/download/stream"];

test("the gate's names are the backend's", () => {
  assert.equal(SECRET_HEADER, "x-ani-gui-internal-secret");
  assert.equal(stubServer.QUERY_SECRET_PATHS, undefined);
});

test("the gate admits the secret as the header on any API route", () => {
  for (const [method, path] of [
    ["GET", "/api/settings"],
    ["POST", "/api/play"],
    ["GET", "/api/image"],
  ]) {
    assert.equal(
      gateAdmits({ method, path, query: "", headers: { [SECRET_HEADER]: SECRET }, secret: SECRET }),
      true,
      path,
    );
  }
});

test("the gate reads no secret from the query, on any route", () => {
  const query = `url=x&secret=${SECRET}`;
  for (const path of [...HEADERLESS, "/api/settings"]) {
    assert.equal(gateAdmits({ method: "GET", path, query, headers: {}, secret: SECRET }), false, path);
  }
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

// Values a careless gate would let through: the secret cut short,
// grown by a character, with one character changed, in another case,
// or padded.
const nearMiss = fc.oneof(
  fc.integer({ min: 0, max: SECRET.length - 1 }).map((n) => SECRET.slice(0, n)),
  fc.constantFrom(..."0123456789abcdef").map((c) => SECRET + c),
  fc
    .tuple(fc.integer({ min: 0, max: SECRET.length - 1 }), fc.constantFrom("0", "f", "g", " "))
    .map(([i, c]) => SECRET.slice(0, i) + c + SECRET.slice(i + 1))
    .filter((v) => v !== SECRET),
  fc.constant(SECRET.toUpperCase()),
  fc.constantFrom(` ${SECRET}`, `${SECRET} `, `${SECRET}\t`),
  fc.string(),
);

test("only the secret itself, as the header, is admitted", () => {
  const method = fc.constantFrom("GET", "POST", "PUT", "DELETE");
  const route = fc.constantFrom(...HEADERLESS, "/api/history", "/api/play");
  fc.assert(
    fc.property(nearMiss, method, route, (guess, m, p) => {
      fc.pre(guess !== SECRET);
      const asQuery = new URLSearchParams({ secret: guess }).toString();
      assert.equal(
        gateAdmits({ method: m, path: p, query: asQuery, headers: { [SECRET_HEADER]: guess }, secret: SECRET }),
        false,
      );
    }),
  );
  fc.assert(
    fc.property(method, route, fc.boolean(), (m, p, inQuery) => {
      const request = inQuery
        ? { query: `other=1&secret=${SECRET}`, headers: {} }
        : { query: "", headers: { [SECRET_HEADER]: SECRET } };
      const expected = !inQuery;
      assert.equal(gateAdmits({ method: m, path: p, secret: SECRET, ...request }), expected);
    }),
  );
  // The secret under another name is no secret.
  assert.equal(
    gateAdmits({ method: "GET", path: "/api/image", query: `token=${SECRET}`, headers: {}, secret: SECRET }),
    false,
  );
});

// The stub server is a copy of the backend's rules, so it is held to
// the backend's own source: the names and values below are read from
// it, verbatim. A gate that reads the secret from anywhere but the
// header goes red here.
test("the gate's copy is the backend's: header only, its header, renderer origin and preflight age", () => {
  const root = path.join(__dirname, "..", "..", "backend", "src");
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  assert.ok(
    read("api/gate.rs").includes(
      "pub fn admits(secret: &InternalSecret, headers: &HeaderMap) -> bool {\n    secret.validate_header(headers).is_ok()\n}",
    ),
    "backend/src/api/gate.rs admits, header only",
  );
  assert.ok(!read("api/gate.rs").includes("SECRET_QUERY"), "backend/src/api/gate.rs SECRET_QUERY");
  assert.ok(
    read("account/internal_secret.rs").includes(`HeaderName::from_static("${SECRET_HEADER}")`),
    "backend/src/account/internal_secret.rs INTERNAL_SECRET_HEADER",
  );
  assert.ok(read("renderer_origin.rs").includes('pub const PACKAGED: &str = "app://localhost";'));
  assert.ok(
    read("renderer_origin.rs").includes(
      "pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);",
    ),
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
  const vary = "origin, access-control-request-method, access-control-request-headers";
  assert.deepEqual(corsHeaders("app://localhost"), {
    "access-control-allow-origin": "app://localhost",
    "access-control-expose-headers": "*",
    vary,
  });
  assert.deepEqual(corsHeaders("https://example.com"), { vary });
  assert.deepEqual(corsHeaders(undefined), { vary });
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
    assert.equal(image.status, 403);
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
    // The backend's layer (tower-http) answers it the same, but names
    // no origin, so a browser refuses the request it precedes.
    assert.equal(foreign.status, 200);
    assert.equal(foreign.headers.get("access-control-allow-origin"), null);
    assert.equal(foreign.headers.get("access-control-allow-methods"), "*");
    assert.equal(foreign.headers.get("access-control-max-age"), "7200");
    // Any OPTIONS is a preflight to that layer, whatever it carries.
    const bare = await fetch(url, { method: "OPTIONS" });
    assert.equal(bare.status, 200);
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

test("a query secret is refused through the server, as the gate refuses it", async () => {
  await withServer(async (server) => {
    for (const method of ["GET", "POST"]) {
      const res = await fetch(
        server.hold({ json: 1 }, { path: "/api/image", query: `secret=${SECRET}`, secret: SECRET }),
        { method },
      );
      assert.equal(res.status, 403, method);
    }
  });
});

test("a stub's own content type and CORS headers give way: contentType names the type, and CORS is the backend's", async () => {
  await withServer(async (server) => {
    const res = await fetch(
      server.hold(
        {
          contentType: "text/plain",
          headers: { "content-type": "image/png", "access-control-allow-origin": "*", "x-keep": "1" },
          body: "x",
        },
        { path: "/s/a/x", query: "", secret: null },
      ),
      { headers: { origin: "https://example.com" } },
    );
    assert.equal(res.headers.get("content-type"), "text/plain");
    assert.equal(res.headers.get("access-control-allow-origin"), null);
    assert.equal(res.headers.get("x-keep"), "1");
  });
});
