"use strict";

// The desktop's reduced-motion preference, read once at launch on
// Linux.
//
// Chromium takes `prefers-reduced-motion` on Linux from GTK's
// `gtk-enable-animations` alone (ui/gfx/animation/animation_linux.cc),
// which follows `org.gnome.desktop.interface enable-animations`.
// GNOME's Reduced Motion switch (Settings › Accessibility › Seeing)
// writes `org.gnome.desktop.a11y.interface reduced-motion` instead,
// and the desktop portal publishes it as `org.freedesktop.appearance
// reduced-motion`. With the switch on, Chromium still reports
// no-preference and every animation plays.
//
// So the main process asks the portal itself — through busctl, or
// gdbus where busctl is missing — and falls back to the gsettings key
// when the portal cannot answer. When the answer is "reduce" it hands
// Chromium its own `--force-prefers-reduced-motion` switch, which
// every CSS media query and matchMedia call then follows. Any other
// outcome forces nothing, so Chromium's own reading of
// `enable-animations` keeps working as before; the opposite switch,
// which would override it, is never used.
//
// The answer is read once: a change to the setting takes effect on the
// next launch. Off Linux nothing is read — Chromium already follows
// Windows' "Animation effects" and macOS's Reduce motion.

const { execFile } = require("node:child_process");

const REDUCED_MOTION_SWITCH = "force-prefers-reduced-motion";

// The budget for the whole read. Each reader answers in about 20 ms on
// a GNOME session; the budget only matters for a session bus that is
// wedged, and is all such a launch can lose.
const READ_TIMEOUT_MS = 500;

const PORTAL = {
  service: "org.freedesktop.portal.Desktop",
  path: "/org/freedesktop/portal/desktop",
  iface: "org.freedesktop.portal.Settings",
  namespace: "org.freedesktop.appearance",
  key: "reduced-motion",
};

// Tried in order; the first that answers decides.
const PORTAL_READERS = [
  {
    cmd: "busctl",
    args: [
      "--user",
      "call",
      PORTAL.service,
      PORTAL.path,
      PORTAL.iface,
      "ReadOne",
      "ss",
      PORTAL.namespace,
      PORTAL.key,
    ],
  },
  {
    cmd: "gdbus",
    args: [
      "call",
      "--session",
      "--dest",
      PORTAL.service,
      "--object-path",
      PORTAL.path,
      "--method",
      `${PORTAL.iface}.ReadOne`,
      PORTAL.namespace,
      PORTAL.key,
    ],
  },
];

const GSETTINGS_READER = {
  cmd: "gsettings",
  args: ["get", "org.gnome.desktop.a11y.interface", "reduced-motion"],
};

// The portal's value: 0 is no preference, 1 is reduce.
const PORTAL_VALUES = { 0: "no-preference", 1: "reduce" };

/**
 * The portal's answer from busctl's `v u N` or gdbus's
 * `(<uint32 N>,)`, or `null` for anything else.
 */
function parsePortalAnswer(out) {
  if (typeof out !== "string") return null;
  const match = /^(?:v u (\d+)|\(<uint32 (\d+)>,\))$/.exec(out.trim());
  if (!match) return null;
  return PORTAL_VALUES[match[1] ?? match[2]] ?? null;
}

/** gsettings' `'reduce'` or `'no-preference'`, or `null`. */
function parseGsettingsAnswer(out) {
  if (typeof out !== "string") return null;
  const match = /^'(reduce|no-preference)'$/.exec(out.trim());
  return match ? match[1] : null;
}

/** Whether an answer asks for Chromium's reduced-motion switch. */
function forcesReducedMotion(answer) {
  return answer === "reduce";
}

/** Run `reader` within what is left of the budget; `null` on failure. */
async function ask(reader, parse, exec, deadline) {
  const remaining = deadline - Date.now();
  if (remaining <= 0) return null;
  try {
    return parse(await exec(reader.cmd, reader.args, { timeoutMs: remaining }));
  } catch {
    return null;
  }
}

async function readChain(exec, deadline) {
  for (const reader of PORTAL_READERS) {
    const answer = await ask(reader, parsePortalAnswer, exec, deadline);
    if (answer !== null) return answer;
  }
  return ask(GSETTINGS_READER, parseGsettingsAnswer, exec, deadline);
}

/**
 * The desktop's reduced-motion preference: "reduce", "no-preference",
 * or `null` when nothing answered in time. Never rejects, and settles
 * within `timeoutMs` whatever the readers do.
 */
function readReducedMotion({ platform, exec, timeoutMs = READ_TIMEOUT_MS }) {
  if (platform !== "linux") return Promise.resolve(null);
  const deadline = Date.now() + timeoutMs;
  let timer;
  const expiry = new Promise((resolve) => {
    timer = setTimeout(() => resolve(null), timeoutMs);
  });
  const read = readChain(exec, deadline).catch(() => null);
  return Promise.race([read, expiry]).finally(() => clearTimeout(timer));
}

/**
 * Read the preference and, when it is "reduce", append Chromium's
 * switch. Resolves whether the switch was appended. Must run before
 * the first window is created: a window takes the value it opens with.
 */
async function applyReducedMotion({
  platform,
  exec,
  appendSwitch,
  timeoutMs = READ_TIMEOUT_MS,
}) {
  const answer = await readReducedMotion({ platform, exec, timeoutMs });
  if (!forcesReducedMotion(answer)) return false;
  appendSwitch(REDUCED_MOTION_SWITCH);
  return true;
}

/**
 * Run a command and resolve its stdout. Rejects on a spawn failure, a
 * non-zero exit, or a run past `timeoutMs`, which kills the command.
 */
function execText(cmd, args, { timeoutMs }) {
  return new Promise((resolve, reject) => {
    execFile(
      cmd,
      args,
      { timeout: timeoutMs, killSignal: "SIGKILL", encoding: "utf8", windowsHide: true },
      (err, stdout) => (err ? reject(err) : resolve(stdout)),
    );
  });
}

module.exports = {
  READ_TIMEOUT_MS,
  REDUCED_MOTION_SWITCH,
  applyReducedMotion,
  execText,
  forcesReducedMotion,
  parseGsettingsAnswer,
  parsePortalAnswer,
  readReducedMotion,
};
