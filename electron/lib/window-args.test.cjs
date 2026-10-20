// What a window is opened with: the flags main.js hands the preload
// through `webPreferences.additionalArguments`, which the preload reads
// back off `process.argv` (see preload.test.cjs for that half).

const test = require("node:test");
const assert = require("node:assert/strict");

const { windowArguments } = require("./window-args.cjs");

test("the API's origin, the locale and the secret go as they always have", () => {
  assert.deepEqual(
    windowArguments({ apiBase: "http://127.0.0.1:1", internalSecret: "s", lanes: {} }, "pt-BR"),
    [
      "--ani-gui-api-base=http://127.0.0.1:1",
      "--ani-gui-locale=pt-BR",
      "--ani-gui-internal-secret=s",
    ],
  );
});

test("a missing locale or secret is left out rather than sent empty", () => {
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
    "--ani-gui-internal-secret=s",
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
