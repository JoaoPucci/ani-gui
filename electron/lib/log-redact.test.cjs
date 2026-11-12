// What the main process writes to its log — the renderer's console
// lines, failed loads, the backend's output — never carries the
// launch's secret. The renderer's <img> and EventSource URLs carry it
// as `secret=` (frontend/src/lib/backend/secret.ts), Chromium logs a
// failed load with its URL, and the main process's stdout lands in the
// journal or a terminal another user may read.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const { REDACTED, redactSecrets } = require("./log-redact.cjs");

const SECRET = "0123456789abcdef".repeat(4);

test("a secret= query parameter is redacted, wherever it sits in the query", () => {
  assert.equal(
    redactSecrets(`http://127.0.0.1:4/api/image?url=https%3A%2F%2Fa%2Fb.jpg&secret=${SECRET}`),
    `http://127.0.0.1:4/api/image?url=https%3A%2F%2Fa%2Fb.jpg&secret=${REDACTED}`,
  );
  assert.equal(
    redactSecrets(`GET /api/play/stream?secret=${SECRET}&title=a 503`),
    `GET /api/play/stream?secret=${REDACTED}&title=a 503`,
  );
  assert.equal(
    redactSecrets(`Failed to load resource (http://h/api/image?secret=${SECRET}#x)`),
    `Failed to load resource (http://h/api/image?secret=${REDACTED}#x)`,
  );
});

test("a URL-encoded secret= inside another URL is redacted too", () => {
  assert.equal(
    redactSecrets(`next=%2Fapi%2Fimage%3Fsecret%3D${SECRET}%26url%3Dx`),
    `next=%2Fapi%2Fimage%3Fsecret%3D${REDACTED}%26url%3Dx`,
  );
});

test("the header and the handshake line are redacted", () => {
  assert.equal(
    redactSecrets(`x-ani-gui-internal-secret: ${SECRET}`),
    `x-ani-gui-internal-secret: ${REDACTED}`,
  );
  assert.equal(
    redactSecrets(`{"x-ani-gui-internal-secret":"${SECRET}"}`),
    `{"x-ani-gui-internal-secret":"${REDACTED}"}`,
  );
  assert.equal(
    redactSecrets(`ANI_GUI_INTERNAL_SECRET ${SECRET}`),
    `ANI_GUI_INTERNAL_SECRET ${REDACTED}`,
  );
});

test("a known secret is redacted wherever it appears, in any form", () => {
  assert.equal(
    redactSecrets(`token ${SECRET} and again${SECRET}`, [SECRET]),
    `token ${REDACTED} and again${REDACTED}`,
  );
});

test("a line with no secret in it is left as it is", () => {
  for (const line of [
    "[renderer:error] boom (app://localhost/a.js:12)",
    "http://127.0.0.1:4/api/image?url=https%3A%2F%2Fa%2Fb.jpg",
    "the secret service did not answer",
    "?secretive=1&notsecret=2",
    "",
  ]) {
    assert.equal(redactSecrets(line, [SECRET]), line);
  }
});

test("a short or missing known secret redacts nothing by itself", () => {
  assert.equal(redactSecrets("a b c", ["", null, undefined, "a"]), "a b c");
});

test("no known secret survives redaction, whatever surrounds it", () => {
  const hex = fc.stringMatching(/^[0-9a-f]{64}$/);
  fc.assert(
    fc.property(hex, fc.string(), fc.string(), (secret, before, after) => {
      const out = redactSecrets(`${before}${secret}${after}`, [secret]);
      assert.ok(!out.includes(secret), out);
    }),
  );
});

test("no secret= value survives redaction, in a query or a header, known or not", () => {
  const value = fc.stringMatching(/^[0-9A-Za-z_-]{8,80}$/);
  const query = fc.stringMatching(/^[a-z]{1,8}=[a-z0-9]{0,8}$/);
  fc.assert(
    fc.property(value, query, fc.boolean(), (v, other, first) => {
      const url = first
        ? `http://127.0.0.1:9/api/image?secret=${v}&${other}`
        : `http://127.0.0.1:9/api/image?${other}&secret=${v}`;
      assert.ok(!redactSecrets(url).includes(`secret=${v}`), url);
      assert.ok(!redactSecrets(`x-ani-gui-internal-secret: ${v}`).includes(v));
    }),
  );
});

// The backend's stderr reaches the main process in chunks of whatever
// size the pipe delivers; a value split across two of them would slip
// past a redaction run on each chunk alone.
const { redactingLines } = require("./log-redact.cjs");

test("output relayed in chunks is redacted a whole line at a time", () => {
  const written = [];
  const lines = redactingLines((text) => written.push(text));
  lines.push("GET /api/image?sec");
  lines.push(`ret=${SECRET.slice(0, 20)}`);
  lines.push(`${SECRET.slice(20)}&url=x\nnext `);
  assert.deepEqual(written, [`GET /api/image?secret=${REDACTED}&url=x\n`]);
  lines.push("line\n");
  lines.end();
  lines.end();
  assert.deepEqual(written.slice(1), ["next line\n"]);
});

test("an unfinished last line is written, redacted, at the end", () => {
  const written = [];
  const lines = redactingLines((text) => written.push(text));
  lines.push(`x-ani-gui-internal-secret: ${SECRET}`);
  assert.deepEqual(written, []);
  lines.end();
  assert.deepEqual(written, [`x-ani-gui-internal-secret: ${REDACTED}`]);
});

test("however the output is cut into chunks, what is written is the redacted whole", () => {
  fc.assert(
    fc.property(fc.array(fc.nat(), { maxLength: 8 }), (cuts) => {
      const text = `a\nGET /api/play/stream?title=a&secret=${SECRET}&mode=sub\nb ${SECRET}\nc`;
      const at = [...new Set(cuts.map((c) => c % text.length))].sort((x, y) => x - y);
      const written = [];
      const lines = redactingLines((t) => written.push(t), () => [SECRET]);
      let from = 0;
      for (const cut of at) {
        lines.push(text.slice(from, cut));
        from = cut;
      }
      lines.push(text.slice(from));
      lines.end();
      assert.equal(written.join(""), redactSecrets(text, [SECRET]));
    }),
  );
});

// The backend's own output is relayed before the launch's secret is
// known (its handshake carries it), and after. Once it is known, the
// relays redact it by value as well as by shape.
const { backendRelay } = require("./log-redact.cjs");

test("the backend's relays redact the launch's secret by value once the handshake names it", () => {
  const out = [];
  const err = [];
  const relay = backendRelay({ stdout: (t) => out.push(t), stderr: (t) => err.push(t) });
  relay.stderr(`early ${SECRET} secret=${SECRET}\n`);
  relay.learn(SECRET);
  relay.stdout(`late "secret":"${SECRET}"`);
  relay.stderr(`bare ${SECRET}\n`);
  relay.stderr("unfinished");
  relay.end();
  assert.deepEqual(out, [`late "secret":"${REDACTED}"\n`]);
  assert.deepEqual(err, [
    `[backend] early ${SECRET} secret=${REDACTED}\n`,
    `[backend] bare ${REDACTED}\n`,
    "[backend] unfinished",
  ]);
});

test("a redacted value ends at a separator, and what follows it is kept", () => {
  assert.equal(redactSecrets(`secret=${SECRET};next=1`), `secret=${REDACTED};next=1`);
  assert.equal(redactSecrets(`x secret=${SECRET}, then more`), `x secret=${REDACTED}, then more`);
  assert.equal(redactSecrets(`a secret=${SECRET}|next`), `a secret=${REDACTED}|next`);
});
