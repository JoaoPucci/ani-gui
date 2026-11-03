const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  MAX_DISPLAYS,
  GRANT_SETTLE_MS,
  displayKey,
  grantedSizeRecorder,
  initialSize,
  readGeometry,
  remember,
  writeGeometry,
} = require("./window-geometry.cjs");

// The headless GNOME Wayland session: Wayland reports no work area, so
// the work area is the whole screen; the compositor maximizes to the
// height below its top bar.
const wayland = {
  id: 1,
  bounds: { x: 0, y: 0, width: 1920, height: 1080 },
  workAreaSize: { width: 1920, height: 1080 },
  scaleFactor: 1,
};

test("a display is known by which it is, its size and its scale", () => {
  assert.equal(displayKey(wayland), "1:1920x1080@1");
  assert.notEqual(displayKey({ ...wayland, scaleFactor: 2 }), displayKey(wayland));
  assert.notEqual(
    displayKey({ ...wayland, bounds: { ...wayland.bounds, height: 1200 } }),
    displayKey(wayland),
  );
  assert.notEqual(displayKey({ ...wayland, id: 2 }), displayKey(wayland));
});

test("the first launch on a display makes the window at its work area", () => {
  assert.deepEqual(initialSize({}, wayland), { width: 1920, height: 1080 });
});

test("a later launch makes it at the size the desktop granted last time", () => {
  const store = remember({}, wayland, { x: 0, y: 32, width: 1920, height: 1048 });
  assert.deepEqual(initialSize(store, wayland), { width: 1920, height: 1048 });
});

test("a size remembered for another display, size or scale is not used", () => {
  const store = remember({}, wayland, { x: 0, y: 32, width: 1920, height: 1048 });
  const scaled = { ...wayland, scaleFactor: 1.25 };
  assert.deepEqual(initialSize(store, scaled), { width: 1920, height: 1080 });
  const other = { ...wayland, id: 7 };
  assert.deepEqual(initialSize(store, other), { width: 1920, height: 1080 });
});

test("a remembered size that could not be the display's is not used", () => {
  for (const bad of [
    { width: 4000, height: 1048 },
    { width: 1920, height: 0 },
    { width: -1, height: 1048 },
    { width: 1920.5, height: 1048 },
    { width: "1920", height: 1048 },
    null,
    "1920x1048",
  ]) {
    const store = { [displayKey(wayland)]: bad };
    assert.deepEqual(initialSize(store, wayland), { width: 1920, height: 1080 }, JSON.stringify(bad));
  }
});

test("remembering keeps the newest few displays and replaces a display's old size", () => {
  let store = {};
  for (let id = 1; id <= MAX_DISPLAYS + 3; id += 1) {
    store = remember(store, { ...wayland, id }, { x: 0, y: 0, width: 1000 + id, height: 700 });
  }
  assert.equal(Object.keys(store).length, MAX_DISPLAYS);
  assert.deepEqual(initialSize(store, { ...wayland, id: 1 }), { width: 1920, height: 1080 });
  assert.deepEqual(initialSize(store, { ...wayland, id: MAX_DISPLAYS + 3 }), {
    width: 1000 + MAX_DISPLAYS + 3,
    height: 700,
  });
  const again = remember(store, { ...wayland, id: MAX_DISPLAYS + 3 }, { x: 0, y: 0, width: 900, height: 600 });
  assert.equal(Object.keys(again).length, MAX_DISPLAYS);
  assert.deepEqual(initialSize(again, { ...wayland, id: MAX_DISPLAYS + 3 }), { width: 900, height: 600 });
});

