// What the AppImage's AppRun does at launch, run rather than read.
//
// The repack (appimage-tree.cjs) rewrites electron-builder's AppRun to
// start the app's own binary once as the sandbox probe and pass
// `--no-sandbox` to the real launch only when the probe gets no answer
// (see sandbox-probe.cjs for why the binary is the only thing that can
// answer). Each test prepares an image from electron-builder's own
// AppRun, puts a stub where the app's binary goes, runs AppRun under
// bash, and reads back what the stub was started with.
//
// The stub answers the probe through the real answerSandboxProbe, so
// the two halves cannot drift apart unnoticed; the other probe outcomes
// stand in for what Chromium does when its sandbox does not start: an
// abort on SIGTRAP (no usable sandbox), an exit 1 (started as root
// without --no-sandbox), an exit 0 that never reached main.js, and a
// start that never returns.

const nodeTest = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { prepareTree, PROBE_BOUND_SECONDS } = require("./appimage-tree.cjs");

// AppRun is a Linux launcher, run here under bash and checked through
// /proc; on any other platform there is nothing to run it on.
const test = (name, fn) => nodeTest(name, { skip: process.platform !== "linux" }, fn);
const { SANDBOX_PROBE_SWITCH } = require("./sandbox-probe.cjs");

const fixture = (name) => fs.readFileSync(path.join(__dirname, "fixtures", name), "utf8");
const TEMPLATES = {
  "electron-builder 25": fixture("AppRun.electron-builder-25"),
  "electron-builder 26": fixture("AppRun.electron-builder-26"),
};

const FALLBACK_LINE = /^ani-gui: Chromium's sandbox cannot start on this system\. Starting without it \(--no-sandbox\)\.\n$/;

/** The stub that stands where `$APPDIR/ani-gui` goes. */
function stubFor(probe) {
  const answer = `exec "${process.execPath}" -e 'require(process.argv[1]).answerSandboxProbe(process.argv, process.stdout, process.exit)' "${path.join(__dirname, "sandbox-probe.cjs")}" "$@"`;
  const outcome = {
    answers: answer,
    aborts: 'kill -TRAP "$$"',
    "exits 1": "exit 1",
    "exits 0 unanswered": "exit 0",
    hangs: 'echo "$$" > "$here/probe-pid"; exec sleep 600',
    "answers, then exits 1": `${answer.replace(/^exec /, "")}; exit 1`,
    "answers, then hangs": `${answer.replace(/^exec /, "")}; exec sleep 600`,
    "records its core limit, then aborts": 'ulimit -c > "$here/probe-core-limit"; kill -TRAP "$$"',
  }[probe];
  return [
    "#!/bin/bash",
    'here="$(dirname "$0")"',
    'printf "%s\\n" "$*" >> "$here/calls"',
    'for a in "$@"; do',
    `  if [ "$a" = ${SANDBOX_PROBE_SWITCH} ]; then`,
    `    ${outcome}`,
    "  fi",
    "done",
    // One NUL-terminated entry per argument; printf alone would write
    // one empty entry for a launch with none.
    ': > "$here/launched"',
    'for a in "$@"; do printf "%s\\0" "$a" >> "$here/launched"; done',
    "",
  ].join("\n");
}

/** An image prepared from `template`, its binary a stub for `probe`. */
function image(template, probe) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "apprun-sandbox-"));
  const app = path.join(root, "squashfs-root");
  const tmp = path.join(root, "tmp");
  fs.mkdirSync(app);
  fs.mkdirSync(tmp);
  fs.writeFileSync(path.join(app, "AppRun"), template, { mode: 0o755 });
  fs.writeFileSync(path.join(app, "ani-gui.desktop"), fixture("ani-gui.desktop"));
  prepareTree(app);
  fs.writeFileSync(path.join(app, "ani-gui"), stubFor(probe), { mode: 0o755 });
  // Marks every process this run starts, so one left behind is found.
  const token = crypto.randomUUID();
  const env = (extraEnv) => ({
    PATH: process.env.PATH,
    HOME: root,
    TMPDIR: tmp,
    APPDIR: app,
    APPRUN_TEST_TOKEN: token,
    ...extraEnv,
  });
  return { root, app, tmp, token, env };
}

/** What a finished run left behind. */
function outcome(img, run, elapsedMs) {
  const readOr = (name, fallback) =>
    fs.existsSync(path.join(img.app, name)) ? fs.readFileSync(path.join(img.app, name), "utf8") : fallback;
  const launched = readOr("launched", null);
  return {
    run,
    elapsedMs,
    token: img.token,
    root: img.root,
    app: img.app,
    calls: readOr("calls", "").split("\n").filter(Boolean),
    launched: launched === null ? null : launched.split("\0").slice(0, -1),
    probePid: readOr("probe-pid", null),
    probeCoreLimit: readOr("probe-core-limit", null),
    cores: fs.readdirSync(img.root).filter((f) => f.startsWith("core")),
    tmpLeft: fs.readdirSync(img.tmp),
  };
}

