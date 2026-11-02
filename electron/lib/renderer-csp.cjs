"use strict";

// The renderer's content security policy — the half the main process
// writes for each launch.
//
// Two policies apply to the app's document, and a resource has to pass
// both:
//
//   - The bundle's own, a `<meta http-equiv>` SvelteKit writes into the
//     page at build time (`kit.csp` in frontend/svelte.config.js):
//     `script-src 'self'` plus the hash of SvelteKit's inline bootstrap
//     script, which changes with every build and only the build knows.
//     It also says `object-src 'none'` and `base-uri 'none'`.
//   - This one, sent as a response header: every fetch directive except
//     the script ones. It has to be written at run time because it
//     names the backend's loopback lanes, and the kernel assigns their
//     ports at every start (backend/src/lanes.rs; the handshake in
//     lib/backend-handshake.cjs announces them).
//
// This half never names a script source and has no `default-src`, so it
// can neither widen nor narrow what the meta allows to run; scripts are
// the meta's alone. No `'unsafe-eval'` anywhere, in either build: the
// loader's Lottie runs on lottie-web's light build, which has no
// expression engine.
//
// Packaged builds send it on every response of the `app://` protocol
// handler (`policedResponse`); development appends it to the Vite dev
// server's main-frame responses through `webRequest`
// (`withPolicyHeader`). Pure: no Electron, no I/O.

/** The lanes the renderer reaches, besides the API's (`apiBase`). The
 *  handshake records any lowercase name it is told; only these become
 *  sources. */
const LANE_NAMES = Object.freeze(["stream", "image", "probe", "resolve", "download"]);

/** What the backend binds: 127.0.0.1, a port, nothing after it. A
 *  string that is not exactly this is dropped, never widened: an origin
 *  goes into a header value, where a `;` would start a directive and a
 *  space or `,` would add a source or a policy. */
const LOOPBACK = /^http:\/\/127\.0\.0\.1:([1-9][0-9]{0,4})$/;

/** The one inline style attribute the app renders: SvelteKit's route
 *  announcer (`#svelte-announcer`, @sveltejs/kit
 *  src/core/sync/write_root.js), which reads each new page's title to
 *  screen readers while staying off screen. Refused, it would sit in the
 *  page and print the title after every navigation. It is allowed by its
 *  hash and nothing else; a SvelteKit that changes it shows up as a
 *  violation in the e2e suite's policy guard. */
const ANNOUNCER_STYLE =
  "position: absolute; left: 0; top: 0; clip: rect(0 0 0 0); clip-path: inset(50%); overflow: hidden; white-space: nowrap; width: 1px; height: 1px";
const ANNOUNCER_HASH = `'sha256-${require("node:crypto").createHash("sha256").update(ANNOUNCER_STYLE).digest("base64")}'`;

function loopbackOrigin(value) {
  if (typeof value !== "string") return null;
  const m = LOOPBACK.exec(value);
  return m && Number(m[1]) <= 65535 ? value : null;
}

/** The API's origin and the known lanes', validated, deduplicated, in
 *  a fixed order. Empty before the handshake. */
function loopbackOrigins(backend) {
  if (!backend || typeof backend !== "object") return [];
  const lanes = backend.lanes && typeof backend.lanes === "object" ? backend.lanes : {};
  const candidates = [
    backend.apiBase,
    ...LANE_NAMES.map((name) => (Object.hasOwn(lanes, name) ? lanes[name] : null)),
  ];
  const out = [];
  for (const candidate of candidates) {
    const origin = loopbackOrigin(candidate);
    if (origin && !out.includes(origin)) out.push(origin);
  }
  return out;
}

/** The dev server's origin from VITE_DEV_URL, or null for anything that
 *  is not a plain http(s) origin. */
