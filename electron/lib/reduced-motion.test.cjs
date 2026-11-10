// The desktop's reduced-motion preference, read at launch on Linux.
//
// Chromium takes `prefers-reduced-motion` on Linux from GTK's
// `gtk-enable-animations` alone. GNOME's Reduced Motion switch
// (Accessibility › Seeing) writes a different key, which the desktop
// portal publishes as `org.freedesktop.appearance reduced-motion`, so
// the main process reads it and forces the media query when it says
// "reduce". Every reader runs through a fake exec here: no test reads
// or changes the desktop it runs on.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const {
  REDUCED_MOTION_SWITCH,
  applyReducedMotion,
  execText,
  forcesReducedMotion,
  parseGsettingsAnswer,
  parsePortalAnswer,
  readReducedMotion,
} = require("./reduced-motion.cjs");

const PORTAL_REDUCE = { busctl: "v u 1\n" };
const HANG = Symbol("hang");

/**
 * An exec answering from `outcomes`, keyed by command name: a string
 * is the command's stdout, an Error its failure, HANG a command that
 * never returns. A command with no entry fails as a missing binary.
 */
function fakeExec(outcomes) {
  const calls = [];
  const exec = (cmd, args, options) => {
    calls.push({ cmd, args, options });
    const outcome = Object.hasOwn(outcomes, cmd)
      ? outcomes[cmd]
      : Object.assign(new Error(`spawn ${cmd} ENOENT`), { code: "ENOENT" });
    if (outcome === HANG) return new Promise(() => {});
    if (outcome instanceof Error) return Promise.reject(outcome);
    return Promise.resolve(outcome);
  };
  return { exec, calls };
}

/**
 * A clock the test drives: `now` moves only when `advance` is called,
 * which fires the timers it passes, in the order they were set; one set
 * by a firing timer waits for the next `advance`. No test here reads the wall clock,
 * so the budget arithmetic comes out the same on any machine.
 */
function fakeClock() {
  let t = 1_000_000;
  let nextId = 1;
  const timers = new Map();
  return {
    now: () => t,
    setTimeout: (fn, ms) => {
      const id = nextId++;
      timers.set(id, { at: t + ms, fn });
      return id;
    },
    clearTimeout: (id) => {
      timers.delete(id);
    },
    advance(ms) {
      t += ms;
      for (const [id, timer] of [...timers]) {
        if (timer.at <= t) {
          timers.delete(id);
          timer.fn();
        }
      }
    },
    pending: () => timers.size,
  };
}

/** Let every queued promise reaction run. */
const flush = () => new Promise((resolve) => setImmediate(resolve));

async function apply(outcomes, { platform = "linux", timeoutMs = 200 } = {}) {
  const { exec, calls } = fakeExec(outcomes);
  const appended = [];
  const result = await applyReducedMotion({
    platform,
    exec,
    appendSwitch: (name) => appended.push(name),
    timeoutMs,
    clock: fakeClock(),
  });
  return { result, appended, calls };
}

test("the switch is Chromium's own and forces reduce, never the opposite", () => {
  assert.equal(REDUCED_MOTION_SWITCH, "force-prefers-reduced-motion");
});

test("the portal saying reduce (1) forces reduced motion", async () => {
  const { result, appended } = await apply(PORTAL_REDUCE);
  assert.equal(result, true);
  assert.deepEqual(appended, ["force-prefers-reduced-motion"]);
});

test("the portal saying no preference (0) leaves Chromium's own reading alone", async () => {
  const { result, appended, calls } = await apply({
    busctl: "v u 0\n",
    gsettings: "'reduce'\n",
  });
  assert.equal(result, false);
  assert.deepEqual(appended, []);
  // An answer from the portal is the answer: gsettings is the fallback
  // for a portal that cannot say, not a second opinion.
  assert.deepEqual(
    calls.map((c) => c.cmd),
    ["busctl"],
  );
});

