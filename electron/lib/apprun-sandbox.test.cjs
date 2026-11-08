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

const test = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { prepareTree, PROBE_BOUND_SECONDS } = require("./appimage-tree.cjs");
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

/** Prepares an image from `template`, runs its AppRun with `args`. */
function launch(template, probe, args, extraEnv = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "apprun-sandbox-"));
  const app = path.join(root, "squashfs-root");
  fs.mkdirSync(app);
  fs.writeFileSync(path.join(app, "AppRun"), template, { mode: 0o755 });
  fs.writeFileSync(path.join(app, "ani-gui.desktop"), fixture("ani-gui.desktop"));
  prepareTree(app);
  fs.writeFileSync(path.join(app, "ani-gui"), stubFor(probe), { mode: 0o755 });
  // Marks every process this run starts, so one left behind is found.
  const token = crypto.randomUUID();
  const started = Date.now();
  const run = spawnSync("bash", [path.join(app, "AppRun"), ...args], {
    cwd: root,
    encoding: "utf8",
    env: { PATH: process.env.PATH, HOME: root, APPDIR: app, APPRUN_TEST_TOKEN: token, ...extraEnv },
  });
  const elapsedMs = Date.now() - started;
  const readOr = (name, fallback) =>
    fs.existsSync(path.join(app, name)) ? fs.readFileSync(path.join(app, name), "utf8") : fallback;
  const launched = readOr("launched", null);
  const result = {
    run,
    elapsedMs,
    token,
    root,
    app,
    calls: readOr("calls", "").split("\n").filter(Boolean),
    launched: launched === null ? null : launched.split("\0").slice(0, -1),
    probePid: readOr("probe-pid", null),
    cores: fs.readdirSync(root).filter((f) => f.startsWith("core")),
  };
  return result;
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
