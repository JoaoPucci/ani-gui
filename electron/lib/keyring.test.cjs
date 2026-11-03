// The OS keyring as the main process uses it (lib/keyring.cjs): every
// call asynchronous, and on Linux made by the keyring helper, so a
// keyring waiting on its unlock prompt — or not answering at all —
// holds a promise rather than the thread the windows run on.
//
// The helper is a stand-in here: a fake child for the protocol, and one
// real child process (plain Node serving the helper's protocol over a
// safeStorage that never answers) for what waiting looks like from
// this side. No test touches a real keyring.

const test = require("node:test");
const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const { PassThrough } = require("node:stream");
const { spawn } = require("node:child_process");
const path = require("node:path");
const fc = require("fast-check");

const {
  createHelperClient,
  createKeyring,
  helperCommand,
  isRealEncryption,
  readToken,
  writeToken,
} = require("./keyring.cjs");
const { KEYRING_HELPER_SWITCH, REPLY_MARK } = require("./keyring-helper.cjs");

const tick = () => new Promise((r) => setImmediate(r));

/** A spawn whose children the test answers by hand. */
function fakeSpawn() {
  const children = [];
  const spawnFn = (command, args, options) => {
    const child = new EventEmitter();
    child.stdin = new PassThrough();
    child.stdout = new PassThrough();
    child.requests = [];
    child.killed = [];
    child.kill = (sig) => child.killed.push(sig);
    let buf = "";
    child.stdin.on("data", (c) => {
      buf += c;
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        child.requests.push(JSON.parse(buf.slice(0, nl)));
        buf = buf.slice(nl + 1);
      }
    });
    child.reply = (obj) =>
      child.stdout.write(`${REPLY_MARK}${JSON.stringify(obj)}\n`);
    children.push({ command, args, options, child });
    return child;
  };
  return { spawnFn, children };
}

function settledState(promise) {
  const state = { settled: false };
  promise.then(
    (v) => Object.assign(state, { settled: true, value: v }),
    (e) => Object.assign(state, { settled: true, error: e }),
  );
  return state;
}

// ─── helperCommand ────────────────────────────────────────────────

test("the packaged helper is the app's own binary, headless and windowless", () => {
  const cmd = helperCommand({
    execPath: "/opt/ani-gui/ani-gui.bin",
    isPackaged: true,
    appPath: "/opt/ani-gui/resources/app.asar",
    env: { PATH: "/usr/bin", ELECTRON_RUN_AS_NODE: "1" },
  });
  assert.equal(cmd.command, "/opt/ani-gui/ani-gui.bin");
  assert.deepEqual(cmd.args, [
    KEYRING_HELPER_SWITCH,
    "--ozone-platform=headless",
    "--disable-gpu",
    "--no-sandbox",
  ]);
  // Run as Node it would never reach safeStorage.
  assert.deepEqual(cmd.env, { PATH: "/usr/bin" });
});

test("an unpackaged helper gets the app's path, as `electron .` does", () => {
  const cmd = helperCommand({
    execPath: "/x/electron",
    isPackaged: false,
    appPath: "/repo/electron",
    env: {},
  });
  assert.equal(cmd.args[0], "/repo/electron");
  assert.equal(cmd.args[1], KEYRING_HELPER_SWITCH);
});

// ─── createHelperClient ───────────────────────────────────────────

test("the helper starts on the first request, not before, and serves the rest", async () => {
  const { spawnFn, children } = fakeSpawn();
  const client = createHelperClient({
    spawn: spawnFn,
    command: "bin",
    args: ["a"],
    env: { E: "1" },
  });
  assert.equal(children.length, 0);
  const first = client.request("status");
  const second = client.request("decrypt", { data: "eA==" });
  await tick();
  assert.equal(children.length, 1);
  const { child, options } = children[0];
  assert.deepEqual(options.env, { E: "1" });
  assert.deepEqual(
    child.requests.map((r) => r.op),
    ["status", "decrypt"],
  );
  // Answers are matched by id, whatever order they come in; anything
  // else on stdout is not an answer.
  child.stdout.write("[1103/184304.393919:ERROR:object_proxy.cc(576)] noise\n");
  child.reply({ id: child.requests[1].id, ok: true, text: "plain" });
  child.reply({
    id: child.requests[0].id,
    ok: true,
    available: true,
    backend: "kwallet6",
  });
  assert.equal((await second).text, "plain");
  assert.equal((await first).backend, "kwallet6");
});