test("the portal is asked for org.freedesktop.appearance reduced-motion", async () => {
  const { calls } = await apply(PORTAL_REDUCE);
  assert.deepEqual(calls[0].args, [
    "--user",
    "call",
    "org.freedesktop.portal.Desktop",
    "/org/freedesktop/portal/desktop",
    "org.freedesktop.portal.Settings",
    "ReadOne",
    "ss",
    "org.freedesktop.appearance",
    "reduced-motion",
  ]);
});

test("when busctl gives no answer the portal is asked again through gdbus", async () => {
  const { result, appended, calls } = await apply({ gdbus: "(<uint32 1>,)\n" });
  assert.equal(result, true);
  assert.deepEqual(appended, ["force-prefers-reduced-motion"]);
  assert.deepEqual(calls[1].args, [
    "call",
    "--session",
    "--dest",
    "org.freedesktop.portal.Desktop",
    "--object-path",
    "/org/freedesktop/portal/desktop",
    "--method",
    "org.freedesktop.portal.Settings.ReadOne",
    "org.freedesktop.appearance",
    "reduced-motion",
  ]);
});

test("a portal that cannot answer falls back to GNOME's gsettings key", async () => {
  const notFound = new Error("Call failed: Requested setting not found");
  const { result, appended, calls } = await apply({
    busctl: notFound,
    gdbus: notFound,
    gsettings: "'reduce'\n",
  });
  assert.equal(result, true);
  assert.deepEqual(appended, ["force-prefers-reduced-motion"]);
  assert.deepEqual(calls.at(-1).args, [
    "get",
    "org.gnome.desktop.a11y.interface",
    "reduced-motion",
  ]);
});

test("gsettings saying no-preference forces nothing", async () => {
  const { result, appended } = await apply({ gsettings: "'no-preference'\n" });
  assert.equal(result, false);
  assert.deepEqual(appended, []);
});

test("when every reader fails, nothing is forced", async () => {
  const { result, appended, calls } = await apply({});
  assert.equal(result, false);
  assert.deepEqual(appended, []);
  assert.deepEqual(
    calls.map((c) => c.cmd),
    ["busctl", "gdbus", "gsettings"],
  );
});

test("a reader that never returns costs the budget and no more", { timeout: 5000 }, async () => {
  const clock = fakeClock();
  const { exec } = fakeExec({ busctl: HANG, gdbus: HANG, gsettings: HANG });
  const appended = [];
  let settled = false;
  const read = applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: (name) => appended.push(name),
    // Far longer than the test may run, so only the fake clock can be
    // what settles it.
    timeoutMs: 60_000,
    clock,
  }).then((result) => {
    settled = true;
    return result;
  });
  clock.advance(59_999);
  await flush();
  assert.equal(settled, false, "settled before the budget was spent");
  clock.advance(1);
  assert.equal(await read, false);
  assert.deepEqual(appended, []);
});

test("each reader is handed what is left of the budget as its own timeout", async () => {
  const clock = fakeClock();
  const calls = [];
  const exec = (cmd, _args, options) => {
    calls.push({ cmd, timeoutMs: options.timeoutMs });
    // busctl spends 80 ms of the budget before it fails.
    if (cmd === "busctl") clock.advance(80);
    return Promise.reject(new Error("failed"));
  };
  await applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: () => {},
    timeoutMs: 300,
    clock,
  });
  assert.deepEqual(calls, [
    { cmd: "busctl", timeoutMs: 300 },
    { cmd: "gdbus", timeoutMs: 220 },
    { cmd: "gsettings", timeoutMs: 220 },
  ]);
});

test("no reader is started once the budget is spent", { timeout: 5000 }, async () => {
  const clock = fakeClock();
  const calls = [];
  const exec = (cmd) => {
    calls.push(cmd);
    // busctl runs the budget out to the millisecond, then fails: what
    // follows would be handed a timeout of zero, which execFile reads
    // as none, so it must not start at all.
    clock.advance(300);
    return Promise.reject(new Error("failed"));
  };
  const result = await applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: () => {},
    timeoutMs: 300,
    clock,
  });
  await flush();
  assert.equal(result, false);
  assert.deepEqual(calls, ["busctl"]);
});

