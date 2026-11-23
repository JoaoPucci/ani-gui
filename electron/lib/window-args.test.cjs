// What a window is opened with: the flags main.js hands the preload
// through `webPreferences.additionalArguments`, which the preload reads
// back off `process.argv` (see preload.test.cjs for that half).

const test = require("node:test");
const assert = require("node:assert/strict");

const { windowArguments } = require("./window-args.cjs");

test("the API's origin and the locale go on the window's command line", () => {
  assert.deepEqual(
    windowArguments({ apiBase: "http://127.0.0.1:1", internalSecret: "s", lanes: {} }, "pt-BR"),
    ["--ani-gui-api-base=http://127.0.0.1:1", "--ani-gui-locale=pt-BR"],
  );
});

// A renderer's command line is readable by other local programs
// (/proc/<pid>/cmdline on Linux, the process list on Windows). The
// launch's secret reaches the preload over IPC instead
// (lib/secret-handoff.cjs).
test("the launch's secret never goes on the window's command line", () => {
  const secret = "c0ffee".repeat(10) + "abcd";
  const args = windowArguments(
    { apiBase: "http://127.0.0.1:1", internalSecret: secret, lanes: { image: "http://127.0.0.1:4" } },
    "en",
    { origin: "file", flags: { a: true } },
  );
  for (const a of args) {
    assert.ok(!a.includes(secret), a);
    assert.ok(!a.startsWith("--ani-gui-internal-secret"), a);
  }
});

test("a missing locale is left out rather than sent empty", () => {
  assert.deepEqual(windowArguments({ apiBase: "http://127.0.0.1:1" }, null), [
    "--ani-gui-api-base=http://127.0.0.1:1",
  ]);
});

test("every lane goes as a flag of its own, named after the lane", () => {
  const args = windowArguments(
    {
      apiBase: "http://127.0.0.1:1",
      internalSecret: "s",
      lanes: { probe: "http://127.0.0.1:3", image: "http://127.0.0.1:4" },
    },
    null,
  );
  assert.deepEqual(args, [
    "--ani-gui-api-base=http://127.0.0.1:1",
    "--ani-gui-lane-probe=http://127.0.0.1:3",
    "--ani-gui-lane-image=http://127.0.0.1:4",
  ]);
});

test("a lane whose name or origin would not survive the command line is dropped", () => {
  const args = windowArguments(
    {
      apiBase: "http://127.0.0.1:1",
      lanes: { "pro=be": "http://127.0.0.1:3", image: "", stream: "http://a b", ok: "http://127.0.0.1:5" },
    },
    null,
  );
  assert.deepEqual(args, ["--ani-gui-api-base=http://127.0.0.1:1", "--ani-gui-lane-ok=http://127.0.0.1:5"]);
});

test("the launch's flags go as one flag, with their origin", () => {
  const args = windowArguments({ apiBase: "http://127.0.0.1:1" }, null, {
    origin: "file",
    flags: { a: false, "b c": "old=new" },
  });
  const flagsArg = args.find((a) => a.startsWith("--ani-gui-flags="));
  assert.ok(flagsArg, JSON.stringify(args));
  assert.deepEqual(
    JSON.parse(decodeURIComponent(flagsArg.slice("--ani-gui-flags=".length))),
    { a: false, "b c": "old=new" },
  );
  assert.ok(!/\s/.test(flagsArg), "survives the command line unquoted");
  assert.ok(args.includes("--ani-gui-flags-origin=file"));
});

test("no flags from the backend sends no flags", () => {
  assert.deepEqual(windowArguments({ apiBase: "http://127.0.0.1:1" }, null, null), [
    "--ani-gui-api-base=http://127.0.0.1:1",
  ]);
});
