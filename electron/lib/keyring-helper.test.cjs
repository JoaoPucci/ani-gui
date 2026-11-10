// The keyring helper's side of the conversation (lib/keyring-helper.cjs):
// requests in on a stream, answers out behind the reply mark, through a
// stand-in for safeStorage. No test here touches a real keyring.

const test = require("node:test");
const assert = require("node:assert/strict");
const { PassThrough } = require("node:stream");

const {
  KEYRING_HELPER_SWITCH,
  REPLY_MARK,
  serveKeyring,
} = require("./keyring-helper.cjs");

function fakeSafeStorage(over = {}) {
  return {
    isEncryptionAvailable: () => true,
    getSelectedStorageBackend: () => "gnome_libsecret",
    encryptString: (text) => Buffer.from(`enc:${text}`),
    decryptString: (buf) => {
      const s = buf.toString();
      if (!s.startsWith("enc:"))
        throw new Error("Error while decrypting the ciphertext");
      return s.slice(4);
    },
    ...over,
  };
}

function serve(safeStorage) {
  const input = new PassThrough();
  const lines = [];
  let ended = false;
  serveKeyring({
    input,
    write: (l) => lines.push(l),
    safeStorage,
    onEnd: () => (ended = true),
  });
  const replies = () =>
    lines.map((l) => {
      assert.ok(
        l.startsWith(REPLY_MARK) && l.endsWith("\n"),
        `a reply line: ${l}`,
      );
      return JSON.parse(l.slice(REPLY_MARK.length));
    });
  return { input, replies, ended: () => ended };
}

const tick = () => new Promise((r) => setImmediate(r));

test("the switch is one Chromium ignores and the app recognises", () => {
  assert.match(KEYRING_HELPER_SWITCH, /^--ani-gui-[a-z-]+$/);
});

test("status reports availability and the backend", async () => {
  const s = serve(fakeSafeStorage());
  s.input.write(`${JSON.stringify({ id: 1, op: "status" })}\n`);
  await tick();
  assert.deepEqual(s.replies(), [
    { id: 1, ok: true, available: true, backend: "gnome_libsecret" },
  ]);
});

test("status names no backend where safeStorage has no such call", async () => {
  const ss = fakeSafeStorage();
  delete ss.getSelectedStorageBackend;
  const s = serve(ss);
  s.input.write(`${JSON.stringify({ id: 1, op: "status" })}\n`);
  await tick();
  assert.equal(s.replies()[0].backend, null);
});

test("encrypt and decrypt round-trip through base64", async () => {
  const s = serve(fakeSafeStorage());
  s.input.write(
    `${JSON.stringify({ id: 1, op: "encrypt", text: '{"a":1}' })}\n`,
  );
  await tick();
  const [enc] = s.replies();
  assert.equal(enc.ok, true);
  s.input.write(
    `${JSON.stringify({ id: 2, op: "decrypt", data: enc.data })}\n`,
  );
  await tick();
  assert.deepEqual(s.replies()[1], { id: 2, ok: true, text: '{"a":1}' });
});

test("a failed call answers with its error, and the next request is still served", async () => {
  const s = serve(fakeSafeStorage());
  const junk = Buffer.from("v11junk").toString("base64");
  // Split across writes: a request is a line, however it arrives.
  const line = `${JSON.stringify({ id: 7, op: "decrypt", data: junk })}\n`;
  s.input.write(line.slice(0, 10));
  s.input.write(
    `${line.slice(10)}${JSON.stringify({ id: 8, op: "status" })}\n`,
  );
  await tick();
  const [failed, status] = s.replies();
  assert.deepEqual(failed, {
    id: 7,
    ok: false,
    message: "Error while decrypting the ciphertext",
  });
  assert.equal(status.id, 8);
  assert.equal(status.ok, true);
});

test("lines that are not requests are ignored, unknown requests answered as failures", async () => {
  const s = serve(fakeSafeStorage());
  s.input.write("not json\n\n42\n");
  s.input.write(`${JSON.stringify({ id: 3, op: "wipe" })}\n`);
  await tick();
  assert.deepEqual(s.replies(), [
    { id: 3, ok: false, message: "unknown keyring request: wipe" },
  ]);
});

test("the helper ends when its input closes", async () => {
  const s = serve(fakeSafeStorage());
  s.input.end();
  await tick();
  assert.equal(s.ended(), true);
});
