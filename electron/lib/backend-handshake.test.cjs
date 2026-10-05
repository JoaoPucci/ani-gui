// The backend's startup handshake. The boot waits on it before it
// opens a window, so every way the backend can fail to start has to
// end the wait — otherwise the app sits with no window and no exit:
//
//   - a spawn the OS refuses (EACCES on a binary that lost its exec
//     bit, EPERM from an antivirus quarantine on Windows) emits only
//     'error', never 'exit';
//   - a backend that starts but never prints the handshake hangs the
//     wait forever.

const test = require("node:test");
const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");

const { HANDSHAKE_TIMEOUT_MS, awaitHandshake } = require("./backend-handshake.cjs");

const fakeChild = () => {
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  return child;
};
const say = (child, text) => child.stdout.emit("data", Buffer.from(text));

test("both handshake lines resolve the wait, in either order and split across chunks", async () => {
  const child = fakeChild();
  const got = awaitHandshake(child, { timeoutMs: 1_000, stopChild: () => {} });
  say(child, "ANI_GUI_INTERNAL_SECRET ab");
  say(child, "cd\nANI_GUI_LISTENING http://127.0.0.1:4242\n");
  assert.deepEqual(await got, { apiBase: "http://127.0.0.1:4242", internalSecret: "abcd" });
});

test("a backend that exits before the handshake fails the wait", async () => {
  const child = fakeChild();
  const got = awaitHandshake(child, { timeoutMs: 1_000, stopChild: () => {} });
  child.emit("exit", 1, null);
  await assert.rejects(got, /exited before handshake \(code=1/);
});

test("a spawn the OS refuses fails the wait", async () => {
  const child = fakeChild();
  const got = awaitHandshake(child, { timeoutMs: 1_000, stopChild: () => {} });
  child.emit("error", Object.assign(new Error("spawn EACCES"), { code: "EACCES" }));
  await assert.rejects(got, (err) => err.code === "EACCES");
});

// Bounded so a wait with no timer of its own fails instead of hanging the run.
test("a backend that never completes the handshake is stopped and fails the wait", { timeout: 2_000 }, async () => {
  const child = fakeChild();
  const stopped = [];
  const got = awaitHandshake(child, { timeoutMs: 20, stopChild: (c) => stopped.push(c) });
  say(child, "ANI_GUI_LISTENING http://127.0.0.1:4242\n");
  await assert.rejects(got, /handshake within 20 ms/);
  assert.deepEqual(stopped, [child]);
});

test("after the handshake, output and exit are logged and the timer is gone", async () => {
  const child = fakeChild();
  const logged = [];
  const stopped = [];
  const got = awaitHandshake(child, {
    timeoutMs: 20,
    stopChild: (c) => stopped.push(c),
    log: (line) => logged.push(line),
  });
  say(child, "ANI_GUI_LISTENING http://x\nANI_GUI_INTERNAL_SECRET s\n");
  await got;
  say(child, "ready\n");
  await new Promise((r) => setTimeout(r, 40));
  child.emit("error", new Error("kill ESRCH"));
  child.emit("exit", 0, null);
  assert.deepEqual(stopped, []);
  assert.deepEqual(logged.slice(0, 1), ["[backend] ready"]);
  assert.equal(logged.length, 3);
});

// The budget itself. The deadline is there for a backend that will
// never answer, not for one that is slow — and a first start can be
// slow: it creates the database and runs every migration, each a
// commit the disk has to confirm. Measured on a hard disk with a fresh
// profile, that start took about a second on an idle disk and about
// 24 s while an 8 GB write was in progress. A budget shorter than
// that turns a busy disk into "could not start".

/** Lets the promise callbacks run; setImmediate is not mocked. */
const settled = () => new Promise((resolve) => setImmediate(resolve));

test("a backend a minute and a half into a slow first start is late, not failed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const child = fakeChild();
  const stopped = [];
  let outcome = "waiting";
  const got = awaitHandshake(child, {
    timeoutMs: HANDSHAKE_TIMEOUT_MS,
    stopChild: (c) => stopped.push(c),
  });
  got.then(
    () => (outcome = "up"),
    () => (outcome = "failed"),
  );
  t.mock.timers.tick(90_000);
  await settled();
  assert.equal(outcome, "waiting");
  assert.deepEqual(stopped, []);
  say(child, "ANI_GUI_LISTENING http://127.0.0.1:4242\nANI_GUI_INTERNAL_SECRET s\n");
  assert.deepEqual(await got, { apiBase: "http://127.0.0.1:4242", internalSecret: "s" });
});

test("a backend that never answers still ends the boot, at the end of the budget", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const child = fakeChild();
  const stopped = [];
  const got = awaitHandshake(child, {
    timeoutMs: HANDSHAKE_TIMEOUT_MS,
    stopChild: (c) => stopped.push(c),
  });
  t.mock.timers.tick(HANDSHAKE_TIMEOUT_MS);
  await assert.rejects(got, /did not complete its handshake/);
  assert.deepEqual(stopped, [child]);
});
