"use strict";

// How a renderer console line is relayed to the main process log,
// lifted out of main.js so it can be exercised without Electron.

const { redactSecrets } = require("./log-redact.cjs");

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

/**
 * The line the main process logs for a renderer console message, with
 * the launch's secret redacted from the message and from where it came
 * from (lib/log-redact.cjs): a failed <img> load is logged with its
 * URL, which carries the secret. `secrets` are the launch's own.
 */
function rendererConsoleLine(level, message, line, source, secrets = []) {
  return redactSecrets(
    `[renderer:${rendererLogTag(level)}] ${message} (${source}:${line})`,
    secrets,
  );
}

module.exports = { rendererConsoleLine, rendererLogTag };