test("the remembered sizes survive a write and read back, and a missing or broken file is none", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-geometry-"));
  const file = path.join(dir, "window-geometry.json");
  assert.deepEqual(readGeometry(file, fs), {});
  const store = remember({}, wayland, { x: 0, y: 32, width: 1920, height: 1048 });
  writeGeometry(file, store, fs);
  assert.deepEqual(readGeometry(file, fs), store);
  for (const body of ["", "{", "[]", "null", '"x"']) {
    fs.writeFileSync(file, body);
    assert.deepEqual(readGeometry(file, fs), {}, body);
  }
  fs.rmSync(dir, { recursive: true, force: true });
});

test("a write that cannot land only loses the memory", () => {
  const throwing = {
    writeFileSync() {
      throw new Error("read-only file system");
    },
    mkdirSync() {},
  };
  assert.doesNotThrow(() => writeGeometry("/nowhere/window-geometry.json", {}, throwing));
});

test("a remembered size taller or wider than today's work area is cut to it", () => {
  // A panel that grew since: the window must not be made taller than
  // the area it will be maximized into.
  const x11 = {
    id: 3,
    bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    workAreaSize: { width: 1920, height: 1000 },
    scaleFactor: 1,
  };
  const store = remember({}, x11, { x: 0, y: 0, width: 1920, height: 1040 });
  assert.deepEqual(initialSize(store, x11), { width: 1920, height: 1000 });
});

test("remembering the same size again leaves the store as it was", () => {
  const store = remember({}, wayland, { x: 0, y: 32, width: 1920, height: 1048 });
  assert.equal(remember(store, wayland, { x: 0, y: 32, width: 1920, height: 1048 }), store);
});

test("remembering a display's unchanged size still makes it the newest", () => {
  let store = {};
  for (let id = 1; id <= MAX_DISPLAYS; id += 1) {
    store = remember(store, { ...wayland, id }, { x: 0, y: 0, width: 1000 + id, height: 700 });
  }
  // Display 1, used again at the same size, is the newest now ...
  store = remember(store, { ...wayland, id: 1 }, { x: 0, y: 0, width: 1001, height: 700 });
  // ... so a new display evicts display 2, not display 1.
  store = remember(store, { ...wayland, id: 99 }, { x: 0, y: 0, width: 900, height: 600 });
  assert.deepEqual(initialSize(store, { ...wayland, id: 1 }), { width: 1001, height: 700 });
  assert.deepEqual(initialSize(store, { ...wayland, id: 2 }), { width: 1920, height: 1080 });
});

test("only the sizes granted while the launch's reveal settles are recorded", (t) => {
  // What the next launch must match is the size this launch's window
  // got as it was revealed maximized on the display it was made for.
  // Recording runs from just before the reveal maximizes the window
  // until the reveal has settled; a later maximize, or a maximized
  // window moved to another monitor (by keyboard it stays maximized),
  // is not recorded.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const writes = [];
  const recorder = grantedSizeRecorder({ save: (size) => writes.push(size) });
  recorder.maximized({ width: 1, height: 1 }); // before the reveal
  recorder.revealing();
  recorder.maximized({ width: 1920, height: 1080 });
  recorder.maximized({ width: 1920, height: 1048 }); // the compositor's configure
  t.mock.timers.tick(GRANT_SETTLE_MS);
  recorder.maximized({ width: 2560, height: 1400 }); // moved by keyboard, still maximized
  assert.deepEqual(writes, [
    { width: 1920, height: 1080 },
    { width: 1920, height: 1048 },
  ]);
});

test("the reveal's own unmaximize while it settles does not stop the recording", (t) => {
  // Measured on GNOME under Wayland: the window reports unmaximized
  // for a moment after its show, before the compositor's maximized
  // configure arrives with the size that must be kept.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const writes = [];
  const recorder = grantedSizeRecorder({ save: (size) => writes.push(size) });
  recorder.revealing();
  recorder.maximized({ width: 1920, height: 1080 });
  recorder.shown();
  recorder.unmaximized();
  recorder.maximized({ width: 1920, height: 1048 });
  assert.deepEqual(writes.at(-1), { width: 1920, height: 1048 });
});
