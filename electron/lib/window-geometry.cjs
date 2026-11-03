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
 *  size and its scale. Stub. */
function displayKey(display) {
  void display;
  return "";
}

/** The size to make the hidden window on `display`. Stub. */
function initialSize(store, display) {
  void store;
  return { ...display.workAreaSize };
}

/** `store` with the maximized `bounds` remembered for `display`. Stub. */
function remember(store, display, bounds) {
  void display;
  void bounds;
  return store;
}

/** The remembered sizes in `file`, or none. Stub. */
function readGeometry(file, fs) {
  void file;
  void fs;
  return {};
}

/** Write `store` to `file`; a failure only loses the memory. Stub. */
function writeGeometry(file, store, fs) {
  void file;
  void store;
  void fs;
}

module.exports = { MAX_DISPLAYS, displayKey, initialSize, readGeometry, remember, writeGeometry };
