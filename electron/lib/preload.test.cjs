// The preload: what it puts on `window.aniGui` from the flags main.js
// opened the window with. It runs sandboxed, where `require` reaches
// only Electron's own modules, so it parses those flags itself rather
// than through a module in lib/; this runs the file as Electron would,
// with a stand-in for the bridge, and reads back what it exposed.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "..", "preload.js"), "utf-8");

function exposedWith(argv) {
  const exposed = {};
  const electron = {
    contextBridge: {
      exposeInMainWorld: (name, value) => {
        exposed[name] = value;
      },
    },
    ipcRenderer: { invoke() {}, send() {}, sendSync() {}, on() {}, removeListener() {} },
  };
  vm.runInNewContext(source, {
    require: (id) => {
      assert.equal(id, "electron", "a sandboxed preload can require only Electron");
      return electron;
    },
    process: { argv, platform: "linux" },
    console: { error() {} },
  });
  return exposed.aniGui;
}

test("the API's origin and the secret come off their flags", () => {
  const bridge = exposedWith([
    "electron",
    "--ani-gui-api-base=http://127.0.0.1:1",
    "--ani-gui-internal-secret=s",
  ]);
  assert.equal(bridge.apiBase, "http://127.0.0.1:1");
  assert.equal(bridge.internalSecret, "s");
});

test("every lane flag becomes a lane by name", () => {
  const bridge = exposedWith([
    "electron",
    "--ani-gui-api-base=http://127.0.0.1:1",
    "--ani-gui-lane-probe=http://127.0.0.1:3",
    "--ani-gui-lane-image=http://127.0.0.1:4",
  ]);
  assert.deepEqual({ ...bridge.lanes }, { probe: "http://127.0.0.1:3", image: "http://127.0.0.1:4" });
});

test("a window opened with no lanes has none, and the renderer falls back to the API's origin", () => {
  const bridge = exposedWith(["electron", "--ani-gui-api-base=http://127.0.0.1:1"]);
  assert.equal(typeof bridge.lanes, "object");
  assert.deepEqual(Object.keys(bridge.lanes), []);
});