// The one test on the system clock, which is what a launch runs on. It
// asserts only that the read settles, never how long it took; a clock
// that waits far past its budget runs into the test's own timeout.
test("on the system clock a read that gets no answer still settles", { timeout: 5000 }, async () => {
  const { exec } = fakeExec({ busctl: HANG, gdbus: HANG, gsettings: HANG });
  assert.equal(await readReducedMotion({ platform: "linux", exec, timeoutMs: 20 }), null);
});

test("a read that answers early leaves no timer behind", async () => {
  const clock = fakeClock();
  const { exec } = fakeExec(PORTAL_REDUCE);
  await applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: () => {},
    timeoutMs: 10_000,
    clock,
  });
  assert.equal(clock.pending(), 0);
});

test("a reader that answers after the deadline is not acted on", { timeout: 5000 }, async () => {
  const clock = fakeClock();
  const appended = [];
  let release;
  const late = new Promise((resolve) => {
    release = resolve;
  });
  const read = applyReducedMotion({
    platform: "linux",
    exec: () => late,
    appendSwitch: (name) => appended.push(name),
    timeoutMs: 20,
    clock,
  });
  clock.advance(20);
  assert.equal(await read, false);
  release("v u 1\n");
  await flush();
  assert.deepEqual(appended, []);
});

test("an appendSwitch that throws is no answer, not a failed launch", async () => {
  const { exec } = fakeExec(PORTAL_REDUCE);
  const result = await applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: () => {
      throw new Error("switch refused");
    },
    timeoutMs: 200,
    clock: fakeClock(),
  });
  assert.equal(result, false);
});

test("readers run without DISPLAY, so a missing session bus is not autolaunched", async () => {
  const { exec, calls } = fakeExec({});
  await applyReducedMotion({
    platform: "linux",
    exec,
    appendSwitch: () => {},
    timeoutMs: 200,
    env: { DISPLAY: ":0", DBUS_SESSION_BUS_ADDRESS: "unix:path=/run/user/1/bus", PATH: "/usr/bin" },
    clock: fakeClock(),
  });
  assert.equal(calls.length, 3);
  for (const call of calls) {
    assert.deepEqual(call.options.env, {
      DBUS_SESSION_BUS_ADDRESS: "unix:path=/run/user/1/bus",
      PATH: "/usr/bin",
    });
  }
});

test("garbage from every reader is no answer", async () => {
  for (const garbage of ["", "hello", "v u 2\n", "v s \"reduce\"\n", "(<uint32 1>,) and more"]) {
    const { result, appended } = await apply({
      busctl: garbage,
      gdbus: garbage,
      gsettings: garbage,
    });
    assert.equal(result, false, JSON.stringify(garbage));
    assert.deepEqual(appended, []);
  }
});

test("garbage from the portal falls through to gsettings", async () => {
  const { result } = await apply({
    busctl: "v u 9\n",
    gdbus: "nonsense",
    gsettings: "'reduce'\n",
  });
  assert.equal(result, true);
});

test("off Linux nothing is read and nothing is forced", async () => {
  for (const platform of ["win32", "darwin"]) {
    const { result, appended, calls } = await apply(PORTAL_REDUCE, { platform });
    assert.equal(result, false);
    assert.deepEqual(appended, []);
    assert.deepEqual(calls, [], platform);
  }
});

test("readReducedMotion reports the answer it acted on", async () => {
  const { exec } = fakeExec({ busctl: "v u 0\n" });
  assert.equal(
    await readReducedMotion({ platform: "linux", exec, timeoutMs: 100, clock: fakeClock() }),
    "no-preference",
  );
  assert.equal(
    await readReducedMotion({
      platform: "linux",
      exec: fakeExec({}).exec,
      timeoutMs: 100,
      clock: fakeClock(),
    }),
    null,
  );
});