test("a failed call rejects with the helper's message", async () => {
  const { spawnFn, children } = fakeSpawn();
  const client = createHelperClient({
    spawn: spawnFn,
    command: "bin",
    args: [],
  });
  const p = client.request("decrypt", { data: "" });
  await tick();
  const { child } = children[0];
  child.reply({
    id: child.requests[0].id,
    ok: false,
    message: "Decryption is not available.",
  });
  await assert.rejects(p, /Decryption is not available/);
});

test("requests out when the helper ends reject; the next request starts a new one", async () => {
  const { spawnFn, children } = fakeSpawn();
  const client = createHelperClient({
    spawn: spawnFn,
    command: "bin",
    args: [],
  });
  const p = client.request("status");
  await tick();
  children[0].child.emit("exit", null, "SIGKILL");
  await assert.rejects(p, /keyring helper ended \(SIGKILL\)/);
  const q = client.request("status");
  await tick();
  assert.equal(children.length, 2);
  const { child } = children[1];
  child.reply({
    id: child.requests[0].id,
    ok: true,
    available: false,
    backend: null,
  });
  assert.equal((await q).available, false);
});

test("a helper that fails to start rejects what was asked of it", async () => {
  const { spawnFn, children } = fakeSpawn();
  const client = createHelperClient({
    spawn: spawnFn,
    command: "bin",
    args: [],
  });
  const p = client.request("status");
  children[0].child.emit("error", new Error("spawn bin ENOENT"));
  await assert.rejects(p, /ENOENT/);
});

test("stop kills the helper with SIGKILL, the one signal a helper waiting on the keyring obeys", async () => {
  const { spawnFn, children } = fakeSpawn();
  const client = createHelperClient({
    spawn: spawnFn,
    command: "bin",
    args: [],
  });
  client.stop(); // nothing started, nothing to stop
  void client.request("status").catch(() => {});
  await tick();
  client.stop();
  assert.deepEqual(children[0].child.killed, ["SIGKILL"]);
});

test("a real helper that never answers holds its promise, not this process", async () => {
  // Plain Node serving the helper's protocol, over a safeStorage whose
  // first call blocks its thread for good — as libsecret's does on an
  // unanswered prompt.
  const script = `
    const { serveKeyring } = require(${JSON.stringify(path.join(__dirname, "keyring-helper.cjs"))});
    serveKeyring({
      input: process.stdin,
      write: (l) => process.stdout.write(l),
      safeStorage: { isEncryptionAvailable() { for (;;) {} } },
      onEnd: () => process.exit(0),
    });
  `;
  const client = createHelperClient({
    spawn,
    command: process.execPath,
    args: ["-e", script],
    env: process.env,
  });
  const read = settledState(client.request("status"));
  // This process's event loop keeps turning while the helper is stuck.
  let ticks = 0;
  const started = Date.now();
  while (Date.now() - started < 300) {
    await new Promise((r) => setTimeout(r, 10));
    ticks += 1;
  }
  assert.ok(ticks > 10, `the event loop kept turning (${ticks} ticks)`);
  assert.equal(read.settled, false);
  client.stop();
  for (let i = 0; i < 100 && !read.settled; i += 1)
    await new Promise((r) => setTimeout(r, 20));
  assert.equal(read.settled, true);
  assert.match(String(read.error), /keyring helper ended \(SIGKILL\)/);
});

// ─── createKeyring ────────────────────────────────────────────────