/**
 * Prepares an image from `template`, runs its AppRun with `args`. With
 * `shell`, AppRun is started from that bash snippet instead, which ends
 * by exec'ing `"$@"` (AppRun and its arguments).
 */
function launch(template, probe, args, extraEnv = {}, shell = null) {
  const img = image(template, probe);
  const appRun = [path.join(img.app, "AppRun"), ...args];
  const argv = shell === null ? ["bash", appRun] : ["bash", ["-c", shell, "bash", "bash", ...appRun]];
  const started = Date.now();
  const run = spawnSync(argv[0], argv[1], { cwd: img.root, encoding: "utf8", env: img.env(extraEnv) });
  return outcome(img, run, Date.now() - started);
}

/** Starts AppRun in a process group of its own, signals it after `afterMs`. */
async function launchAndSignal(template, probe, args, { afterMs, signal, group }) {
  const { spawn } = require("node:child_process");
  const img = image(template, probe);
  const started = Date.now();
  const child = spawn("bash", [path.join(img.app, "AppRun"), ...args], {
    cwd: img.root,
    env: img.env({}),
    detached: true,
    stdio: ["ignore", "ignore", "pipe"],
  });
  let stderr = "";
  child.stderr.on("data", (d) => {
    stderr += d;
  });
  const timer = setTimeout(() => (group ? process.kill(-child.pid, signal) : child.kill(signal)), afterMs);
  const [status, killedBy] = await new Promise((resolve) => child.on("close", (code, sig) => resolve([code, sig])));
  clearTimeout(timer);
  return { ...outcome(img, { status, signal: killedBy, stderr }, Date.now() - started) };
}

/** Processes still running that a launch started. */
function leftovers(token) {
  const found = [];
  for (const pid of fs.readdirSync("/proc").filter((d) => /^\d+$/.test(d))) {
    try {
      if (fs.readFileSync(`/proc/${pid}/environ`, "utf8").includes(`APPRUN_TEST_TOKEN=${token}`)) {
        found.push(`${pid}: ${fs.readFileSync(`/proc/${pid}/cmdline`, "utf8").replaceAll("\0", " ")}`);
      }
    } catch {
      // gone, or not ours
    }
  }
  return found;
}

/** Waits briefly for processes being killed to go, then reports any left. */
function assertNothingLeft(token) {
  const until = Date.now() + 2000;
  let left = leftovers(token);
  while (left.length > 0 && Date.now() < until) {
    spawnSync("sleep", ["0.05"]);
    left = leftovers(token);
  }
  assert.deepEqual(left, [], "the launch left processes behind");
}

function cleanup(r) {
  fs.rmSync(r.root, { recursive: true, force: true });
}

const USER_ARGS = ["ani-gui://anime/1", "--a flag with spaces", "last"];

for (const [name, template] of Object.entries(TEMPLATES)) {
  test(`${name}: a probe that answers starts the app sandboxed, with the user's arguments as given`, () => {
    const r = launch(template, "answers", USER_ARGS);
    try {
      assert.equal(r.run.status, 0, r.run.stderr);
      assert.deepEqual(r.launched, USER_ARGS);
      assert.deepEqual(r.calls, [SANDBOX_PROBE_SWITCH, USER_ARGS.join(" ")], "one probe, then one launch");
      assert.equal(r.run.stderr, "");
      assert.ok(r.elapsedMs < 3000, `an answered probe holds the launch up for ${r.elapsedMs} ms`);
      assert.deepEqual(r.tmpLeft, [], "the probe's answer file is removed");
      assertNothingLeft(r.token);
    } finally {
      cleanup(r);
    }
  });

  test(`${name}: a probe that answers starts an argument-less launch sandboxed`, () => {
    const r = launch(template, "answers", []);
    try {
      assert.deepEqual(r.launched, []);
    } finally {
      cleanup(r);
    }
  });

  test(`${name}: a sandbox that aborts the probe starts the app with --no-sandbox, quietly`, () => {
    const r = launch(template, "aborts", USER_ARGS);
    try {
      assert.equal(r.run.status, 0, r.run.stderr);
      assert.deepEqual(r.launched, ["--no-sandbox", ...USER_ARGS]);
      assert.match(r.run.stderr, FALLBACK_LINE, "one line, and nothing of the abort");
      assert.deepEqual(r.cores, [], "no core dump");
      assertNothingLeft(r.token);
    } finally {
      cleanup(r);
    }
  });

  test(`${name}: a user's own --no-sandbox is kept where it was, and no probe runs`, () => {
    const args = ["ani-gui://anime/1", "--no-sandbox", "last"];
    const r = launch(template, "aborts", args);
    try {
      assert.deepEqual(r.calls, [args.join(" ")], "no probe");
      assert.deepEqual(r.launched, args);
      assert.equal(r.run.stderr, "");
    } finally {
      cleanup(r);
    }
  });
}

const EB25 = TEMPLATES["electron-builder 25"];

for (const probe of ["exits 1", "exits 0 unanswered"]) {
  test(`a probe that ${probe} starts the app with --no-sandbox`, () => {
    const r = launch(EB25, probe, USER_ARGS);
    try {
      assert.deepEqual(r.launched, ["--no-sandbox", ...USER_ARGS]);
      assert.match(r.run.stderr, FALLBACK_LINE);
    } finally {
      cleanup(r);
    }
  });
}

