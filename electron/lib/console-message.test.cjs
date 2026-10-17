// The renderer's console lines reach the main process log tagged with
// their level. Electron's `console-message` event numbers the levels
// 0 to 3 as verbose, info, warning and error (electron.d.ts for the
// bundled Electron), and Chromium reports `console.debug` as verbose,
// `console.log` and `console.info` as info. A tag table that started
// at "log" for 0 shifted every level by one: a renderer error read as
// "log", a warning as "error", and every ordinary log line as
// "warning".

const test = require("node:test");
const assert = require("node:assert/strict");

const { rendererConsoleLine, rendererLogTag } = require("./console-message.cjs");

test("each of Electron's four levels gets its own name", () => {
  assert.equal(rendererLogTag(0), "verbose");
  assert.equal(rendererLogTag(1), "info");
  assert.equal(rendererLogTag(2), "warning");
  assert.equal(rendererLogTag(3), "error");
});

test("a level outside Electron's range is tagged as a plain log line", () => {
  for (const level of [-1, 4, 1.5, Number.NaN, undefined]) {
    assert.equal(rendererLogTag(level), "log");
  }
});

test("the relayed line names its level, the message and where it came from", () => {
  assert.equal(
    rendererConsoleLine(3, "boom", 12, "app://x/a.js"),
    "[renderer:error] boom (app://x/a.js:12)"
  );
  assert.equal(
    rendererConsoleLine(0, "[play] quality: start", 7, "http://localhost:5173/p.js"),
    "[renderer:verbose] [play] quality: start (http://localhost:5173/p.js:7)"
  );
});