function devOriginFrom(url) {
  if (typeof url !== "string") return null;
  let u;
  try {
    u = new URL(url);
  } catch {
    return null;
  }
  if (u.protocol !== "http:" && u.protocol !== "https:") return null;
  // WHATWG URL lets `;` and `,` through in a host; a header value must not.
  if (!/^[a-z0-9.-]+(:[0-9]{1,5})?$/.test(u.host)) return null;
  return `${u.protocol}//${u.host}`;
}

/**
 * The header policy for this launch. `backend` is what the handshake
 * resolved with (null before it); `devOrigin` is the Vite server's
 * origin in development (from `devOriginFrom`), null when packaged.
 */
function rendererPolicy({ backend, devOrigin }) {
  const lanes = loopbackOrigins(backend);
  const dev = typeof devOrigin === "string" ? devOriginFrom(devOrigin) : null;
  const hmr = dev ? dev.replace(/^http/, "ws") : null;
  const directives = [
    // Bundled CSS. Vite injects each module's CSS as a <style> in dev.
    ["style-src", ["'self'", ...(dev ? ["'unsafe-inline'"] : [])]],
    // Style attributes: SvelteKit's announcer and nothing else. Svelte
    // writes every other style through the CSSOM, which this does not
    // govern.
    ["style-src-attr", ["'unsafe-hashes'", ANNOUNCER_HASH]],
    // Posters and every other image go through the image lane (with the
    // API's origin as the renderer's fallback); `data:` is the grain
    // texture written into the stylesheets.
    ["img-src", ["'self'", "data:", ...lanes]],
    // No faces are bundled today; ready for self-hosted ones.
    ["font-src", ["'self'"]],
    // The API, the probes, the play and download event streams, hls.js's
    // playlists and segments, sidecar subtitle listings. 'self' is
    // SvelteKit's version.json; the socket is Vite's HMR.
    ["connect-src", ["'self'", ...(hmr ? [hmr] : []), ...lanes]],
    // hls.js attaches through a MediaSource blob; MP4 sessions and
    // subtitle <track>s load from the stream lane.
    ["media-src", ["blob:", ...lanes]],
    ["worker-src", ["'none'"]],
    ["frame-src", ["'none'"]],
    ["child-src", ["'none'"]],
    ["object-src", ["'none'"]],
    ["manifest-src", ["'none'"]],
    ["base-uri", ["'none'"]],
    ["form-action", ["'none'"]],
    ["frame-ancestors", ["'none'"]],
  ];
  return directives.map(([name, sources]) => `${name} ${sources.join(" ")}`).join("; ");
}

/**
 * Development: the response headers for a `webRequest.onHeadersReceived`
 * callback. Only the dev server's main frame gets the policy, appended
 * to any policy it already sends (SvelteKit's own, with the bootstrap's
 * hash); every other response is left alone (`{}`).
 */
function withPolicyHeader(details, { devOrigin, policy }) {
  if (!devOrigin || details.resourceType !== "mainFrame") return {};
  let u;
  try {
    u = new URL(details.url);
  } catch {
    return {};
  }
  // An http(s) URL's origin is a real one (unlike app:'s, which is 'null').
  if (u.origin !== devOrigin) return {};
  const headers = { ...(details.responseHeaders || {}) };
  const key =
    Object.keys(headers).find((k) => k.toLowerCase() === "content-security-policy") ||
    "Content-Security-Policy";
  const existing = headers[key];
  headers[key] = [...(Array.isArray(existing) ? existing : existing ? [existing] : []), policy];
  return { responseHeaders: headers };
}

/**
 * Packaged: `response` as the protocol handler served it, rebuilt with
 * the policy. `net.fetch` answers with immutable headers, so the
 * response is rebuilt — keeping its body, status and every header, the
 * content type above all: a module script served without its
 * JavaScript type fails Chromium's strict MIME check, and the window
 * stays blank.
 */
function policedResponse(response, policy) {
  const headers = new Headers(response.headers);
  headers.set("content-security-policy", policy);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

module.exports = {
  LANE_NAMES,
  loopbackOrigins,
  devOriginFrom,
  rendererPolicy,
  withPolicyHeader,
  policedResponse,
};