test("a probe that never returns is killed at the bound and the app starts with --no-sandbox", () => {
  const r = launch(EB25, "hangs", USER_ARGS);
  try {
    assert.deepEqual(r.launched, ["--no-sandbox", ...USER_ARGS]);
    assert.match(r.run.stderr, FALLBACK_LINE);
    assert.ok(r.elapsedMs >= PROBE_BOUND_SECONDS * 1000, `waited ${r.elapsedMs} ms`);
    assert.ok(r.elapsedMs < (PROBE_BOUND_SECONDS + 5) * 1000, `waited ${r.elapsedMs} ms`);
    assert.ok(r.probePid, "the probe started");
    assertNothingLeft(r.token);
  } finally {
    cleanup(r);
  }
});

// electron-builder's install-only run (APPIMAGE_EXIT_AFTER_INSTALL)
// never launches the app, so it has nothing to probe for either.
test("an install-only run starts nothing, the probe included", () => {
  const r = launch(EB25, "answers", [], { APPIMAGE_EXIT_AFTER_INSTALL: "1" });
  try {
    assert.deepEqual(r.calls, []);
    assert.equal(r.launched, null);
  } finally {
    cleanup(r);
  }
});

for (const probe of ["answers, then exits 1", "answers, then hangs"]) {
  test(`a probe that ${probe} is not trusted, and the app starts with --no-sandbox`, () => {
    const r = launch(EB25, probe, USER_ARGS);
    try {
      assert.deepEqual(r.launched, ["--no-sandbox", ...USER_ARGS]);
      assert.match(r.run.stderr, FALLBACK_LINE);
      assertNothingLeft(r.token);
    } finally {
      cleanup(r);
    }
  });
}

// The fallback is for the hosts where the sandbox cannot start, and it
// must start the app there whatever happens to its one line of output:
// a launcher with no stderr (closed, or a full disk) still launches.
for (const [how, shell] of [
  ["closed", 'exec 2>&-; exec "$@"'],
  ["unwritable", 'exec 2>/dev/full; exec "$@"'],
]) {
  test(`a fallback whose stderr is ${how} still starts the app with --no-sandbox`, () => {
    for (const template of Object.values(TEMPLATES)) {
      const r = launch(template, "aborts", USER_ARGS, {}, shell);
      try {
        assert.equal(r.run.status, 0);
        assert.deepEqual(r.launched, ["--no-sandbox", ...USER_ARGS]);
      } finally {
        cleanup(r);
      }
    }
  });
}

// A probe that aborts would dump core on every launch on the hosts it
// fails on. Whether a core file would land anywhere depends on the
// host's core_pattern (Ubuntu pipes cores to apport), so what is
// checked is the limit the probe runs under, from a launcher whose own
// limit allows cores. Where the hard limit is already 0 the check has
// nothing to show and says so.
nodeTest("the probe runs with core dumps off", { skip: process.platform !== "linux" }, (t) => {
  const hard = spawnSync("bash", ["-c", "ulimit -H -c"], { encoding: "utf8" }).stdout.trim();
  if (hard === "0") {
    t.skip("the hard core limit here is 0: nothing to turn off");
    return;
  }
  const r = launch(EB25, "records its core limit, then aborts", [], {}, 'ulimit -S -c "$(ulimit -H -c)"; exec "$@"');
  try {
    assert.equal(r.probeCoreLimit, "0\n");
    assert.deepEqual(r.launched, ["--no-sandbox"]);
  } finally {
    cleanup(r);
  }
});

// Interrupting the launcher while it waits on the probe stops the
// launch: Ctrl-C (SIGINT to the whole process group), a SIGTERM to
// AppRun alone, or a SIGHUP (the terminal closing) ends it promptly,
// starts nothing, and leaves no probe, watchdog or answer file behind.
// AppRun then dies by that same signal, as a shell expects of a
// command it interrupted, so a calling loop stops too.
for (const [name, template] of Object.entries(TEMPLATES)) {
  for (const [signal, group] of [
    ["SIGINT", true],
    ["SIGTERM", false],
    ["SIGHUP", false],
  ]) {
    test(`${name}: ${signal} while the probe runs stops the launch and leaves nothing behind`, async () => {
      const r = await launchAndSignal(template, "hangs", USER_ARGS, { afterMs: 700, signal, group });
      try {
        assert.equal(r.launched, null, "nothing launched");
        assert.equal(r.run.signal, signal, `AppRun ended with status ${r.run.status}, not by ${signal}`);
        assert.ok(r.elapsedMs < 3000, `stopped after ${r.elapsedMs} ms`);
        assert.doesNotMatch(r.run.stderr, /command not found|Terminated|Killed|Hangup/);
        assert.deepEqual(r.tmpLeft, [], "the probe's answer file is removed");
        assertNothingLeft(r.token);
      } finally {
        cleanup(r);
      }
    });
  }
}
