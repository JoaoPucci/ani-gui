// The sandbox probe: the AppImage's AppRun starts this app's own binary
// once with SANDBOX_PROBE_SWITCH before the real launch, to learn
// whether Chromium's sandbox starts for it. Chromium sets its sandbox up
// before main.js runs, so a run that reaches main.js has already passed
// that point; main.js answers with the marker and exits, and AppRun
// starts the app sandboxed. A run that does not reach it — the sandbox
// aborted the start — gets no answer, and AppRun starts the app with
// `--no-sandbox` instead.

const test = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const {
  SANDBOX_PROBE_SWITCH,
  SANDBOX_PROBE_ANSWER,
  answerSandboxProbe,
} = require("./sandbox-probe.cjs");

function recorder() {
  const calls = { written: "", exits: [] };
  return {
    calls,
    out: { write: (s) => { calls.written += s; } },
    exit: (code) => calls.exits.push(code),
  };
}

test("a run started as the probe answers and exits 0", () => {
  const r = recorder();
  const answered = answerSandboxProbe(["/app/ani-gui.bin", "--enable-wayland-ime", SANDBOX_PROBE_SWITCH], r.out, r.exit);
  assert.equal(answered, true);
  assert.equal(r.calls.written, `${SANDBOX_PROBE_ANSWER}\n`);
  assert.deepEqual(r.calls.exits, [0]);
});

test("an ordinary launch is left alone", () => {
  const r = recorder();
  const answered = answerSandboxProbe(["/app/ani-gui.bin", "--enable-wayland-ime", "ani-gui://x"], r.out, r.exit);
  assert.equal(answered, false);
  assert.equal(r.calls.written, "");
  assert.deepEqual(r.calls.exits, []);
});

// The probe is a switch, never an environment variable: a variable
// exported in the user's session would reach every launch, and the app
// would quit silently each time it was started.
test("only the exact switch is the probe", () => {
  for (const arg of [`${SANDBOX_PROBE_SWITCH}=1`, "--ani-gui-sandbox", SANDBOX_PROBE_SWITCH.toUpperCase()]) {
    const r = recorder();
    assert.equal(answerSandboxProbe(["/app/ani-gui.bin", arg], r.out, r.exit), false, arg);
    assert.deepEqual(r.calls.exits, [], arg);
  }
});

// The answer means the sandbox has been set up, so main.js has to give
// it before it does anything of its own: before the single-instance
// claim (a probe would otherwise hand over to a running app, or hold the
// lock the real launch then waits on), the backend, a window or the
// profile. Run under plain Node, main.js's first require of Electron
// yields a path string and its first Electron call throws; a main.js
// that answers first never gets that far.
test("main.js answers the probe before it touches Electron", () => {
  const main = path.join(__dirname, "..", "main.js");
  const run = spawnSync(process.execPath, [main, SANDBOX_PROBE_SWITCH], {
    encoding: "utf8",
    env: { PATH: process.env.PATH },
  });
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.stdout, `${SANDBOX_PROBE_ANSWER}\n`);
});
