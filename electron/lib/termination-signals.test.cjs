// The Linux stop-signal takeover: a signal to the main process alone
// quits the app the ordinary way; a signal that reached the app's
// helpers too ends it by that signal, after its storage is written
// out, instead of letting Chromium abort with a SIGTRAP.

const test = require("node:test");
const assert = require("node:assert/strict");

const {
  HELPER_WATCH_INTERVAL_MS,
  HELPER_WATCH_WINDOW_MS,
  STORAGE_COMMIT_WAIT_MS,
  TERMINATION_SIGNALS,
  blockFor,
  exitedHelper,
  installTerminationSignals,
  persistStorage,
  procStatState,
} = require("./termination-signals.cjs");

const MAIN = 100;
const running = (pid) => `${pid} (ani-gui.bin) S 1 2 3`;
const zombie = (pid) => `${pid} (ani-gui.bin) Z 1 2 3`;

/** A fake process, clock and helper table, and what the takeover did. */
function rig({ helpers = [{ pid: MAIN, type: "Browser" }, { pid: 200, type: "GPU" }] } = {}) {
  const state = {
    listeners: {},
    removed: [],
    raised: [],
    quits: [],
    ended: [],
    log: [],
    stats: new Map(helpers.map((h) => [h.pid, running(h.pid)])),
    helpers,
    clock: 0,
    timers: new Map(),
    nextTimer: 1,
  };
  const options = {
    source: {
      on: (signal, fn) => {
        (state.listeners[signal] ||= []).push(fn);
      },
      removeAllListeners: (signal) => {
        state.removed.push(signal);
        state.log.push(`remove ${signal}`);
        delete state.listeners[signal];
      },
    },
    mainPid: MAIN,
    listHelpers: () => state.helpers,
    readProcStat: (pid) => (state.stats.has(pid) ? state.stats.get(pid) : null),
    requestQuit: (signal) => state.quits.push(signal),
    beforeEnd: (signal, reason) => {
      state.ended.push({ signal, reason });
      state.log.push(`beforeEnd ${signal}`);
    },
    raise: (signal) => {
      state.raised.push(signal);
      state.log.push(`raise ${signal}`);
    },
    setInterval: (fn, ms) => {
      const id = state.nextTimer++;
      state.timers.set(id, { fn, ms });
      return id;
    },
    clearInterval: (id) => state.timers.delete(id),
    now: () => state.clock,
  };
  const send = (signal) => {
    for (const fn of state.listeners[signal] || []) fn();
  };
  /** Advance the clock in interval steps, firing the watch each step. */
  const advance = (ms) => {
    for (let t = 0; t < ms; t += HELPER_WATCH_INTERVAL_MS) {
      state.clock += HELPER_WATCH_INTERVAL_MS;
      for (const { fn } of [...state.timers.values()]) fn();
    }
  };
  return { state, options, send, advance };
}

test("procStatState reads the state letter after the last ')'", () => {
  assert.equal(procStatState("12 (ani-gui.bin) S 1 2"), "S");
  assert.equal(procStatState("12 (we ird) (name)) Z 1 2"), "Z");
  assert.equal(procStatState("garbage"), null);
  assert.equal(procStatState("12 (x) "), null);
  assert.equal(procStatState(null), null);
});

test("exitedHelper skips the main process and renderers", () => {
  const stats = { 100: null, 300: null };
  const read = (pid) => (pid in stats ? stats[pid] : running(pid));
  const helpers = [
    { pid: 100, type: "Browser" },
    { pid: 300, type: "Tab" },
    { pid: 200, type: "GPU" },
  ];
  assert.equal(exitedHelper(helpers, 100, read), null);
});

test("exitedHelper names a helper that is gone or a zombie", () => {
  const helpers = [
    { pid: 200, type: "GPU" },
    { pid: 201, type: "Utility" },
  ];
  assert.equal(exitedHelper(helpers, MAIN, (pid) => (pid === 201 ? null : running(pid))), "Utility process 201 is gone");
  assert.equal(exitedHelper(helpers, MAIN, (pid) => zombie(pid)), "GPU process 200 exited");
  assert.equal(
    exitedHelper(helpers, MAIN, (pid) => `${pid} (x) X 1`),
    "GPU process 200 exited",
  );
});

test("the takeover listens for SIGTERM, SIGINT and SIGHUP", () => {
  const { state, options } = rig();
  installTerminationSignals(options);
  assert.deepEqual(Object.keys(state.listeners).sort(), [...TERMINATION_SIGNALS].sort());
  assert.deepEqual(TERMINATION_SIGNALS, ["SIGTERM", "SIGINT", "SIGHUP"]);
});

test("a signal to the main process alone asks for the ordinary quit", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 50);
  assert.deepEqual(state.quits, ["SIGTERM"]);
  assert.deepEqual(state.raised, []);
  assert.deepEqual(state.ended, []);
  // The watch stops once its window has passed.
  assert.equal(state.timers.size, 0);
});

