"use strict";

// The splash's stuck state.
//
// A launch whose first page has not said it rendered SPLASH_STUCK_MS
// after the splash painted, whatever step is still pending, keeps the
// splash up rather than revealing a window that may still be empty.
// The splash's line says the start is taking longer than usual, the
// pending stretch of its rail pulses faintly, and two actions appear
// under the line: Restart and Quit (splash/progress.js draws them,
// in the app's language: lib/main-messages.cjs). The film keeps
// running: the app is alive and still trying.
//
// Going stuck, the splash stops being always-on-top — it is now
// waiting for a decision and should not cover the person's other work
// indefinitely — and flashes its taskbar entry once if it does not
// have focus. It asks the page for a choice: the page answers when an
// action is used (a click, or Enter or Space on the focused one;
// nothing is focused for the person, and Escape does nothing). There
// is no new final give-up: a packaged launch keeps its 120 s
// deadlines for the backend's handshake and the window's first paint
// (lib/backend-handshake.cjs, lib/startup.cjs), whose failure dialog
// takes the splash away; a page that painted but never says it
// rendered keeps the splash up until the person chooses.
//
// What a choice does is lib/splash.cjs's. A page that renders after
// the stuck state hands over as usual: its rail completes and the page
// makes the actions inert. A second stall, after a Restart, is a new
// launch and goes the same way; the log says each.

/** How long after the splash painted a pending first page makes the
 *  splash go stuck. */
const SPLASH_STUCK_MS = 15_000;

/** How long the taskbar entry flashes: about one flash. */
const SPLASH_FLASH_MS = 1_000;

/** What the main process runs in the splash to show the stuck state.
 *  It evaluates to the page's choice once one is made ("restart" or
 *  "quit"), or to null when the page has nothing to offer (it is
 *  complete, or was opened without the words). */
const STUCK_CALL =
  "window.splashProgress && window.splashProgress.stuck && window.splashProgress.stuck()";

const CHOICES = new Set(["restart", "quit"]);

/** Whether `answer` is one of the page's choices. */
function isChoice(answer) {
  return typeof answer === "string" && CHOICES.has(answer);
}

/**
 * The stuck state's clock and window handling, for one splash.
 *
 * - `arm()` starts the count, once: at the paint, or as soon as the
 *   splash cannot be told it is shown, or at the paint's own bound when
 *   it never answers (lib/splash.cjs). Later calls change nothing.
 * - When the count runs out with `pending()` naming a step (the boot is
 *   not past the splash and the page has not rendered), the splash goes
 *   stuck: it is logged, taken off always-on-top, shown (without focus)
 *   if it is not on screen, flashed once, and asked for a choice, which
 *   goes to `onChoice`.
 * - `offerAgain()` asks the page again, after a choice that could not
 *   be carried out.
 * - `landed()` says the page rendered: logged with how long after the
 *   stuck state, when there was one.
 * - `stop()` clears the timers; the splash is going.
 */
function stuckWatch({
  window,
  pending,
  onChoice,
  onStuck = () => {},
  log = () => {},
  logError = () => {},
  clock = () => Date.now(),
  since = () => "",
  stuckAfterMs = SPLASH_STUCK_MS,
  flashMs = SPLASH_FLASH_MS,
}) {
  let timer = null;
  let flashTimer = null;
  let armedAt = null;
  let stuckAt = null;
  const ask = (win) => {
    Promise.resolve(win.webContents.executeJavaScript(STUCK_CALL)).then(
      (answer) => {
        if (isChoice(answer)) onChoice(answer);
      },
      (err) => logError("[main] could not show the splash's stuck state:", err),
    );
  };
  const flash = (win) => {
    if (win.isFocused()) return;
    win.flashFrame(true);
    flashTimer = setTimeout(() => {
      flashTimer = null;
      if (!win.isDestroyed()) win.flashFrame(false);
    }, flashMs);
  };
  const enter = () => {
    timer = null;
    const step = pending();
    const win = window();
    if (!step || !win || win.isDestroyed()) return;
    stuckAt = clock();
    onStuck();
    log(
      `[main] splash stuck: the ${step} step is still pending ${stuckAt - armedAt} ms after the splash painted (${since()})`,
    );
    try {
      win.setAlwaysOnTop(false);
      if (!win.isVisible()) win.showInactive();
      flash(win);
    } catch (err) {
      logError("[main] could not step the stuck splash back from the top:", err);
    }
    ask(win);
  };
  return {
    arm() {
      if (armedAt !== null) return;
      armedAt = clock();
      timer = setTimeout(enter, stuckAfterMs);
    },
    offerAgain() {
      const win = window();
      if (stuckAt !== null && win && !win.isDestroyed()) ask(win);
    },
    landed() {
      if (stuckAt === null) return;
      log(`[main] splash: the page rendered ${clock() - stuckAt} ms after the splash went stuck (${since()})`);
    },
    stuck: () => stuckAt !== null,
    stop() {
      clearTimeout(timer);
      timer = null;
      if (flashTimer === null) return;
      // A flash still on stops with the splash: a hidden one (a boot
      // that failed) would otherwise flash on behind the dialog.
      clearTimeout(flashTimer);
      flashTimer = null;
      const win = window();
      if (win && !win.isDestroyed()) win.flashFrame(false);
    },
  };
}

module.exports = { SPLASH_FLASH_MS, SPLASH_STUCK_MS, STUCK_CALL, isChoice, stuckWatch };
