"use strict";

// The sandbox probe: the AppImage's AppRun starts this app's own binary
// once with SANDBOX_PROBE_SWITCH before the real launch, to learn
// whether Chromium's sandbox starts for it on this host.
//
// The image cannot use Chromium's setuid helper (it runs from a
// read-only FUSE mount, so `chrome-sandbox` cannot be SUID), which
// leaves the namespace sandbox. Whether that one starts depends on the
// host — unprivileged user namespaces, and on Ubuntu 23.10+ an AppArmor
// restriction whose exceptions are granted per binary — and a sandbox
// that fails to start aborts the launch. Chromium makes that decision
// before main.js runs, so a run that reaches main.js has passed it:
// main.js calls answerSandboxProbe first, which prints the answer and
// exits. AppRun starts the app sandboxed when it gets the answer, and
// with `--no-sandbox` when it does not. The appimage-tree.cjs repack
// writes the AppRun side from these same constants.
//
// A switch and never an environment variable: a variable exported in
// the user's session would reach every launch and the app would quit
// silently each time it was started.

/** The switch that starts the app's binary as the sandbox probe. */
const SANDBOX_PROBE_SWITCH = "--ani-gui-sandbox-probe";

/** The line a probe run prints on stdout when it reached main.js. */
const SANDBOX_PROBE_ANSWER = "ani-gui-sandbox-probe-ok";

/**
 * Answers the probe when `argv` carries the switch: writes the answer
 * line to `out` and exits 0 through `exit`. Returns whether it did, so
 * a caller whose `exit` returns stops there too.
 */
function answerSandboxProbe(argv, out, exit) {
  if (!argv.includes(SANDBOX_PROBE_SWITCH)) return false;
  out.write(`${SANDBOX_PROBE_ANSWER}\n`);
  exit(0);
  return true;
}

module.exports = { SANDBOX_PROBE_SWITCH, SANDBOX_PROBE_ANSWER, answerSandboxProbe };
