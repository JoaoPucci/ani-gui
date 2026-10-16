"use strict";

// How a renderer console line is relayed to the main process log,
// lifted out of main.js so it can be exercised without Electron.

/**
 * Electron's `console-message` event numbers the levels 0 to 3 as
 * verbose, info, warning and error (electron.d.ts for the bundled
 * Electron). Chromium reports `console.debug` as verbose, and
 * `console.log` and `console.info` as info.
 */
const LEVEL_TAGS = ["verbose", "info", "warning", "error"];

/** The tag for an Electron console level; "log" for anything else. */
function rendererLogTag(level) {
  return Number.isInteger(level) ? (LEVEL_TAGS[level] ?? "log") : "log";
}

/** The line the main process logs for a renderer console message. */
function rendererConsoleLine(level, message, line, source) {
  return `[renderer:${rendererLogTag(level)}] ${message} (${source}:${line})`;
}

module.exports = { rendererConsoleLine, rendererLogTag };