test("the portal's two output dialects both parse", () => {
  assert.equal(parsePortalAnswer("v u 1\n"), "reduce");
  assert.equal(parsePortalAnswer("v u 0\n"), "no-preference");
  assert.equal(parsePortalAnswer("(<uint32 1>,)\n"), "reduce");
  assert.equal(parsePortalAnswer("(<uint32 0>,)\n"), "no-preference");
  assert.equal(parsePortalAnswer("(<uint32 2>,)\n"), null);
});

test("gsettings' enum parses", () => {
  assert.equal(parseGsettingsAnswer("'reduce'\n"), "reduce");
  assert.equal(parseGsettingsAnswer("'no-preference'\n"), "no-preference");
  assert.equal(parseGsettingsAnswer("reduce"), null);
});

// ── properties ────────────────────────────────────────────────────

const ANSWERS = ["reduce", "no-preference", null];

test("property: the decision forces exactly when the answer is reduce", () => {
  fc.assert(
    fc.property(fc.oneof(fc.constantFrom(...ANSWERS), fc.anything()), (answer) => {
      assert.equal(forcesReducedMotion(answer), answer === "reduce");
    }),
  );
});

test("property: the parsers are total over any output", () => {
  fc.assert(
    fc.property(fc.string(), (out) => {
      assert.ok(ANSWERS.includes(parsePortalAnswer(out)));
      assert.ok(ANSWERS.includes(parseGsettingsAnswer(out)));
    }),
  );
});

const outcome = fc.oneof(
  fc.constant(new Error("failed")),
  fc.constantFrom("v u 0\n", "v u 1\n", "(<uint32 0>,)\n", "(<uint32 1>,)\n"),
  fc.constantFrom("'reduce'\n", "'no-preference'\n"),
  fc.string(),
);

test("property: the portal decides when it answers, gsettings only when it cannot, and only reduce is ever forced", async () => {
  await fc.assert(
    fc.asyncProperty(outcome, outcome, outcome, async (busctl, gdbus, gsettings) => {
      const { appended } = await apply({ busctl, gdbus, gsettings });
      const text = (o) => (o instanceof Error ? "" : o);
      const expected =
        parsePortalAnswer(text(busctl)) ??
        parsePortalAnswer(text(gdbus)) ??
        parseGsettingsAnswer(text(gsettings));
      assert.deepEqual(
        appended,
        expected === "reduce" ? ["force-prefers-reduced-motion"] : [],
      );
    }),
  );
});

// ── the real exec ─────────────────────────────────────────────────

test("execText resolves a command's stdout", async () => {
  const out = await execText(process.execPath, ["-e", "process.stdout.write('v u 1')"], {
    timeoutMs: 5000,
  });
  assert.equal(out, "v u 1");
});

test("execText runs the command in the environment it is given", async () => {
  const out = await execText(
    process.execPath,
    ["-e", "process.stdout.write(String(process.env.ANI_GUI_PROBE) + ':' + String(process.env.DISPLAY))"],
    { timeoutMs: 5000, env: { ANI_GUI_PROBE: "seen" } },
  );
  assert.equal(out, "seen:undefined");
});

test("execText rejects a command that exits non-zero", async () => {
  await assert.rejects(
    execText(process.execPath, ["-e", "process.exit(1)"], { timeoutMs: 5000 }),
  );
});

test("execText rejects a missing command", async () => {
  await assert.rejects(execText("ani-gui-no-such-command", [], { timeoutMs: 5000 }));
});

// A real child and a real timer: what is under test is that execFile's
// kill fires at all. A kill that never comes hangs here, and the test's
// own timeout turns that hang into a failure that names it.
test("execText kills a command that outlives its timeout", { timeout: 5000 }, async () => {
  await assert.rejects(
    execText(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { timeoutMs: 100 }),
    (err) => err.killed === true && err.signal === "SIGKILL",
  );
});
