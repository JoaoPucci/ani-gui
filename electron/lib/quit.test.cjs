// before-quit runs the active-downloads prompt, then stops the backend
// tree. A quit the user cancels at that prompt leaves the app running,
// so it must leave the backend running too — otherwise the window
// stays up over a dead backend and every request fails.

const test = require("node:test");
const assert = require("node:assert/strict");

const { closePromptOptions, handleBeforeQuit } = require("./quit.cjs");
const { LOCALES, messagesFor } = require("./main-messages.cjs");

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

// The boot needs to know a quit is under way: a quit during the
// backend's handshake stops the backend, which fails the handshake, and
// that failure is the quit's doing, not a broken install.
test("before-quit says whether the quit goes ahead", () => {
  assert.equal(handleBeforeQuit({ promptOnClose: () => true, stopBackend: () => {} }), false);
  assert.equal(handleBeforeQuit({ promptOnClose: () => false, stopBackend: () => {} }), true);
});

// The prompt itself. It is the main process's, not the renderer's —
// it has to be synchronous for the close to be preventable — and so
// it never went through the renderer's messages: an app running in
// Portuguese asked about its downloads in English.

test("the downloads prompt is in the app's language, whichever it ships", () => {
  for (const locale of LOCALES) {
    const messages = messagesFor(locale);
    const options = closePromptOptions({ locale, count: 3 });
    assert.equal(options.title, messages.downloadsTitle, locale);
    assert.equal(options.message, messages.downloadsInProgress(3), locale);
    assert.equal(options.detail, messages.downloadsQuitDetail, locale);
    assert.deepEqual(options.buttons, [messages.cancel, messages.quitAnyway], locale);
  }
});

test("the prompt says how many downloads, and reads right for one", () => {
  assert.equal(closePromptOptions({ locale: "en", count: 1 }).message, "1 download in progress.");
  assert.equal(closePromptOptions({ locale: "en", count: 3 }).message, "3 downloads in progress.");
  for (const locale of LOCALES) {
    for (const count of [1, 2, 5, 21]) {
      const { message } = closePromptOptions({ locale, count });
      assert.ok(message.includes(String(count)), `${locale}: ${message}`);
    }
  }
});

test("cancelling is the prompt's default and its escape", () => {
  for (const locale of LOCALES) {
    const options = closePromptOptions({ locale, count: 2 });
    assert.equal(options.type, "question");
    assert.equal(options.defaultId, 0);
    assert.equal(options.cancelId, 0);
    assert.equal(options.buttons[0], messagesFor(locale).cancel);
  }
});