test("on Linux the keyring is the helper's, and safeStorage is never called here", async () => {
  const calls = [];
  const helper = {
    request: async (op, fields) => {
      calls.push([op, fields]);
      if (op === "status")
        return { available: true, backend: "gnome_libsecret" };
      if (op === "encrypt")
        return { data: Buffer.from("cipher").toString("base64") };
      return { text: "plain" };
    },
  };
  const untouchable = new Proxy(
    {},
    {
      get() {
        throw new Error("safeStorage called in the main process on Linux");
      },
    },
  );
  const keyring = createKeyring({
    platform: "linux",
    safeStorage: untouchable,
    helper,
  });
  assert.deepEqual(await keyring.status(), {
    available: true,
    backend: "gnome_libsecret",
  });
  assert.equal((await keyring.encrypt("t")).toString(), "cipher");
  assert.equal(await keyring.decrypt(Buffer.from("cipher")), "plain");
  assert.deepEqual(calls[2], [
    "decrypt",
    { data: Buffer.from("cipher").toString("base64") },
  ]);
});

test("on Windows the keyring is safeStorage (DPAPI), called directly, with no helper", async () => {
  const helper = {
    request() {
      throw new Error("helper used on Windows");
    },
  };
  const safeStorage = {
    isEncryptionAvailable: () => true,
    encryptString: (t) => Buffer.from(`dpapi:${t}`),
    decryptString: (b) => b.toString().slice(6),
  };
  const keyring = createKeyring({ platform: "win32", safeStorage, helper });
  assert.deepEqual(await keyring.status(), { available: true, backend: null });
  const enc = await keyring.encrypt("{}");
  assert.equal(enc.toString(), "dpapi:{}");
  assert.equal(await keyring.decrypt(enc), "{}");
});

test("a direct keyring that throws rejects instead of throwing", async () => {
  const safeStorage = {
    isEncryptionAvailable: () => true,
    decryptString: () => {
      throw new Error("bad");
    },
  };
  const keyring = createKeyring({
    platform: "win32",
    safeStorage,
    helper: null,
  });
  await assert.rejects(keyring.decrypt(Buffer.from("x")), /bad/);
});

// ─── isRealEncryption ─────────────────────────────────────────────

test("encryption is real when available and not one of the obfuscating fallbacks", () => {
  fc.assert(
    fc.property(
      fc.boolean(),
      fc.oneof(
        fc.constant(null),
        fc.constantFrom(
          "basic_text",
          "unknown",
          "gnome_libsecret",
          "kwallet",
          "kwallet5",
          "kwallet6",
        ),
        fc.string(),
      ),
      (available, backend) => {
        const expected =
          available && backend !== "basic_text" && backend !== "unknown";
        assert.equal(isRealEncryption({ available, backend }), expected);
      },
    ),
  );
});

// ─── readToken / writeToken ───────────────────────────────────────

function memFs(files = {}) {
  const store = new Map(
    Object.entries(files).map(([k, v]) => [k, Buffer.from(v)]),
  );
  return {
    store,
    existsSync: (p) => store.has(p),
    readFileSync: (p) => {
      if (!store.has(p)) throw new Error(`ENOENT ${p}`);
      return store.get(p);
    },
    writeFileSync: (p, data) => store.set(p, Buffer.from(data)),
  };
}

function keyringOf({
  status = { available: true, backend: "gnome_libsecret" },
  decrypt,
  encrypt,
} = {}) {
  const asked = [];
  return {
    asked,
    status: async () => {
      asked.push("status");
      return status;
    },
    decrypt: (b) => {
      asked.push("decrypt");
      return decrypt
        ? decrypt(b)
        : Promise.resolve(b.toString().replace(/^enc:/, ""));
    },
    encrypt: (t) => {
      asked.push("encrypt");
      return encrypt ? encrypt(t) : Promise.resolve(Buffer.from(`enc:${t}`));
    },
  };
}

const FILE = "/profile/tokens/anilist.bin";

test("no token file reads as not_found without asking the keyring anything", async () => {
  const keyring = keyringOf();
  const r = await readToken({ file: () => FILE, fs: memFs(), keyring });
  assert.deepEqual(r, { ok: false, kind: "not_found" });
  assert.deepEqual(keyring.asked, []);
});

