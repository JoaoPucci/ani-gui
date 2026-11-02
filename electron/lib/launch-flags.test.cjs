const test = require("node:test");
const assert = require("node:assert/strict");

const { FLAGS_WAIT_MS, isFlagsAnswer, waitForLaunchFlags } = require("./launch-flags.cjs");

const answer = (status, body) => async () => ({
  ok: status >= 200 && status < 300,
  status,
  json: async () => {
    if (body instanceof Error) throw body;
    return body;
  },
});

test("the wait outlasts the backend's own limit on deciding", () => {
  // backend/src/flags/store.rs: SETTLE_LIMIT is three seconds.
  assert.equal(FLAGS_WAIT_MS, 4000);
  assert.ok(FLAGS_WAIT_MS > 3000);
});

test("the backend's answer is the launch's flags", async () => {
  let asked = null;
  const body = { origin: "file", flags: { a: false, b: "old" } };
  const got = await waitForLaunchFlags("http://127.0.0.1:9", {
    fetch: async (url, init) => {
      asked = url;
      assert.ok(init && init.signal, "the request can be cut off");
      return answer(200, body)();
    },
  });
  assert.equal(asked, "http://127.0.0.1:9/api/flags");
  assert.deepEqual(got, body);
});

test("an error status, a body that is not JSON, or a failed request is no flags", async () => {
  for (const fetch of [
    answer(500, { origin: "file", flags: {} }),
    answer(200, new Error("not json")),
    async () => {
      throw new Error("connection refused");
    },
  ]) {
    assert.equal(await waitForLaunchFlags("http://127.0.0.1:9", { fetch }), null);
  }
});

test("an answer of the wrong shape is no flags", () => {
  for (const body of [
    null,
    [],
    "flags",
    { flags: {} },
    { origin: "file" },
    { origin: 1, flags: {} },
    { origin: "file", flags: [] },
    { origin: "file", flags: null },
    { origin: "file", flags: { a: 1 } },
    { origin: "file", flags: { a: null } },
    { origin: "file", flags: { a: { b: true } } },
  ]) {
    assert.equal(isFlagsAnswer(body), false, JSON.stringify(body));
  }
  assert.equal(isFlagsAnswer({ origin: "test", flags: {} }), true);
  assert.equal(isFlagsAnswer({ origin: "cache", flags: { a: true, b: "x" } }), true);
});

test("a backend that never answers is given up on at the bound, and the request cut off", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal = null;
  const pending = waitForLaunchFlags("http://127.0.0.1:9", {
    fetch: (_url, init) => {
      signal = init.signal;
      return new Promise(() => {});
    },
  });
  t.mock.timers.tick(FLAGS_WAIT_MS - 1);
  await Promise.resolve();
  assert.equal(signal.aborted, false);
  t.mock.timers.tick(1);
  assert.equal(await pending, null);
  assert.equal(signal.aborted, true);
});
