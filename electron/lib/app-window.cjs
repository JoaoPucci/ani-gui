"use strict";

// The app's own window: how it is made, and the renderer's word that
// its first page has actually rendered, which the reveal waits for.
//
// The window is revealed at ready-to-show, which is the first paint of
// whatever document has loaded. On a slow first render — Vite compiling
// the frontend on a cold dev start, a cold disk — that is the bare shell
// before the app's stylesheet and first route, which Chromium paints
// white. So the window paints the app's own background from the start,
// and the reveal waits for the renderer to say its first page is up.

/** The app's page background (tokens.css --ink-000), which the window
 *  paints before its page does, and the splash too. */
const APP_BACKGROUND = "#0c0a09";

/** The channel the renderer says it has rendered on. */
const APP_READY_CHANNEL = "ani-gui:app-ready";

/** The channel the renderer says its first page has mounted on: before
 *  any frame, so it comes whether or not the window gets frames. */
const APP_MOUNTED_CHANNEL = "ani-gui:app-mounted";

/** How long after the page mounted its rendered word may take before
 *  the window is woken (frameWake). Two frames take tens of
 *  milliseconds where a hidden window gets frames, and a heavy first
 *  layout a few hundred; where it gets none (native Wayland without GPU
 *  compositing) the word never comes, and this is what the boot pays
 *  before the window is mapped. */
const FRAME_WAKE_MS = 1500;

/** The longest the reveal waits for that word, counted from the
 *  window's creation, when no splash stands in for the window: a
 *  window opened later (macOS reopening one against the running
 *  backend), or a boot whose splash could not be made. Past it, the
 *  window is revealed anyway, on the dark background, never white —
 *  it would otherwise stay hidden for good, with nothing on screen.
 *
 *  While the boot's splash stands in, the wait has no bound
 *  (appReadyBound): a page that is late leaves the splash stuck,
 *  offering Restart and Quit (lib/splash-stuck.cjs), and the window is
 *  revealed only once its page has rendered. The bound used to apply
 *  there too, and revealed a window whose page had not painted: dark
 *  and empty. */
const APP_READY_WAIT_MS = 15_000;

/** The bound the wait for the window's first page takes: none while
 *  `splash` stands in for the window, APP_READY_WAIT_MS otherwise. */
function appReadyBound(splash) {
  return splash.standsIn() ? null : APP_READY_WAIT_MS;
}

/**
 * What fails the window's first open once the splash is `lost`
 * (lib/splash.cjs): the page gets back a bound of APP_READY_WAIT_MS,
 * counted from the loss, or from now when the splash was lost before
 * the window was made. A page that has rendered, or renders within it,
 * fails nothing and is revealed as usual. One that has not by then
 * rejects this with the loss's error, and the boot fails on it (main.js,
 * openFirstPage's `abort`): nothing else on screen could end or continue
 * the launch, and revealing a page that never said it rendered may show
 * it empty. A page whose renderer went (`ready` false) is left to the
 * boot's own renderer-gone failure, which names the cause. Never
 * settles otherwise.
 */
function abortWhenLost(lost, ready, { graceMs = APP_READY_WAIT_MS } = {}) {
  let outcome = null;
  const settledReady = Promise.resolve(ready).then((r) => {
    outcome = r === true ? "rendered" : "gone";
    return outcome;
  });
  const never = () => new Promise(() => {});
  return Promise.resolve(lost).then(never, (err) => {
    if (outcome) return never();
    let timer;
    const late = new Promise((resolve) => {
      timer = setTimeout(() => resolve("late"), graceMs);
    });
    return Promise.race([settledReady, late]).then((what) => {
      clearTimeout(timer);
      if (what !== "late") return never();
      throw err;
    });
  });
}

/** The app window's options. */
function appWindowOptions({ width, height, icon, preload, additionalArguments }) {
  return {
    width,
    height,
    minWidth: 960,
    minHeight: 600,
    show: false,
    backgroundColor: APP_BACKGROUND,
    icon,
    // Frameless: the renderer draws its titlebar and controls. Resizable
    // stays at its default (true), so the OS resize edges still work.
    frame: false,
    webPreferences: { preload, contextIsolation: true, nodeIntegration: false, additionalArguments },
  };
}

let current = { rendered: false, gaveUp: false, beforeReveal: false };

/**
 * Wait for `contents`' renderer to say, on APP_READY_CHANNEL of `ipc`,
 * that its first page has rendered. `ready` settles true when it does,
 * false after `timeoutMs` (never, when that is null), and false when
 * the renderer goes or the contents are destroyed first — `gone()` then
 * says so. A renderer gone fails the boot (lib/startup.cjs
 * waitBeforeShow); destroyed contents are a window closed by a quit,
 * which reveals nothing (revealMaximized).
 * Either way the listeners go. Start it when the window is made, so a
 * word that comes early is not missed. The record notes whether the
 * window was still hidden (`isShown`) when the word came: the reveal
 * waited for it.
 *
 * The renderer sends the word after two animation frames, which run in
 * a hidden window only because `paintWhenInitiallyHidden` is left at its
 * default (true). Turned off, `ready-to-show` never fires either, and
 * the window is never revealed (packaged: the boot fails at the
 * first-show deadline). Even left on, a hidden window gets no frames
 * under native Wayland with software compositing; frameWake maps such a
 * window so the word can come.
 */
