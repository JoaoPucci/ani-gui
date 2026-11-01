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

async function apply(outcomes, { platform = "linux", timeoutMs = 200 } = {}) {
  const { exec, calls } = fakeExec(outcomes);
  const appended = [];
  const result = await applyReducedMotion({
    platform,
    exec,
    appendSwitch: (name) => appended.push(name),
    timeoutMs,
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

test("without busctl the portal is read through gdbus", async () => {
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

test("a reader that never returns costs no more than the timeout", async () => {
  const started = Date.now();
  const { result, appended } = await apply(
    { busctl: HANG, gdbus: HANG, gsettings: HANG },
    { timeoutMs: 50 },
  );
  const elapsed = Date.now() - started;
  assert.equal(result, false);
  assert.deepEqual(appended, []);
  assert.ok(elapsed < 500, `took ${elapsed} ms against a 50 ms budget`);
});

test("each reader is handed the remaining budget as its own timeout", async () => {
  const { calls } = await apply({}, { timeoutMs: 300 });
  for (const call of calls) {
    assert.ok(call.options.timeoutMs > 0 && call.options.timeoutMs <= 300);
  }
});

test("a reader that answers after the deadline is not acted on", async () => {
  const appended = [];
  let release;
  const late = new Promise((resolve) => {
    release = resolve;
  });
  const result = await applyReducedMotion({
    platform: "linux",
    exec: () => late,
    appendSwitch: (name) => appended.push(name),
    timeoutMs: 20,
  });
  release("v u 1\n");
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(result, false);
  assert.deepEqual(appended, []);
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
    await readReducedMotion({ platform: "linux", exec, timeoutMs: 100 }),
    "no-preference",
  );
  assert.equal(
    await readReducedMotion({ platform: "linux", exec: fakeExec({}).exec, timeoutMs: 100 }),
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

test("execText rejects a command that exits non-zero", async () => {
  await assert.rejects(
    execText(process.execPath, ["-e", "process.exit(1)"], { timeoutMs: 5000 }),
  );
});

test("execText rejects a missing command", async () => {
  await assert.rejects(execText("ani-gui-no-such-command", [], { timeoutMs: 5000 }));
});

test("execText kills a command that outlives its timeout", async () => {
  const started = Date.now();
  await assert.rejects(
    execText(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { timeoutMs: 100 }),
  );
  assert.ok(Date.now() - started < 3000);
});
