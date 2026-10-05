// What a failed boot tells the user.
//
// A boot that fails logs its reason and exits with code 1. From a
// terminal that is enough. A packaged app started from a desktop has
// no terminal: the user double-clicks, nothing appears, and nothing
// says why. So a failed boot shows an error dialog before it exits —
// except where nobody is there to read one:
//
//   - an unattended run (ANI_GUI_UNATTENDED=1, which the e2e suites
//     set): the cold-launch retry depends on a failed boot ending the
//     process at once, and a dialog waits for a person;
//   - a dev launch: its terminal already carries the reason, and a
//     dialog would hold `pnpm dev` open until someone clicked it away.
//
// And wherever it is shown it ends by itself, so a run nobody is
// watching and nobody flagged — a packager's smoke test under Xvfb —
// still exits instead of sitting on a dialog for good.

const test = require("node:test");
const assert = require("node:assert/strict");

const { bootFailureDialog, showBounded } = require("./boot-failure.cjs");
const { LOCALES, messagesFor } = require("./main-messages.cjs");

const failure = new Error("spawn /opt/ani-gui/resources/ani-gui-backend EACCES");

test("an unattended run gets no dialog", () => {
  const dialog = bootFailureDialog({
    env: { ANI_GUI_UNATTENDED: "1" },
    isDev: false,
    locale: "en",
    error: failure,
  });
  assert.equal(dialog, null);
});

test("a dev launch gets no dialog", () => {
  assert.equal(bootFailureDialog({ env: {}, isDev: true, locale: "en", error: failure }), null);
});

test("a packaged app somebody started gets an error dialog carrying the reason", () => {
  const dialog = bootFailureDialog({ env: {}, isDev: false, locale: "en", error: failure });
  assert.equal(dialog.options.type, "error");
  assert.equal(dialog.options.message, messagesFor("en").bootFailed);
  assert.ok(dialog.options.detail.includes(failure.message));
  assert.deepEqual(dialog.options.buttons, [messagesFor("en").close]);
  assert.equal(dialog.timeoutMs, 60_000);
});

test("the dialog is in the app's language, whichever it ships", () => {
  for (const locale of LOCALES) {
    const { options } = bootFailureDialog({ env: {}, isDev: false, locale, error: failure });
    assert.equal(options.message, messagesFor(locale).bootFailed, locale);
    assert.deepEqual(options.buttons, [messagesFor(locale).close], locale);
    assert.ok(options.detail.includes(failure.message), locale);
  }
});

test("a failure that is not an Error still has its reason shown", () => {
  const { options } = bootFailureDialog({ env: {}, isDev: false, locale: "en", error: "boom" });
  assert.ok(options.detail.includes("boom"));
});

test("anything but 1 does not mark a run unattended", () => {
  for (const value of ["", "0", "true"]) {
    assert.notEqual(
      bootFailureDialog({
        env: { ANI_GUI_UNATTENDED: value },
        isDev: false,
        locale: "en",
        error: failure,
      }),
      null,
      JSON.stringify(value),
    );
  }
});

// showBounded: the dialog may hold the exit only so long.

const dialogFor = (timeoutMs) => ({ options: { message: "m" }, timeoutMs });
const settled = (promise, withinMs) =>
  Promise.race([
    promise.then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), withinMs)),
  ]);

test("no dialog shows nothing and holds nothing", async () => {
  let shown = 0;
  await showBounded(null, () => (shown += 1));
  assert.equal(shown, 0);
});

test("the wait lasts until the dialog is dismissed", async () => {
  let dismiss;
  const shown = [];
  const wait = showBounded(dialogFor(60_000), (options) => {
    shown.push(options);
    return new Promise((resolve) => (dismiss = resolve));
  });
  assert.equal(await settled(wait, 50), false);
  assert.deepEqual(shown, [{ message: "m" }]);
  dismiss({ response: 0 });
  assert.equal(await settled(wait, 1_000), true);
});

test("a dialog nobody dismisses ends by itself", async () => {
  const wait = showBounded(dialogFor(40), () => new Promise(() => {}));
  assert.equal(await settled(wait, 10), false);
  assert.equal(await settled(wait, 1_000), true);
});

test("a dialog that cannot be shown does not hold the exit", async () => {
  const throws = showBounded(dialogFor(60_000), () => {
    throw new Error("no display");
  });
  assert.equal(await settled(throws, 1_000), true);
  const rejects = showBounded(dialogFor(60_000), () => Promise.reject(new Error("no display")));
  assert.equal(await settled(rejects, 1_000), true);
});