function appReadyWaiter(contents, ipc, { timeoutMs = APP_READY_WAIT_MS, isShown = () => false } = {}) {
  const state = { rendered: false, gaveUp: false, beforeReveal: false };
  current = state;
  let settle;
  const ready = new Promise((resolve) => (settle = resolve));
  let timer = null;
  let settled = false;
  let lost = false;
  const watchesContents = typeof contents.on === "function";
  const done = (rendered, { gaveUp = !rendered } = {}) => {
    if (settled) return;
    settled = true;
    ipc.removeListener(APP_READY_CHANNEL, onReady);
    if (watchesContents) {
      contents.removeListener("render-process-gone", onGone);
      contents.removeListener("destroyed", onGone);
    }
    if (timer) clearTimeout(timer);
    state.rendered = rendered;
    state.gaveUp = gaveUp;
    settle(rendered);
  };
  // Matched by the sending contents' id: Electron may hand over another
  // wrapper of the same contents.
  function onReady(event) {
    if (!event || !event.sender || event.sender.id !== contents.id) return;
    state.beforeReveal = !isShown();
    done(true);
  }
  // With no bound, nothing else would end the wait for a renderer that
  // can no longer say anything.
  function onGone() {
    lost = true;
    done(false, { gaveUp: false });
  }
  ipc.on(APP_READY_CHANNEL, onReady);
  if (watchesContents) {
    contents.on("render-process-gone", onGone);
    contents.on("destroyed", onGone);
  }
  if (timeoutMs != null) timer = setTimeout(() => done(false), timeoutMs);
  return { ready, gone: () => lost };
}

/** What the latest launch's wait for the renderer saw, for the
 *  end-to-end suite. */
function lastAppReady() {
  return { ...current };
}

let currentWake = { mounted: false, woken: false };

/**
 * Wake a hidden window whose page cannot draw.
 *
 * The reveal waits for the page's rendered word (appReadyWaiter), which
 * takes two animation frames. Under native Wayland with software
 * compositing (a virtual machine, a GPU that failed or is blocklisted,
 * `--disable-gpu`) a window that is not mapped gets no frames after its
 * first, sometimes not even that, so the word would never come. X11
 * keeps hidden windows ticking, and so does a GPU, which is why the
 * reveal can wait for it at all.
 *
 * So the page also says when it has mounted (APP_MOUNTED_CHANNEL),
 * which needs no frame. `ready` is the waiter's: once the window's own
 * page (matched by its contents' id) has mounted, if `ready` has not
 * settled within `boundMs`, `wake` runs, once — main.js maps the window
 * (wakeHidden) and its frames resume; the rendered word then follows and
 * the reveal goes as usual. Nothing runs when `ready` settles first,
 * when `stop()` was called (the reveal came, or the window went), or
 * when the launch was `cancelled` by then.
 *
 * Returns `woken()`, and `stop()`. The latest wait's record, for the
 * end-to-end suite, is lastWake().
 */
function frameWake(contents, ipc, { ready, wake, cancelled = () => false, boundMs = FRAME_WAKE_MS }) {
  const record = { mounted: false, woken: false };
  currentWake = record;
  let timer = null;
  let over = false;
  const end = () => {
    if (over) return;
    over = true;
    clearTimeout(timer);
    timer = null;
    ipc.removeListener(APP_MOUNTED_CHANNEL, onMounted);
  };
  function onMounted(event) {
    if (!event || !event.sender || event.sender.id !== contents.id) return;
    if (record.mounted || over) return;
    record.mounted = true;
    timer = setTimeout(() => {
      timer = null;
      if (over || cancelled()) return end();
      end();
      record.woken = true;
      wake();
    }, boundMs);
  }
  ipc.on(APP_MOUNTED_CHANNEL, onMounted);
  Promise.resolve(ready).then(end, end);
  return { woken: () => record.woken, stop: end };
}

/** What the latest window's wake saw: whether its page said it mounted,
 *  and whether the window was woken. For the end-to-end suite. */
function lastWake() {
  return { ...currentWake };
}

/**
 * Map the hidden window `win` so it gets frames, behind the splash
 * where the platform keeps the splash on top.
 *
 * Mapped inactive first, then maximized: maximize() on a hidden window
 * maps and activates it, which would take the focus from wherever the
 * person is. `revealing` runs first (main.js arms the granted-size
 * recorder, lib/window-geometry.cjs): the window is maximized here, not
 * at the reveal, so the size the desktop grants it lands now.
 *
 * Native Wayland honours no always-on-top: the compositor stacks the
 * new window over the splash and, the app being the focused one, gives
 * it the focus. Each focus it takes before the reveal runs `restack`
 * (main.js maps the splash again, which puts it back on top). The window
 * is still visible around the splash meanwhile; on those machines it
 * is that or no window at all.
 *
 * Returns what detaches the restack (the reveal calls it), or null for
 * a window gone or already on screen, which is left alone.
 */
function wakeHidden(win, { revealing = () => {}, restack }) {
  if (win.isDestroyed() || win.isVisible()) return null;
  revealing();
  win.showInactive();
  win.maximize();
  const onFocus = () => restack();
  win.on("focus", onFocus);
  return () => win.removeListener("focus", onFocus);
}

/** The reveal's gate: open once the splash is done (`whenShown`) and
 *  the renderer has said its first page rendered, or the wait for it
 *  gave up — and, either way, once the splash has been told and has
 *  completed its rail (`pageReady`, lib/splash.cjs), so no reveal closes
 *  the splash on a rail that is not seen complete. */
function revealGate(splash, appReady) {
  return Promise.all([
    splash.whenShown(),
    // A renderer that went is no page to complete the rail for: the
    // boot is about to fail.
    appReady.ready.then((rendered) => (appReady.gone && appReady.gone() ? undefined : splash.pageReady(rendered === true))),
  ]);
}

module.exports = {
  abortWhenLost,
  revealGate,
  APP_BACKGROUND,
  APP_MOUNTED_CHANNEL,
  FRAME_WAKE_MS,
  frameWake,
  lastWake,
  wakeHidden,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  appReadyBound,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
};
