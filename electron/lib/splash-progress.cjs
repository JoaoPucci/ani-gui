"use strict";

// What the splash's progress rail is told, and what its stall line
// says (splash/progress.js draws both).
//
// The rail has three stretches, one per step of the boot, lit as each
// step really lands: the backend's handshake, the launch's flags
// decided, and the app's first page rendered. It carries no words and
// no numbers. The line under it appears only when the pending step has
// taken longer than STALL_MS, and goes when the step lands.

/** The boot's steps, in the order they land. */
const STEPS = ["backend", "flags", "page"];

/** How long a step may be pending before the stall line says so. */
const STALL_MS = 700;

/**
 * What the stall line says while each step is pending, from the main
 * process's own message table (lib/main-messages.cjs), in step order.
 *
 * Every step says the generic line for now. A step that is slow for a
 * reason the user would want named — a one-time migration or cache
 * reclaim in the backend's start — gets its own line here once the
 * backend reports that job.
 */
function stallTexts(messages) {
  return STEPS.map(() => messages.splashStartingUp);
}

module.exports = { STALL_MS, STEPS, stallTexts };
