// The renderer's content security policy, as the main process writes it
// for each launch.
//
// Two policies apply to the app's document and a resource must pass
// both. The bundle's own `<meta>` (SvelteKit's `kit.csp`) carries
// `script-src 'self'` and the hash of the inline bootstrap, which only
// the build knows. This module writes the other: every fetch directive
// but the script ones, naming the backend's loopback lanes, whose ports
// the kernel assigns at every start. It never names a script source,
// so it can neither widen nor narrow what the meta allows to run.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const {
  loopbackOrigins,
  rendererPolicy,
  devOriginFrom,
  withPolicyHeader,
  policedResponse,
} = require("./renderer-csp.cjs");

const HANDSHAKE = Object.freeze({
  apiBase: "http://127.0.0.1:41001",
  internalSecret: "s",
  lanes: Object.freeze({
    stream: "http://127.0.0.1:41002",
    image: "http://127.0.0.1:41003",
    probe: "http://127.0.0.1:41004",
    resolve: "http://127.0.0.1:41005",
    download: "http://127.0.0.1:41006",
  }),
});
const SIX = [
  "http://127.0.0.1:41001",
  "http://127.0.0.1:41002",
  "http://127.0.0.1:41003",
  "http://127.0.0.1:41004",
  "http://127.0.0.1:41005",
  "http://127.0.0.1:41006",
];

/** `{ directive: [sources] }` from a policy string. */
function parse(policy) {
  const out = {};
  for (const part of policy.split(";")) {
    const [name, ...sources] = part.trim().split(/\s+/);
    if (!name) continue;
    assert.equal(out[name], undefined, `directive ${name} is written once`);
    out[name] = sources;
  }
  return out;
}

test("the packaged policy, directive by directive", () => {
  const lanes = SIX.join(" ");
  assert.equal(
    rendererPolicy({ backend: HANDSHAKE, devOrigin: null }),
    [
      "style-src 'self'",
      `img-src 'self' data: ${lanes}`,
      "font-src 'self'",
      `connect-src 'self' ${lanes}`,
      `media-src blob: ${lanes}`,
      "worker-src 'none'",
      "frame-src 'none'",
      "child-src 'none'",
      "object-src 'none'",
      "manifest-src 'none'",
      "base-uri 'none'",
      "form-action 'none'",
      "frame-ancestors 'none'",
    ].join("; "),
  );
});

test("it names no script source: the bundle's meta owns scripts", () => {
  const d = parse(rendererPolicy({ backend: HANDSHAKE, devOrigin: null }));
  for (const name of ["script-src", "script-src-elem", "script-src-attr", "default-src"]) {
    assert.equal(d[name], undefined, `${name} would intersect with the meta's script rule`);
  }
});

test("nothing is ever allowed to evaluate strings, in either build", () => {
  for (const devOrigin of [null, "http://localhost:5173"]) {
    const p = rendererPolicy({ backend: HANDSHAKE, devOrigin });
    assert.ok(!p.includes("unsafe-eval"), p);
    assert.ok(!p.includes("wasm-unsafe-eval"), p);
  }
});

test("the packaged policy allows no inline style", () => {
  assert.ok(!rendererPolicy({ backend: HANDSHAKE, devOrigin: null }).includes("unsafe-inline"));
});

test("development adds Vite's injected styles and its HMR socket, and nothing else", () => {
  const packaged = parse(rendererPolicy({ backend: HANDSHAKE, devOrigin: null }));
  const dev = parse(rendererPolicy({ backend: HANDSHAKE, devOrigin: "http://localhost:5173" }));
  assert.deepEqual(dev["style-src"], ["'self'", "'unsafe-inline'"]);
  assert.deepEqual(dev["connect-src"], ["'self'", "ws://localhost:5173", ...SIX]);
  for (const name of Object.keys(packaged)) {
    if (name === "style-src" || name === "connect-src") continue;
    assert.deepEqual(dev[name], packaged[name], name);
  }
  assert.deepEqual(Object.keys(dev).sort(), Object.keys(packaged).sort());
});

test("an https dev server gets a wss socket", () => {
  const dev = parse(rendererPolicy({ backend: HANDSHAKE, devOrigin: "https://dev.example:8443" }));
  assert.ok(dev["connect-src"].includes("wss://dev.example:8443"));
});

