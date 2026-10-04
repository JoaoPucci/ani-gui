// before-quit runs the active-downloads prompt, then stops the backend
// tree. A quit the user cancels at that prompt leaves the app running,
// so it must leave the backend running too — otherwise the window
// stays up over a dead backend and every request fails.

const test = require("node:test");
const assert = require("node:assert/strict");

const { handleBeforeQuit } = require("./quit.cjs");

test("a quit cancelled at the prompt keeps the backend", () => {
  let stopped = 0;
  handleBeforeQuit({ promptOnClose: () => true, stopBackend: () => (stopped += 1) });
  assert.equal(stopped, 0);
});

test("a quit that goes ahead stops the backend", () => {
  let stopped = 0;
  handleBeforeQuit({ promptOnClose: () => false, stopBackend: () => (stopped += 1) });
  assert.equal(stopped, 1);
});