test("a second signal asks for the quit again instead of killing the app", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(20);
  send("SIGTERM");
  send("SIGINT");
  assert.deepEqual(state.quits, ["SIGTERM", "SIGTERM", "SIGINT"]);
  assert.deepEqual(state.raised, []);
  // One watch, started by the first signal.
  assert.equal(state.timers.size, 1);
});

test("a signal that finds a helper gone ends the app by that signal", () => {
  const { state, options, send } = rig();
  installTerminationSignals(options);
  state.stats.set(200, zombie(200));
  send("SIGHUP");
  assert.deepEqual(state.quits, []);
  assert.deepEqual(state.ended, [{ signal: "SIGHUP", reason: "GPU process 200 exited" }]);
  assert.deepEqual(state.raised, ["SIGHUP"]);
});

test("the app's storage is written and its listeners removed before the raise", () => {
  const { state, options, send } = rig();
  installTerminationSignals(options);
  state.stats.delete(200);
  send("SIGTERM");
  assert.deepEqual(state.log, [
    "beforeEnd SIGTERM",
    "remove SIGTERM",
    "remove SIGINT",
    "remove SIGHUP",
    "raise SIGTERM",
  ]);
});

test("a helper that dies within the window ends the app by the first signal", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGINT");
  advance(20);
  state.stats.delete(200);
  advance(HELPER_WATCH_INTERVAL_MS);
  assert.deepEqual(state.raised, ["SIGINT"]);
  assert.equal(state.ended[0].reason, "GPU process 200 is gone");
  assert.equal(state.timers.size, 0);
});

test("a helper that exits after the window is the ordinary quit's doing", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  state.stats.set(200, zombie(200));
  advance(50);
  assert.deepEqual(state.raised, []);
});

test("a later signal that finds a helper gone still ends the app", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  state.stats.delete(200);
  send("SIGTERM");
  assert.deepEqual(state.raised, ["SIGTERM"]);
});

test("child-process-gone within the window ends the app", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  send("SIGTERM");
  advance(10);
  takeover.childProcessGone({ type: "Utility", reason: "killed" });
  assert.deepEqual(state.ended, [{ signal: "SIGTERM", reason: "Utility process killed" }]);
  assert.deepEqual(state.raised, ["SIGTERM"]);
});

test("child-process-gone counts only a non-renderer's unclean exit in the window", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  // No signal yet: a GPU crash on its own is Chromium's to recover from.
  takeover.childProcessGone({ type: "GPU", reason: "crashed" });
  send("SIGTERM");
  takeover.childProcessGone({ type: "Tab", reason: "killed" });
  takeover.childProcessGone({ type: "Utility", reason: "clean-exit" });
  advance(HELPER_WATCH_WINDOW_MS + 10);
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  assert.deepEqual(state.raised, []);
});

test("the app ends once, however many signals and helper exits follow", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  state.stats.delete(200);
  send("SIGTERM");
  send("SIGINT");
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  advance(50);
  assert.deepEqual(state.raised, ["SIGTERM"]);
  assert.equal(state.ended.length, 1);
});

test("a failing beforeEnd still ends the app by the signal", () => {
  const { state, options, send } = rig();
  options.beforeEnd = () => {
    throw new Error("backend stop failed");
  };
  installTerminationSignals(options);
  state.stats.delete(200);
  assert.throws(() => send("SIGTERM"), /backend stop failed/);
  assert.deepEqual(state.removed, ["SIGTERM", "SIGINT", "SIGHUP"]);
  assert.deepEqual(state.raised, ["SIGTERM"]);
});

test("a helper table that cannot be read leaves the ordinary quit", () => {
  const { state, options, send } = rig();
  options.listHelpers = () => {
    throw new Error("not ready");
  };
  installTerminationSignals(options);
  send("SIGTERM");
  assert.deepEqual(state.quits, ["SIGTERM"]);
  assert.deepEqual(state.raised, []);
});

test("persistStorage asks for the commit, then holds the thread for the wait", () => {
  const calls = [];
  persistStorage({ flush: () => calls.push("flush"), block: (ms) => calls.push(`block ${ms}`) });
  assert.deepEqual(calls, ["flush", `block ${STORAGE_COMMIT_WAIT_MS}`]);
  assert.equal(STORAGE_COMMIT_WAIT_MS, 100);
});

test("persistStorage still waits when the flush throws", () => {
  const calls = [];
  assert.throws(() =>
    persistStorage({
      flush: () => {
        throw new Error("no session");
      },
      block: (ms) => calls.push(ms),
      waitMs: 7,
    }),
  );
  assert.deepEqual(calls, [7]);
});

test("blockFor holds the thread for about the time asked", () => {
  const start = process.hrtime.bigint();
  blockFor(30);
  const ms = Number(process.hrtime.bigint() - start) / 1e6;
  assert.ok(ms >= 29, `held ${ms} ms`);
  assert.ok(ms < 1000, `held ${ms} ms`);
});
