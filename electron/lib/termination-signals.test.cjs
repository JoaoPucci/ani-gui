// The Linux stop-signal takeover: a signal to the main process alone
// quits the app the ordinary way; a signal that reached the app's
// helpers too ends it by that signal, after its storage is written
// out, instead of letting Chromium abort with a SIGTRAP.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const {
  HELPER_WATCH_INTERVAL_MS,
  HELPER_WATCH_WINDOW_MS,
  STORAGE_COMMIT_WAIT_MS,
  TERMINATION_SIGNALS,
  blockFor,
  exitedGpuProcess,
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
    platform: "linux",
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

test("procStatState reads the state whatever the command name holds", () => {
  const comm = fc.string({ maxLength: 16 }).filter((name) => !name.includes("\n"));
  fc.assert(
    fc.property(
      fc.integer({ min: 1, max: 4194304 }),
      comm,
      fc.constantFrom("R", "S", "D", "Z", "T", "X"),
      (pid, name, state) => procStatState(`${pid} (${name}) ${state} 1 2 3`) === state,
    ),
  );
});

test("exitedGpuProcess counts the GPU process only", () => {
  // Utility processes come and go during an ordinary quit, and a
  // renderer exits with its window: only the GPU process's death makes
  // Chromium abort. The zygote is not counted: app.getAppMetrics()
  // does not list it (Electron 33, measured), so nothing would see it.
  const gone = () => null;
  const helpers = [
    { pid: 100, type: "Browser" },
    { pid: 300, type: "Tab" },
    { pid: 201, type: "Utility" },
    { pid: 202, type: "Zygote" },
    { pid: 203, type: "Sandbox helper" },
  ];
  assert.equal(exitedGpuProcess(helpers, 100, gone), null);
  assert.equal(exitedGpuProcess(helpers, 100, (pid) => zombie(pid)), null);
});

test("exitedGpuProcess names the GPU process once it is gone or a zombie", () => {
  const helpers = [
    { pid: 201, type: "Utility" },
    { pid: 200, type: "GPU" },
  ];
  assert.equal(exitedGpuProcess(helpers, MAIN, running), null);
  assert.equal(exitedGpuProcess(helpers, MAIN, () => null), "GPU process 200 is gone");
  assert.equal(exitedGpuProcess(helpers, MAIN, (pid) => zombie(pid)), "GPU process 200 exited");
  assert.equal(
    exitedGpuProcess(helpers, MAIN, (pid) => `${pid} (x) X 1`),
    "GPU process 200 exited",
  );
});

test("exitedGpuProcess names the GPU process exactly when it has exited", () => {
  const types = fc.constantFrom("Browser", "Tab", "Utility", "Zygote", "Sandbox helper", "GPU", "Unknown");
  const helper = fc.record({ pid: fc.integer({ min: 1, max: 4194304 }), type: types });
  const states = fc.constantFrom("gone", "R", "S", "Z", "X");
  fc.assert(
    fc.property(fc.array(helper, { maxLength: 8 }), states, fc.integer({ min: 1, max: 4194304 }), (helpers, st, mainPid) => {
      const read = (pid) => (st === "gone" ? null : `${pid} (ani-gui.bin) ${st} 1 2 3`);
      const named = exitedGpuProcess(helpers, mainPid, read);
      const counted = helpers.filter((h) => h.pid !== mainPid && h.type === "GPU");
      const exited = st === "gone" || st === "Z" || st === "X";
      if (!exited || counted.length === 0) return named === null;
      return named === `${counted[0].type} process ${counted[0].pid} ${st === "gone" ? "is gone" : "exited"}`;
    }),
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
  // One watch at a time, the latest signal's.
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

test("a utility process that exits within the window leaves the ordinary quit", () => {
  const { state, options, send, advance } = rig({
    helpers: [
      { pid: MAIN, type: "Browser" },
      { pid: 200, type: "GPU" },
      { pid: 201, type: "Utility" },
    ],
  });
  installTerminationSignals(options);
  send("SIGTERM");
  advance(20);
  state.stats.set(201, zombie(201));
  advance(HELPER_WATCH_WINDOW_MS);
  state.stats.delete(201);
  send("SIGTERM");
  assert.deepEqual(state.raised, []);
  assert.deepEqual(state.quits, ["SIGTERM", "SIGTERM"]);
});

test("a signal after the window opens a window of its own", () => {
  // The first signal's quit may be held (a slow quit, the boot-failure
  // dialog); a stop to every process that follows still has to be seen
  // as one, and its GPU process usually dies just after the signal.
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  assert.equal(state.timers.size, 0);
  send("SIGHUP");
  advance(20);
  state.stats.delete(200);
  advance(HELPER_WATCH_INTERVAL_MS);
  assert.deepEqual(state.ended, [{ signal: "SIGHUP", reason: "GPU process 200 is gone" }]);
  assert.deepEqual(state.raised, ["SIGHUP"]);
});

test("child-process-gone after a later signal counts in that signal's window", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  send("SIGINT");
  advance(10);
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  assert.deepEqual(state.raised, ["SIGINT"]);
});

test("a signal late in an earlier one's window gets a full window, and the app ends by it", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS - 10);
  send("SIGHUP");
  advance(20);
  state.stats.delete(200);
  advance(HELPER_WATCH_INTERVAL_MS);
  assert.deepEqual(state.ended, [{ signal: "SIGHUP", reason: "GPU process 200 is gone" }]);
  assert.deepEqual(state.raised, ["SIGHUP"]);
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  assert.equal(state.timers.size, 0);
});

