"use strict";

// What the splash's progress rail is told, and what its stall line
// says (splash/progress.js draws both).
//
// The rail has three stretches, one per step of the boot, lit as each
// step really lands: the backend's handshake, the launch's flags
// decided, and the app's first page rendered. The last stretch lights
// as the window is about to be revealed (lib/app-window.cjs), so the
// rail is always seen complete; a page that is late leaves the splash
// stuck instead (lib/splash-stuck.cjs). The rail carries no words and no numbers. The line
// under it appears only when the pending step has taken longer than
// STALL_MS, and then stays until the splash closes: only its words
// change, when the reason does.

/** The boot's steps, in the order they land. */
const STEPS = ["backend", "flags", "page"];

/** How long a step may be pending before the stall line says so. */
const STALL_MS = 700;

/**
 * What the stall line says while each step is pending, from the main
 * process's own message table (lib/main-messages.cjs), in step order.
 * Once a step whose words are its own (the network, for the flags
 * step) lands, the generic line (`splashStartingUp`) takes over at once.
 *
 * The causes we know of for a step past STALL_MS: a stalled network,
 * which holds the flags step up to the fetch's two-second cap; a
 * first-time database on a slow disk, in the backend's start; and, in
 * development, the page step while Vite compiles. The flags step names
 * the network, which is its cause unless the read of the saved copy is
 * itself slow; every other step says the generic line. A job the
 * backend reports itself (a migration, a cache reclaim) would get its
 * own line for the backend step here.
 */
function stallTexts(messages) {
  return STEPS.map((step) => (step === "flags" ? messages.splashNetworkSlow : messages.splashStartingUp));
}

/**
 * The rest of the splash page's words, from the same table: the
 * language it is in, its name for assistive technology, and its stuck
 * state's line and actions (lib/splash-stuck.cjs).
 */
function splashWords(locale, messages) {
  return {
    lang: locale,
    label: messages.splashStarting,
    stuck: messages.splashStuck,
    restart: messages.splashRestart,
    quit: messages.splashQuit,
  };
}

module.exports = { STALL_MS, STEPS, splashWords, stallTexts };
