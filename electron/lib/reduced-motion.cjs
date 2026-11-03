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
// So the main process asks the portal itself — through busctl, and
// again through gdbus when busctl gives no answer — and falls back to
// the gsettings key when the portal cannot answer. When the answer is
// "reduce" it hands Chromium its own `--force-prefers-reduced-motion`
// switch, which every CSS media query and matchMedia call in a window
// opened afterwards follows. Any other outcome forces nothing, so
// Chromium's own reading of `enable-animations` keeps working as
// before; the opposite switch, which would override it, is never used.
//
// The answer is read once: a change to the setting takes effect on the
// next launch. Off Linux nothing is read — on Windows Chromium already
// follows the system's "Animation effects" setting.

const { execFile } = require("node:child_process");

const REDUCED_MOTION_SWITCH = "force-prefers-reduced-motion";

// The budget for the whole read. Each reader answers in about 20 ms on
// a GNOME session; the budget only matters for a session bus that is
// wedged, and is all such a launch can lose.
const READ_TIMEOUT_MS = 500;

// The wall clock the budget runs on, unless the caller supplies one.
// Wrapped in arrows so the methods work however they are called: a
// bare `setTimeout` lifted onto an object throws "Illegal invocation"
// in a renderer, though not in the Node main process this runs in.
const SYSTEM_CLOCK = {
  now: () => Date.now(),
  setTimeout: (fn, ms) => setTimeout(fn, ms),
  clearTimeout: (id) => clearTimeout(id),
};

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

/**
 * The environment the readers run in: the app's own, without DISPLAY.
 * None of them draws anything, and with no session bus to find, gdbus
 * would otherwise fall back to D-Bus's X11 autolaunch and start a
 * dbus-daemon that outlives the reader.
 */
function readerEnv(env) {
  const { DISPLAY: _display, ...rest } = env;
  return rest;
}

/** Run `reader` within what is left of the budget; `null` on failure. */
async function ask(reader, parse, { exec, deadline, env, clock }) {
  const remaining = deadline - clock.now();
  // Also the guard against a zero timeout, which execFile reads as none.
  if (remaining <= 0) return null;
  try {
    return parse(await exec(reader.cmd, reader.args, { timeoutMs: remaining, env }));
  } catch {
    return null;
  }
}

async function readChain(run) {
  for (const reader of PORTAL_READERS) {
    const answer = await ask(reader, parsePortalAnswer, run);
    if (answer !== null) return answer;
  }
  return ask(GSETTINGS_READER, parseGsettingsAnswer, run);
}

/**
 * The desktop's reduced-motion preference: "reduce", "no-preference",
 * or `null` when nothing answered in time. Never rejects, and settles
 * within `timeoutMs` of `clock` whatever the readers do.
 */
function readReducedMotion({
  platform,
  exec,
  timeoutMs = READ_TIMEOUT_MS,
  env = process.env,
  clock = SYSTEM_CLOCK,
}) {
  if (platform !== "linux") return Promise.resolve(null);
  const deadline = clock.now() + timeoutMs;
  let timer;
  const expiry = new Promise((resolve) => {
    timer = clock.setTimeout(() => resolve(null), timeoutMs);
  });
  const read = readChain({ exec, deadline, env: readerEnv(env), clock }).catch(() => null);
  return Promise.race([read, expiry]).finally(() => clock.clearTimeout(timer));
}

/**
 * Read the preference and, when it is "reduce", append Chromium's
 * switch. Resolves whether the switch was appended, and never rejects:
 * a switch that cannot be appended is the same as no answer. Must run
 * before the first window is created: a window takes the value it
 * opens with.
 */
async function applyReducedMotion({
  platform,
  exec,
  appendSwitch,
  timeoutMs = READ_TIMEOUT_MS,
  env = process.env,
  clock = SYSTEM_CLOCK,
}) {
  const answer = await readReducedMotion({ platform, exec, timeoutMs, env, clock });
  if (!forcesReducedMotion(answer)) return false;
  try {
    appendSwitch(REDUCED_MOTION_SWITCH);
    return true;
  } catch {
    return false;
  }
}

/**
 * Run a command in `env` (the app's own when left out) and resolve its
 * stdout. Rejects on a spawn failure, a non-zero exit, or a run past
 * `timeoutMs`, which kills the command.
 */
function execText(cmd, args, { timeoutMs, env }) {
  return new Promise((resolve, reject) => {
    execFile(
      cmd,
      args,
      { timeout: timeoutMs, killSignal: "SIGKILL", encoding: "utf8", windowsHide: true, env },
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
