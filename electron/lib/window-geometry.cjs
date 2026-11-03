"use strict";

// The size the desktop actually gave the app's window when it
// maximized it, remembered per display, so the next launch can make
// its hidden window at that size and reveal it there.
//
// The window is made hidden and paints its first page before it is
// mapped (lib/splash.cjs). It is made at the display's work area,
// which is the maximized size where the platform reports one; native
// Wayland reports none until a window is on screen, so there the
// work area is the whole screen, and the compositor's maximize then
// takes the top bar's height off a window already on screen — a
// visible correction. Made at the size the compositor granted last
// time on the same display, the window is already the size it will
// be. The first launch on a display, or after its size, scale or top
// bar changes, may still correct once.

/** How many displays are remembered; the oldest is dropped past it. */
const MAX_DISPLAYS = 8;

/** What identifies a display's maximized size: which display, its
 *  size and its scale. */
function displayKey(display) {
  const { width, height } = display.bounds;
  return `${display.id}:${width}x${height}@${display.scaleFactor}`;
}

/** Whether `size` could be a window's size on `display`. */
function fits(size, display) {
  if (!size || typeof size !== "object") return false;
  const ok = (v, max) => Number.isInteger(v) && v > 0 && v <= max;
  return ok(size.width, display.bounds.width) && ok(size.height, display.bounds.height);
}

/** The size to make the hidden window on `display`: the maximized
 *  size remembered for it, or its work area. */
function initialSize(store, display) {
  const remembered = store[displayKey(display)];
  if (fits(remembered, display)) {
    // Never larger than today's work area: a panel may have grown.
    return {
      width: Math.min(remembered.width, display.workAreaSize.width),
      height: Math.min(remembered.height, display.workAreaSize.height),
    };
  }
  return { width: display.workAreaSize.width, height: display.workAreaSize.height };
}

/** `store` with the maximized `bounds` remembered for `display`, as
 *  its newest entry, keeping at most MAX_DISPLAYS. */
function remember(store, display, bounds) {
  const key = displayKey(display);
  const old = store[key];
  const keys = Object.keys(store);
  const unchanged = old && old.width === bounds.width && old.height === bounds.height;
  if (unchanged && keys[keys.length - 1] === key) return store;
  const entries = Object.entries(store).filter(([k]) => k !== key);
  entries.push([key, { width: bounds.width, height: bounds.height }]);
  return Object.fromEntries(entries.slice(-MAX_DISPLAYS));
}

/** The remembered sizes in `file`, or none. */
function readGeometry(file, fs) {
  try {
    const value = JSON.parse(fs.readFileSync(file, "utf-8"));
    return value && typeof value === "object" && !Array.isArray(value) ? value : {};
  } catch {
    return {};
  }
}

/** Write `store` to `file`; a failure only loses the memory. */
function writeGeometry(file, store, fs) {
  try {
    fs.mkdirSync(require("node:path").dirname(file), { recursive: true });
    fs.writeFileSync(file, JSON.stringify(store));
  } catch {
    // The next launch makes the window at the work area instead.
  }
}

/** How long after the reveal begins its granted size is still being
 *  settled: the compositor's configure lands within ~100 ms on GNOME,
 *  so two seconds is generous, and short enough that a user's later
 *  move is not mistaken for it. */
const GRANT_SETTLE_MS = 2000;

/**
 * Records the size this launch's window is granted as it is revealed
 * maximized on the display it was made for, through `save`. Recording
 * starts at `revealing()` — just before the reveal maximizes the
 * window — and ends GRANT_SETTLE_MS later, or at an unmaximize once the
 * window has `shown()`. A later maximize, or a maximized window moved
 * to another monitor (by keyboard it stays maximized, and Wayland gives
 * no position to tell), is not recorded.
 */
function grantedSizeRecorder({ save, setTimer = (fn, ms) => setTimeout(fn, ms) }) {
  let recording = false;
  let shownYet = false;
  let done = false;
  return {
    revealing() {
      if (done) return;
      recording = true;
      setTimer(() => {
        recording = false;
        done = true;
      }, GRANT_SETTLE_MS);
    },
    shown() {
      shownYet = true;
    },
    maximized(size) {
      if (recording) save({ width: size.width, height: size.height });
    },
    unmaximized() {
      if (!shownYet) return;
      recording = false;
      done = true;
    },
  };
}

module.exports = {
  GRANT_SETTLE_MS,
  MAX_DISPLAYS,
  displayKey,
  grantedSizeRecorder,
  initialSize,
  readGeometry,
  remember,
  writeGeometry,
};