test("child-process-gone in a late signal's window ends the app by that signal", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS - 10);
  send("SIGINT");
  advance(30);
  takeover.childProcessGone({ type: "GPU", reason: "crashed" });
  assert.deepEqual(state.ended, [{ signal: "SIGINT", reason: "GPU process crashed" }]);
});

test("a GPU process that died just before the signal counts for it", () => {
  // Chromium may have handled the GPU process's death, its host gone
  // from the metrics, before the signal's handler runs.
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  state.helpers = state.helpers.filter((h) => h.type !== "GPU");
  advance(10);
  send("SIGTERM");
  assert.deepEqual(state.ended, [{ signal: "SIGTERM", reason: "GPU process killed" }]);
  assert.deepEqual(state.raised, ["SIGTERM"]);
});

test("a GPU process death long before the signal, or a clean one, does not count for it", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  takeover.childProcessGone({ type: "GPU", reason: "crashed" });
  advance(HELPER_WATCH_WINDOW_MS + 10);
  takeover.childProcessGone({ type: "GPU", reason: "clean-exit" });
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  assert.deepEqual(state.raised, []);
  assert.deepEqual(state.quits, ["SIGTERM"]);
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
  takeover.childProcessGone({ type: "GPU", reason: "killed" });
  assert.deepEqual(state.ended, [{ signal: "SIGTERM", reason: "GPU process killed" }]);
  assert.deepEqual(state.raised, ["SIGTERM"]);
});

test("child-process-gone counts only the GPU process's unclean exit, in the window", () => {
  const { state, options, send, advance } = rig();
  const takeover = installTerminationSignals(options);
  // No signal near it: a GPU crash on its own is Chromium's to recover from.
  takeover.childProcessGone({ type: "GPU", reason: "crashed" });
  advance(HELPER_WATCH_WINDOW_MS + 10);
  assert.deepEqual(state.raised, []);
  send("SIGTERM");
  takeover.childProcessGone({ type: "Tab", reason: "killed" });
  takeover.childProcessGone({ type: "Utility", reason: "clean-exit" });
  takeover.childProcessGone({ type: "Utility", reason: "killed" });
  takeover.childProcessGone({ type: "Zygote", reason: "killed" });
  // The GPU process's host torn down cleanly is the quit's own doing.
  takeover.childProcessGone({ type: "GPU", reason: "clean-exit" });
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

test("once the session is ending, a stop signal ends the app by it at once", () => {
  // A logout's stop of the app's scope can overtake the quit its
  // announcement started; by then the GPU process may already be gone
  // from the metrics, and nothing is left to look at. The signal is the
  // session's end itself.
  const { state, options, send } = rig();
  const takeover = installTerminationSignals(options);
  takeover.sessionEnding();
  send("SIGTERM");
  assert.deepEqual(state.ended, [{ signal: "SIGTERM", reason: "the session is ending" }]);
  assert.deepEqual(state.raised, ["SIGTERM"]);
  assert.deepEqual(state.quits, []);
});

test("before the session's end is announced, a stop signal alone still quits", () => {
  const { state, options, send, advance } = rig();
  installTerminationSignals(options);
  send("SIGTERM");
  advance(HELPER_WATCH_WINDOW_MS + 10);
  assert.deepEqual(state.raised, []);
});

test("off Linux the signals are left to Electron", () => {
  for (const platform of ["win32", "darwin"]) {
    const { state, options, send } = rig();
    options.platform = platform;
    const takeover = installTerminationSignals(options);
    takeover.sessionEnding();
    assert.deepEqual(state.listeners, {});
    state.stats.delete(200);
    send("SIGTERM");
    takeover.childProcessGone({ type: "GPU", reason: "killed" });
    assert.deepEqual(state.raised, []);
    assert.deepEqual(state.quits, []);
    assert.deepEqual(state.timers.size, 0);
  }
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