test("before the handshake the policy names no loopback origin at all", () => {
  for (const backend of [null, undefined, {}]) {
    const d = parse(rendererPolicy({ backend, devOrigin: null }));
    assert.deepEqual(d["img-src"], ["'self'", "data:"]);
    assert.deepEqual(d["connect-src"], ["'self'"]);
    assert.deepEqual(d["media-src"], ["blob:"]);
  }
});

test("the API and the five known lanes, in a fixed order, each once", () => {
  assert.deepEqual(loopbackOrigins(HANDSHAKE), SIX);
  // The renderer falls back to the API's origin for a lane that was not
  // announced; the policy names what was.
  assert.deepEqual(
    loopbackOrigins({ apiBase: SIX[0], lanes: { image: SIX[2], stream: SIX[0] } }),
    [SIX[0], SIX[2]],
  );
});

test("a lane name the app does not know adds no source", () => {
  // backend-handshake.cjs records any `ANI_GUI_LANE [a-z]+ <origin>`
  // line, and window-args.cjs forwards any lowercase name: an extra
  // announced line must not widen the policy.
  const origins = loopbackOrigins({
    ...HANDSHAKE,
    lanes: { ...HANDSHAKE.lanes, evil: "http://127.0.0.1:9", api: "http://127.0.0.1:8" },
  });
  assert.deepEqual(origins, SIX);
});

test("anything but http://127.0.0.1:<port> is dropped, never widened", () => {
  const bad = [
    "http://127.0.0.1:41002; script-src *",
    "http://127.0.0.1:41002,http://evil.example",
    "http://127.0.0.1:41002 http://evil.example",
    "http://127.0.0.1:41002/",
    "http://127.0.0.1:41002/path",
    "http://127.0.0.1",
    "http://127.0.0.1:0",
    "http://127.0.0.1:65536",
    "http://127.0.0.1:041002",
    "https://127.0.0.1:41002",
    "http://localhost:41002",
    "http://127.0.0.2:41002",
    "http://127.0.0.1:*",
    "http://*:41002",
    "*",
    "http://127.0.0.1:41002@evil.example",
    "HTTP://127.0.0.1:41002",
    "",
    41002,
    null,
  ];
  for (const origin of bad) {
    assert.deepEqual(
      loopbackOrigins({ apiBase: origin, lanes: { stream: origin } }),
      [],
      JSON.stringify(origin),
    );
  }
});

test("property: every source is a keyword, a scheme the app needs, or a valid announced lane", () => {
  const laneNames = fc.constantFrom("api", "stream", "image", "probe", "resolve", "download", "evil", "x");
  const origin = fc.oneof(
    fc.integer({ min: 0, max: 70000 }).map((p) => `http://127.0.0.1:${p}`),
    fc.string(),
    fc.constantFrom("http://127.0.0.1:1;", "http://127.0.0.1:1 *", "'unsafe-eval'", "data:"),
  );
  fc.assert(
    fc.property(
      fc.record({ apiBase: origin, lanes: fc.dictionary(laneNames, origin) }),
      fc.boolean(),
      (backend, dev) => {
        const policy = rendererPolicy({ backend, devOrigin: dev ? "http://localhost:5173" : null });
        const d = parse(policy);
        assert.equal(Object.keys(d).length, 13);
        const allowed = new Set(["'self'", "'none'", "data:", "blob:"]);
        if (dev) allowed.add("'unsafe-inline'").add("ws://localhost:5173");
        const announced = [backend.apiBase, ...Object.entries(backend.lanes)
          .filter(([n]) => ["stream", "image", "probe", "resolve", "download"].includes(n))
          .map(([, o]) => o)];
        for (const sources of Object.values(d)) {
          for (const s of sources) {
            if (allowed.has(s)) continue;
            assert.match(s, /^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/);
            assert.ok(Number(s.split(":")[2]) <= 65535, s);
            assert.ok(announced.includes(s), `${s} was not announced`);
          }
        }
      },
    ),
  );
});

test("the dev origin comes from VITE_DEV_URL, and a malformed one is refused", () => {
  assert.equal(devOriginFrom("http://localhost:5173"), "http://localhost:5173");
  assert.equal(devOriginFrom("http://localhost:5173/some/path?q"), "http://localhost:5173");
  assert.equal(devOriginFrom("http://127.0.0.1:3000/"), "http://127.0.0.1:3000");
  assert.equal(devOriginFrom("https://dev.example"), "https://dev.example");
  for (const bad of ["http://a;b:5173", "http://a,b:5173", "file:///x", "app://localhost", "nope", "", null]) {
    assert.equal(devOriginFrom(bad), null, String(bad));
  }
});