test("a token file decrypts to its payload", async () => {
  const fs = memFs({ [FILE]: `enc:${JSON.stringify({ access_token: "a" })}` });
  const r = await readToken({ file: () => FILE, fs, keyring: keyringOf() });
  assert.deepEqual(r, { ok: true, payload: { access_token: "a" } });
});

test("a keyring that only obfuscates is refused, and the file kept", async () => {
  const fs = memFs({ [FILE]: "enc:{}" });
  const keyring = keyringOf({
    status: { available: true, backend: "basic_text" },
  });
  const r = await readToken({ file: () => FILE, fs, keyring });
  assert.deepEqual(r, { ok: false, kind: "encryption_unavailable" });
  assert.deepEqual(keyring.asked, ["status"]);
  assert.equal(fs.store.get(FILE).toString(), "enc:{}");
});

test("a failed decryption is decrypt_error, and the file kept", async () => {
  const fs = memFs({ [FILE]: "v11junk" });
  const keyring = keyringOf({
    decrypt: () => Promise.reject(new Error("Decryption is not available.")),
  });
  const r = await readToken({ file: () => FILE, fs, keyring });
  assert.equal(r.ok, false);
  assert.equal(r.kind, "decrypt_error");
  assert.match(r.message, /Decryption is not available/);
  assert.equal(fs.store.get(FILE).toString(), "v11junk");
});

test("an invalid provider path is decrypt_error, as before", async () => {
  const r = await readToken({
    file: () => {
      throw new Error("invalid provider slug");
    },
    fs: memFs(),
    keyring: keyringOf(),
  });
  assert.deepEqual(r, {
    ok: false,
    kind: "decrypt_error",
    message: "invalid provider slug",
  });
});

test("a keyring that has not answered leaves the read pending, not failed", async () => {
  const fs = memFs({ [FILE]: "enc:{}" });
  const keyring = keyringOf({ decrypt: () => new Promise(() => {}) });
  const read = settledState(readToken({ file: () => FILE, fs, keyring }));
  await tick();
  await tick();
  assert.equal(read.settled, false);
  assert.equal(fs.store.get(FILE).toString(), "enc:{}");
});

test("a write encrypts the payload into the token file", async () => {
  const fs = memFs();
  const r = await writeToken({
    file: () => FILE,
    fs,
    keyring: keyringOf(),
    payload: { a: 1 },
  });
  assert.deepEqual(r, { ok: true });
  assert.equal(fs.store.get(FILE).toString(), 'enc:{"a":1}');
});

test("a write to an obfuscating or absent keyring is refused before anything is written", async () => {
  for (const status of [
    { available: true, backend: "basic_text" },
    { available: false, backend: null },
  ]) {
    const fs = memFs({ [FILE]: "enc:old" });
    const r = await writeToken({
      file: () => FILE,
      fs,
      keyring: keyringOf({ status }),
      payload: {},
    });
    assert.deepEqual(r, { ok: false, kind: "encryption_unavailable" });
    assert.equal(fs.store.get(FILE).toString(), "enc:old");
  }
});

test("a write whose keyring status fails is encryption_unavailable", async () => {
  const keyring = keyringOf();
  keyring.status = () => Promise.reject(new Error("keyring helper ended"));
  const r = await writeToken({
    file: () => FILE,
    fs: memFs(),
    keyring,
    payload: {},
  });
  assert.deepEqual(r, { ok: false, kind: "encryption_unavailable" });
});

test("a write with no payload object is bad_request", async () => {
  const r = await writeToken({
    file: () => FILE,
    fs: memFs(),
    keyring: keyringOf(),
    payload: null,
  });
  assert.deepEqual(r, { ok: false, kind: "bad_request" });
});

test("a failed encryption is io_error and leaves the saved token as it was", async () => {
  const fs = memFs({ [FILE]: "enc:old" });
  const keyring = keyringOf({
    encrypt: () => Promise.reject(new Error("locked")),
  });
  const r = await writeToken({ file: () => FILE, fs, keyring, payload: {} });
  assert.deepEqual(r, { ok: false, kind: "io_error", message: "locked" });
  assert.equal(fs.store.get(FILE).toString(), "enc:old");
});