test("a refused dev origin adds nothing to the policy", () => {
  assert.equal(
    rendererPolicy({ backend: HANDSHAKE, devOrigin: null }),
    rendererPolicy({ backend: HANDSHAKE, devOrigin: devOriginFrom("http://a;b:1") }),
  );
});

test("dev: the policy is appended to the Vite page's own, whatever the header's case", () => {
  const details = {
    url: "http://localhost:5173/anime/1",
    resourceType: "mainFrame",
    responseHeaders: { "content-security-policy": ["script-src 'self' 'sha256-x'"], "Content-Type": ["text/html"] },
  };
  const out = withPolicyHeader(details, { devOrigin: "http://localhost:5173", policy: "img-src 'self'" });
  assert.deepEqual(out.responseHeaders["content-security-policy"], [
    "script-src 'self' 'sha256-x'",
    "img-src 'self'",
  ]);
  assert.deepEqual(out.responseHeaders["Content-Type"], ["text/html"]);

  const upper = withPolicyHeader(
    { ...details, responseHeaders: { "Content-Security-Policy": ["a"] } },
    { devOrigin: "http://localhost:5173", policy: "b" },
  );
  assert.deepEqual(upper.responseHeaders, { "Content-Security-Policy": ["a", "b"] });

  const none = withPolicyHeader(
    { ...details, responseHeaders: {} },
    { devOrigin: "http://localhost:5173", policy: "b" },
  );
  assert.deepEqual(none.responseHeaders, { "Content-Security-Policy": ["b"] });
});

test("dev: only the Vite server's main frame is touched", () => {
  const base = { responseHeaders: { "x-a": ["1"] } };
  const opts = { devOrigin: "http://localhost:5173", policy: "b" };
  for (const details of [
    { ...base, url: "http://localhost:5173/_app/x.js", resourceType: "script" },
    { ...base, url: "http://localhost:5173/", resourceType: "subFrame" },
    { ...base, url: "http://127.0.0.1:41001/api/x", resourceType: "mainFrame" },
    { ...base, url: "http://localhost:5174/", resourceType: "mainFrame" },
    { ...base, url: "https://localhost:5173/", resourceType: "mainFrame" },
    { ...base, url: "devtools://devtools/bundled/x.html", resourceType: "mainFrame" },
    { ...base, url: "not a url", resourceType: "mainFrame" },
  ]) {
    assert.deepEqual(withPolicyHeader(details, opts), {}, details.url + " " + details.resourceType);
  }
  assert.deepEqual(withPolicyHeader({ ...base, url: "http://localhost:5173/", resourceType: "mainFrame" }, { devOrigin: null, policy: "b" }), {});
});

test("packaged: the served response keeps its body, status and content type and gains the policy", async () => {
  const served = new Response("<!doctype html>", {
    status: 200,
    statusText: "OK",
    headers: { "content-type": "text/html", "content-length": "15" },
  });
  const out = policedResponse(served, "img-src 'self'");
  assert.equal(out.status, 200);
  assert.equal(out.statusText, "OK");
  assert.equal(out.headers.get("content-type"), "text/html");
  assert.equal(out.headers.get("content-security-policy"), "img-src 'self'");
  assert.equal(await out.text(), "<!doctype html>");
});

test("packaged: a module script keeps its JavaScript type, or Chromium refuses to run it", () => {
  const js = new Response("export {}", { headers: { "Content-Type": "text/javascript" } });
  assert.equal(policedResponse(js, "p").headers.get("content-type"), "text/javascript");
});

test("packaged: a policy the served file already carried is replaced, not kept beside ours", () => {
  const res = new Response("x", { headers: { "content-security-policy": "default-src *" } });
  assert.equal(policedResponse(res, "p").headers.get("content-security-policy"), "p");
});

test("packaged: error answers carry the policy too", () => {
  const out = policedResponse(new Response("not found", { status: 404 }), "p");
  assert.equal(out.status, 404);
  assert.equal(out.headers.get("content-security-policy"), "p");
});
