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
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { prepareTree, PROBE_BOUND_SECONDS } = require("./appimage-tree.cjs");

// AppRun is a Linux launcher, run here under bash in a session of its
// own; on any other platform there is nothing to run it on.
const test = (name, fn) => nodeTest(name, { skip: process.platform !== "linux" }, fn);
const { SANDBOX_PROBE_SWITCH } = require("./sandbox-probe.cjs");
const { launcherScript } = require("./linux-launcher.cjs");

const fixture = (name) => fs.readFileSync(path.join(__dirname, "fixtures", name), "utf8");
const TEMPLATES = {
  "electron-builder 25": fixture("AppRun.electron-builder-25"),
  "electron-builder 26": fixture("AppRun.electron-builder-26"),
};

const FALLBACK_LINE = /^ani-gui: Chromium's sandbox cannot start on this system\. Starting without it \(--no-sandbox\)\.\n$/;

/** The stub that stands where `$APPDIR/ani-gui` goes. */
function stubFor(probe) {
  const answer = `exec "${process.execPath}" -e 'require(process.argv[1]).answerSandboxProbe(process.argv, process.stdout, process.exit)' "${path.join(__dirname, "sandbox-probe.cjs")}" "$@"`;
  // Chromium's abort, by a process that is not dumpable: SIGTRAP dumps
  // core, and a host that pipes cores to a crash reporter (Ubuntu's
  // apport) is handed one whatever the core limit. Not dumpable, the
  // probe leaves none, and AppRun sees the same death by SIGTRAP. That
  // the probe runs with core dumps off is the core-limit case's to show.
  const abort = `exec python3 -I -c 'import ctypes, os, signal; ctypes.CDLL(None).prctl(4, 0, 0, 0, 0); signal.signal(signal.SIGTRAP, signal.SIG_DFL); os.kill(os.getpid(), signal.SIGTRAP)'`;
  const outcome = {
    answers: answer,
    aborts: abort,
    "exits 1": "exit 1",
    "exits 0 unanswered": "exit 0",
    hangs: 'echo "$$" > "$here/probe-pid"; exec sleep 600',
    "answers, then exits 1": `${answer.replace(/^exec /, "")}; exit 1`,
    "answers, then hangs": `${answer.replace(/^exec /, "")}; exec sleep 600`,
    "records its core limit, then aborts": `ulimit -c > "$here/probe-core-limit"; ${abort}`,
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

/** An image prepared from `template`, its binary a stub for `probe`.
 *  `throughLauncher` lays it out as the packages do: the stub is
 *  `ani-gui.bin`, behind the real launcher at `ani-gui`
 *  (lib/linux-launcher.cjs). Otherwise the stub stands at `ani-gui`. */
function image(template, probe, throughLauncher = false) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "apprun-sandbox-"));
  const app = path.join(root, "squashfs-root");
  const tmp = path.join(root, "tmp");
  fs.mkdirSync(app);
  fs.mkdirSync(tmp);
  fs.writeFileSync(path.join(app, "AppRun"), template, { mode: 0o755 });
  fs.writeFileSync(path.join(app, "ani-gui.desktop"), fixture("ani-gui.desktop"));
  prepareTree(app);
  if (throughLauncher) {
    fs.writeFileSync(path.join(app, "ani-gui.bin"), stubFor(probe), { mode: 0o755 });
    fs.writeFileSync(path.join(app, "ani-gui"), launcherScript("ani-gui"), { mode: 0o755 });
  } else {
    fs.writeFileSync(path.join(app, "ani-gui"), stubFor(probe), { mode: 0o755 });
  }
  const env = (extraEnv) => ({
    PATH: process.env.PATH,
    HOME: root,
    TMPDIR: tmp,
    APPDIR: app,
    ...extraEnv,
  });
  return { root, app, tmp, env };
}

/** What a finished run left behind. `group` is the process group the
 *  run was started in, its own. */
function outcome(img, run, elapsedMs, group) {
  const readOr = (name, fallback) =>
    fs.existsSync(path.join(img.app, name)) ? fs.readFileSync(path.join(img.app, name), "utf8") : fallback;
  const launched = readOr("launched", null);
  return {
    run,
    elapsedMs,
    group,
    root: img.root,
    app: img.app,
    calls: readOr("calls", "").split("\n").filter(Boolean),
    launched: launched === null ? null : launched.split("\0").slice(0, -1),
    probePid: readOr("probe-pid", null),
    probeCoreLimit: readOr("probe-core-limit", null),
    tmpLeft: fs.readdirSync(img.tmp),
    unreaped: readOr("unreaped", null),
  };
}

// A host whose PID 1 does not reap orphans (a container's bare init,
// for one) keeps every child the launch ends without waiting for as a
// zombie, for good. Where an ordinary init reaps them, nothing shows,
// so each run starts under a subreaper that stands in for such a PID 1:
// it adopts whatever the run orphans, and once the run has ended it
// counts the orphans that end in its hands into `unreaped` and does not
// keep them. With `forward`, a signal it receives is passed on to the
// run, which is otherwise what the signal was meant for; a signal sent
// to the whole group reaches the run directly. The subreaper holds
// SIGINT, SIGTERM and SIGHUP from before it forks the run until its own
// handlers are in place, so none that arrives early ends the subreaper
// instead; the run unblocks them before it starts. The run starts with
// the signal dispositions it would have without the subreaper. Once the
// run has ended the subreaper marks it in `<report>.ended`, and goes on
// reaping for up to `linger` seconds while orphans remain; it then ends
// as the run did, by the same signal or with the same status. It ends
// by a signal only once it is no longer dumpable, so a signal that
// dumps core leaves no core of its own.
const SUBREAPER = `
import ctypes, os, signal, sys, time
if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
    sys.exit("subreaper: prctl failed: " + os.strerror(ctypes.get_errno()))
report, forward, linger, argv = sys.argv[1], sys.argv[2] == "1", float(sys.argv[3]), sys.argv[4:]
held = {signal.SIGINT, signal.SIGTERM, signal.SIGHUP}
signal.pthread_sigmask(signal.SIG_BLOCK, held)
child = os.fork()
if child == 0:
    # Python starts with these two ignored, and an ignored signal stays
    # ignored across exec, as a blocked one stays blocked: the run gets
    # the dispositions and mask a launch has.
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    signal.signal(signal.SIGXFSZ, signal.SIG_DFL)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, held)
    os.execvp(argv[0], argv)
def on_signal(sig, _frame):
    if forward:
        os.kill(child, sig)
for sig in held:
    signal.signal(sig, on_signal)
signal.pthread_sigmask(signal.SIG_UNBLOCK, held)
_, status = os.waitpid(child, 0)
open(report + ".ended", "w").close()
unreaped, until = 0, time.monotonic() + linger
while True:
    try:
        pid, _ = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    if pid:
        unreaped += 1
    elif time.monotonic() > until:
        break
    else:
        time.sleep(0.02)
with open(report, "w") as f:
    f.write(str(unreaped))
if os.WIFSIGNALED(status):
    # Not dumpable, so a signal that dumps core ends it without a core
    # for the host's crash reporter (see the core-dump cases below).
    ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)  # PR_SET_DUMPABLE
    signal.signal(os.WTERMSIG(status), signal.SIG_DFL)
    os.kill(os.getpid(), os.WTERMSIG(status))
sys.exit(os.waitstatus_to_exitcode(status))
`;

/** python3's arguments to run `argv` under the subreaper above. */
function underSubreaper(img, forward, argv, linger = 2) {
  return ["-I", "-c", SUBREAPER, path.join(img.app, "unreaped"), forward ? "1" : "0", String(linger), ...argv];
}

// A run that dies by a signal that dumps core (Chromium ends on
// SIGTRAP when it fails; SIGABRT, SIGSEGV and the rest are crashes) is
// passed on by the subreaper ending itself by that same signal. Were
// the subreaper dumpable then, the kernel would dump its core in turn,
// and a host that pipes cores to a crash reporter (Ubuntu's apport)
// would report python3 itself as crashed, on the desktop of whoever ran
// the suite. A core limit of 0 does not stop that: the pipe is started
// whatever the limit, and apport writes its report anyway. A process
// that is not dumpable is never dumped, so the subreaper has to stop
// being dumpable before it ends itself.
//
// No dumpable process dies here, before the fix or after it. Each run
// makes itself non-dumpable before it signals itself, so it leaves no
// core. The subreaper runs with os.kill replaced for its own pid: in
// place of ending itself, it writes down the signal and whether it is
// dumpable at that moment, and exits.
const CORE_SIGNALS = ["SIGQUIT", "SIGILL", "SIGTRAP", "SIGABRT", "SIGBUS", "SIGFPE", "SIGSEGV", "SIGXCPU", "SIGXFSZ", "SIGSYS"];
for (const sig of CORE_SIGNALS) {
  test(`a run that dies by ${sig} is passed on by a subreaper that cannot dump core`, () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "apprun-sandbox-"));
    try {
      const ending = path.join(root, "ending");
      const shim = [
        "import ctypes, os, sys",
        "real_kill = os.kill",
        "def kill(pid, sig):",
        "    if pid != os.getpid():",
        "        return real_kill(pid, sig)",
        "    dumpable = ctypes.CDLL(None).prctl(3, 0, 0, 0, 0)  # PR_GET_DUMPABLE",
        `    with open(${JSON.stringify(ending)}, "w") as f:`,
        '        f.write(f"{int(sig)} {dumpable}")',
        "    os._exit(0)",
        "os.kill = kill",
        `exec(compile(${JSON.stringify(SUBREAPER)}, "<subreaper>", "exec"))`,
      ].join("\n");
      const run = [
        "import ctypes, os, signal",
        "ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)  # PR_SET_DUMPABLE: this run leaves no core",
        `signal.signal(signal.${sig}, signal.SIG_DFL)`,
        `os.kill(os.getpid(), signal.${sig})`,
      ].join("\n");
      const r = spawnSync("python3", ["-I", "-c", shim, path.join(root, "unreaped"), "0", "0", "python3", "-I", "-c", run], {
        encoding: "utf8",
        timeout: 10000,
      });
      assert.equal(r.status, 0, `the subreaper ended with status ${r.status}, signal ${r.signal}: ${r.stderr}`);
      assert.ok(fs.existsSync(ending), `the subreaper did not signal itself: ${r.stderr}`);
      const [signalled, dumpable] = fs.readFileSync(ending, "utf8").split(" ").map(Number);
      assert.equal(signalled, os.constants.signals[sig], "it ends by the run's own signal");
      assert.equal(dumpable, 0, "it is still dumpable as it ends itself, so its own core is dumped");
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
}

/** SIGKILL to the group a test started, which may already be gone. */
function killOwnGroup(group) {
  try {
    process.kill(-group, "SIGKILL");
  } catch (err) {
    if (err.code !== "ESRCH") throw err;
  }
}

/** Whether `pid` is a live process: present, and not a zombie. */
function liveProcess(pid) {
  try {
    return !/^\d+ \(.*\) Z /s.test(fs.readFileSync(`/proc/${pid}/stat`, "utf8"));
  } catch (err) {
    if (err.code === "ENOENT") return false;
    throw err;
  }
}

const sleepMs = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Waits up to `boundMs` for `done()`; returns whether it came. */
async function until(done, boundMs) {
  const end = Date.now() + boundMs;
  while (!done()) {
    if (Date.now() > end) return false;
    await sleepMs(20);
  }
  return true;
}

/**
 * Prepares an image from `template`, runs its AppRun with `args`. With
 * `shell`, AppRun is started from that bash snippet instead, which ends
 * by exec'ing `"$@"` (AppRun and its arguments).
 */
function launch(template, probe, args, extraEnv = {}, shell = null, throughLauncher = false) {
  const img = image(template, probe, throughLauncher);
  const appRun = [path.join(img.app, "AppRun"), ...args];
  const inner = shell === null ? ["bash", ...appRun] : ["bash", "-c", shell, "bash", "bash", ...appRun];
  // The run gets a session, and so a process group, of its own: setsid
  // starts the recorder, which writes its own pid (the group's leader)
  // and then becomes the run. Whatever the run leaves behind is in that
  // group (assertNothingLeft, below).
  const groupFile = path.join(img.root, "group");
  const recordGroup = 'echo "$$" > "$1"; shift; exec "$@"';
  const started = Date.now();
  const run = spawnSync("python3", underSubreaper(img, false, ["setsid", "--wait", "bash", "-c", recordGroup, "bash", groupFile, ...inner]), {
    cwd: img.root,
    encoding: "utf8",
    env: img.env(extraEnv),
  });
  const elapsedMs = Date.now() - started;
  return outcome(img, run, elapsedMs, Number(fs.readFileSync(groupFile, "utf8").trim()));
}

/**
 * Starts AppRun in a process group of its own and signals it once the
 * probe is running (the probe records its pid) and `settleMs` more have
 * passed, so the signal lands on AppRun waiting on the probe however
 * long the interpreter and shell took to start. `elapsedMs` runs from
 * the signal. A probe that never starts is a failed test, not a signal
 * sent anyway.
 */
async function launchAndSignal(template, probe, args, { settleMs, signal, group, slowToGo = false, then = null }) {
  const { spawn } = require("node:child_process");
  const img = image(template, probe);
  const child = spawn("python3", underSubreaper(img, !group, ["bash", path.join(img.app, "AppRun"), ...args]), {
    cwd: img.root,
    env: img.env(slowToGo ? { PATH: `${slowSleepDir(img, slowReady(img))}:${process.env.PATH}` } : {}),
    detached: true,
    stdio: ["ignore", "ignore", "pipe"],
  });
  let stderr = "";
  child.stderr.on("data", (d) => {
    stderr += d;
  });
  let gone = false;
  const closed = new Promise((resolve) =>
    child.on("close", (code, sig) => {
      gone = true;
      resolve([code, sig]);
    }),
  );
  let signalledAt = null;
  await until(() => gone || fs.existsSync(path.join(img.app, "probe-pid")), 15000);
  // The probe's sleep and the watchdog's sleeper, each with its
  // handlers in place. A probe that never started is reported below.
  const probeStarted = fs.existsSync(path.join(img.app, "probe-pid"));
  if (slowToGo && probeStarted && !(await until(() => gone || fs.readdirSync(slowReady(img)).length >= 2, 15000))) {
    const ready = fs.readdirSync(slowReady(img)).length;
    killOwnGroup(child.pid);
    await closed;
    fs.rmSync(img.root, { recursive: true, force: true });
    assert.fail(`only ${ready} of the two slow sleeps became ready, so nothing was signalled: ${stderr}`);
  }
  if (!gone && fs.existsSync(path.join(img.app, "probe-pid"))) {
    await sleepMs(settleMs);
    signalledAt = Date.now();
    if (group) process.kill(-child.pid, signal);
    else child.kill(signal);
    if (then !== null) {
      // An afterMs of 0 sends the second signal in the same tick as the
      // first, microseconds behind it, as a terminal's hangup and a
      // service manager's SIGTERM can land together.
      if (then.afterMs > 0) await sleepMs(then.afterMs);
      if (!gone) process.kill(-child.pid, then.signal);
    }
  } else if (!gone) {
    killOwnGroup(child.pid);
  }
  const [status, killedBy] = await closed;
  // Detached: the child leads a session, and so a group, of its own,
  // and AppRun, its child, starts in that group.
  const r = outcome(img, { status, signal: killedBy, stderr }, Date.now() - (signalledAt ?? 0), child.pid);
  if (signalledAt === null) {
    cleanup(r);
    assert.fail(`the probe never started, so nothing was signalled (status ${status}, signal ${killedBy}): ${stderr}`);
  }
  return r;
}

/** Where the slow `sleep`s below say they are ready. */
function slowReady(img) {
  const ready = path.join(img.root, "slow-ready");
  fs.mkdirSync(ready, { recursive: true });
  return ready;
}

/** A directory holding a `sleep` that is slow to go: stopped by
 *  SIGTERM or SIGHUP, it takes half a second more to exit, as any
 *  child may on a loaded host. Put ahead of PATH, it stands in for the
 *  watchdog's sleeper and for the probe's own `exec sleep`. Each
 *  one, once its handlers are in place, writes a file named after its
 *  pid into `ready`, so a test can wait for both before it signals. */
function slowSleepDir(img, ready) {
  // The interpreter itself, not what PATH names: a python3 that is a
  // shim (pyenv's is a shell script) starts children of its own before
  // Python runs, and a signal then would leave them orphaned.
  const python = spawnSync("python3", ["-I", "-c", "import sys; print(sys.executable)"], { encoding: "utf8" }).stdout.trim();
  const dir = path.join(img.root, "slow-bin");
  fs.mkdirSync(dir);
  // One process, with no children of its own: the probe is stopped
  // with SIGKILL, which would leave a stand-in's own children orphaned.
  // `sleep` itself only execs Python on the script, so no shebang
  // carries the interpreter's path, however long it is or whatever
  // characters it holds.
  fs.writeFileSync(
    path.join(dir, "sleep"),
    ["#!/bin/sh", `exec ${shellQuote(python)} -I ${shellQuote(path.join(dir, "slow-sleep.py"))} "$@"`, ""].join("\n"),
    { mode: 0o755 },
  );
  fs.writeFileSync(
    path.join(dir, "slow-sleep.py"),
    [
      "import os, signal, sys, time",
      // A short sleep (AppRun's reap runs one between retries) is not
      // one of the two being stood in for: it ends at once, and is not
      // counted as ready.
      "if float(sys.argv[1]) <= 0.05:",
      "    sys.exit(0)",
      "def go(*_):",
      "    signal.signal(signal.SIGTERM, signal.SIG_IGN)",
      "    signal.signal(signal.SIGHUP, signal.SIG_IGN)",
      "    time.sleep(0.5)",
      "    sys.exit(143)",
      "signal.signal(signal.SIGTERM, go)",
      "signal.signal(signal.SIGHUP, go)",
      `open(${JSON.stringify(ready)} + "/" + str(os.getpid()), "w").close()`,
      "time.sleep(float(sys.argv[1]))",
      "",
    ].join("\n"),
  );
  return dir;
}

/** `text` as one single-quoted sh word. */
function shellQuote(text) {
  return `'${text.replaceAll("'", "'\\''")}'`;
}

/** Whether any process is still in `group`, the run's own group:
 *  signal 0 to it, which tests delivery and delivers nothing. A test
 *  that tries to reach a process outside its own group would be
 *  choosing it from the process table (tests/arch/own_processes.sh). */
function groupAlive(group) {
  try {
    process.kill(-group, 0);
    return true;
  } catch (err) {
    if (err.code === "ESRCH") return false;
    throw err;
  }
}

/** Waits briefly for processes being killed to go, then fails if any
 *  is left in the run's group. AppRun, the probe, its watchdog and the
 *  app's stub all start in that group, and nothing in them leaves it. */
function assertNothingLeft(r) {
  const until = Date.now() + 2000;
  while (groupAlive(r.group) && Date.now() < until) spawnSync("sleep", ["0.05"]);
  assert.equal(groupAlive(r.group), false, `the launch left processes behind in its group ${r.group}`);
  assert.equal(r.unreaped, "0", "the launch ended children without waiting for them, which a host that does not reap orphans keeps as zombies");
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
      assertNothingLeft(r);
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
      assertNothingLeft(r);
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

// AppRun's $BIN is the packages' launcher, which puts its own switch
// first and, given no terminal on any of the three streams (as the
// probe never is), execs the binary: the probe AppRun waits on, and its
// watchdog kills, is then the binary itself.
for (const [name, template] of Object.entries(TEMPLATES)) {
  test(`${name}: through the packages' launcher, an answered probe starts the app sandboxed`, () => {
    const r = launch(template, "answers", USER_ARGS, {}, null, true);
    try {
      assert.equal(r.run.status, 0, r.run.stderr);
      assert.deepEqual(r.launched, ["--enable-wayland-ime", ...USER_ARGS]);
      assert.deepEqual(r.calls, [`--enable-wayland-ime ${SANDBOX_PROBE_SWITCH}`, ["--enable-wayland-ime", ...USER_ARGS].join(" ")]);
      assert.equal(r.run.stderr, "");
      assert.deepEqual(r.tmpLeft, []);
      assertNothingLeft(r);
    } finally {
      cleanup(r);
    }
  });

  test(`${name}: through the packages' launcher, an aborted probe starts the app with --no-sandbox`, () => {
    const r = launch(template, "aborts", USER_ARGS, {}, null, true);
    try {
      assert.equal(r.run.status, 0, r.run.stderr);
      assert.deepEqual(r.launched, ["--enable-wayland-ime", "--no-sandbox", ...USER_ARGS]);
      assert.match(r.run.stderr, FALLBACK_LINE);
      assertNothingLeft(r);
    } finally {
      cleanup(r);
    }
  });
}

test("through the packages' launcher, a probe that never returns is killed at the bound", () => {
  const r = launch(EB25, "hangs", USER_ARGS, {}, null, true);
  try {
    assert.deepEqual(r.launched, ["--enable-wayland-ime", "--no-sandbox", ...USER_ARGS]);
    assert.match(r.run.stderr, FALLBACK_LINE);
    assert.ok(r.elapsedMs >= PROBE_BOUND_SECONDS * 1000, `waited ${r.elapsedMs} ms`);
    assert.ok(r.elapsedMs < (PROBE_BOUND_SECONDS + 5) * 1000, `waited ${r.elapsedMs} ms`);
    assert.ok(r.probePid, "the probe started");
    assertNothingLeft(r);
  } finally {
    cleanup(r);
  }
});

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
    assertNothingLeft(r);
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
      assertNothingLeft(r);
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
// AppRun alone or to its whole group (a service manager stopping it),
// or a SIGHUP to AppRun or to its whole group (the terminal closing
// hangs up its foreground group) ends it promptly, starts nothing, and
// leaves no probe, watchdog or answer file behind, reaped or not.
// AppRun then dies by that same signal, as a shell expects of a
// command it interrupted, so a calling loop stops too.
for (const [name, template] of Object.entries(TEMPLATES)) {
  for (const [signal, group] of [
    ["SIGINT", true],
    ["SIGTERM", false],
    ["SIGHUP", false],
    ["SIGTERM", true],
    ["SIGHUP", true],
  ]) {
    const to = group && signal !== "SIGINT" ? " to the whole group" : "";
    test(`${name}: ${signal}${to} while the probe runs stops the launch and leaves nothing behind`, async () => {
      const r = await launchAndSignal(template, "hangs", USER_ARGS, { settleMs: 200, signal, group });
      try {
        assert.equal(r.launched, null, "nothing launched");
        assert.equal(r.run.signal, signal, `AppRun ended with status ${r.run.status}, not by ${signal}`);
        assert.ok(r.elapsedMs < 3000, `stopped after ${r.elapsedMs} ms`);
        assert.doesNotMatch(r.run.stderr, /command not found|Terminated|Killed|Hangup/);
        assert.deepEqual(r.tmpLeft, [], "the probe's answer file is removed");
        assertNothingLeft(r);
      } finally {
        cleanup(r);
      }
    });
  }
}

/**
 * Runs `script` under bash, under the subreaper, for the stop-handler
 * cases: the script records a background job of its own, the bystander,
 * in `$APPDIR/bystander` and then ends itself by a signal, which leaves
 * the bystander to whatever adopts it. Once the script has ended, the
 * bystander is checked, killed if it is alive, and reaped by the
 * subreaper, so no case leaves it as a zombie on a host whose PID 1
 * does not reap. Returns how the script ended and whether the
 * bystander had survived it.
 */
async function runWithBystander(img, script) {
  const { spawn } = require("node:child_process");
  // Detached: the subreaper leads a group of its own, which the script
  // and its bystander start in, so a case that hangs is ended whole.
  const child = spawn("python3", underSubreaper(img, false, ["bash", "-c", script], 15), {
    env: img.env({}),
    detached: true,
    stdio: ["ignore", "ignore", "pipe"],
  });
  let stderr = "";
  child.stderr.on("data", (d) => {
    stderr += d;
  });
  let gone = false;
  const closed = new Promise((resolve) =>
    child.on("close", (code, sig) => {
      gone = true;
      resolve([code, sig]);
    }),
  );
  const ended = () => fs.existsSync(path.join(img.app, "unreaped.ended"));
  await until(() => gone || ended(), 15000);
  if (!ended()) {
    killOwnGroup(child.pid);
    const [status, signal] = await closed;
    assert.fail(`the script did not end, or the subreaper failed (status ${status}, signal ${signal}): ${stderr}`);
  }
  let alive = false;
  const bystanderFile = path.join(img.app, "bystander");
  if (fs.existsSync(bystanderFile)) {
    const bystander = Number(fs.readFileSync(bystanderFile, "utf8"));
    alive = liveProcess(bystander);
    if (alive) process.kill(bystander, "SIGKILL");
  }
  const [status, signal] = await closed;
  return { run: { status, signal, stderr }, alive };
}

// The stop handler signals the probe job, and once that job has ended
// there is nothing of the probe's left to signal: a signal arriving
// between the probe's end and the moment the traps are cleared must
// not reach any other process, such as whatever now holds the job's
// old pid or the shell's latest background job. Exercised on the
// probe's own functions, cut from the rendered AppRun at the lines
// the repack writes, with an answering stub, then a background
// process standing in for the next one `$!` would name.
// A second stop can come while the first is still being handled: a
// service manager that follows its SIGTERM with another, a terminal's
// hangup followed by the user's own kill, or, under load, AppRun's
// SIGTERM to the probe's subshell landing after the hangup to the
// whole group. The children are slow to go here, so the second signal
// lands while the launch is still waiting for them. It must not cut
// those waits short: a child the launch stops waiting for is left
// unreaped. The launch still ends by the first signal.
for (const [name, template] of Object.entries(TEMPLATES)) {
  for (const [signal, second] of [
    ["SIGHUP", "SIGTERM"],
    ["SIGTERM", "SIGTERM"],
    ["SIGTERM", "SIGHUP"],
  ]) {
    test(`${name}: ${second} to the whole group while it handles ${signal} still waits for every child`, async () => {
      const r = await launchAndSignal(template, "hangs", USER_ARGS, {
        settleMs: 200,
        signal,
        group: true,
        slowToGo: true,
        then: { signal: second, afterMs: 150 },
      });
      try {
        assert.equal(r.launched, null, "nothing launched");
        assertNothingLeft(r);
        assert.equal(r.run.signal, signal, `AppRun ended with status ${r.run.status}, not by ${signal}`);
        assert.ok(r.elapsedMs < 3000, `stopped after ${r.elapsedMs} ms`);
        assert.deepEqual(r.tmpLeft, [], "the probe's answer file is removed");
      } finally {
        cleanup(r);
      }
    });
  }
}

// The second stop can also land together with the first, before the
// handler has started: bash then keeps it recorded even once the
// handler ignores it, and a single `wait` in the handler can return at
// once with the child still running, so the handler would exit with it
// unreaped. The window is microseconds wide, so several launches take
// the pair of signals at once, and every one must still reap all.
for (const [name, template] of Object.entries(TEMPLATES)) {
  test(`${name}: SIGTERM to the whole group together with SIGHUP still waits for every child`, async () => {
    // Settled, not all: a launch that fails must not keep the others'
    // images from being removed.
    const settled = await Promise.allSettled(
      Array.from({ length: 32 }, () =>
        launchAndSignal(template, "hangs", USER_ARGS, {
          settleMs: 200,
          signal: "SIGHUP",
          group: true,
          then: { signal: "SIGTERM", afterMs: 0 },
        }),
      ),
    );
    const runs = settled.filter((s) => s.status !== "rejected").map((s) => s.value);
    try {
      const failed = settled.find((s) => s.status === "rejected");
      if (failed) throw failed.reason;
      for (const r of runs) {
        assert.equal(r.launched, null, "nothing launched");
        assertNothingLeft(r);
        assert.deepEqual(r.tmpLeft, [], "the probe's answer file is removed");
      }
    } finally {
      runs.forEach(cleanup);
    }
  });
}

// The stop handlers reap through ani_gui_reap, which may be handed a
// child the shell has already reaped: a signal between the probe's end
// and the moment its pid is cleared. Bash then answers `wait` with the
// remembered status, never "not a child", and the pid may by then
// belong to another live process. The reap must end at once there, not
// wait on that process. `wait` shadowed to answer 0, as bash does for
// the remembered child, and the test runner's own pid, live and not
// the shell's child, stand in for the reuse, which no test can arrange on
// demand.
test("a reap of a child already reaped ends even when its pid is another live process's", () => {
  const img = image(EB25, "answers");
  try {
    const appRun = fs.readFileSync(path.join(img.app, "AppRun"), "utf8");
    const start = appRun.indexOf("ANI_GUI_SANDBOX=(--no-sandbox)\n");
    const end = appRun.indexOf("ani_gui_user_no_sandbox=0\n");
    assert.ok(start > 0 && end > start, "the probe's functions are in AppRun");
    assert.match(appRun.slice(start, end), /^ani_gui_reap\(\)$/m, "the cut carries the reap");
    const script = [
      "set -e",
      appRun.slice(start, end),
      "waits=0",
      "wait() { waits=$((waits + 1)) ; return 0 ; }",
      'ani_gui_reap "$LIVE_PID"',
      'echo "$waits" > "$APPDIR/reap-waits"',
    ].join("\n");
    const r = spawnSync("bash", ["-c", script], { env: img.env({ LIVE_PID: String(process.pid) }), encoding: "utf8", timeout: 5000, killSignal: "SIGKILL" });
    const waitsFile = path.join(img.app, "reap-waits");
    assert.ok(fs.existsSync(waitsFile), `the reap did not end (status ${r.status}, signal ${r.signal}): ${r.stderr}`);
    assert.equal(fs.readFileSync(waitsFile, "utf8").trim(), "1", "the reap waited again on a process that is not the shell's child");
    assert.equal(r.status, 0, r.stderr);
  } finally {
    fs.rmSync(img.root, { recursive: true, force: true });
  }
});

// A stale signal can also cut every `wait` short for good: a stress run
// caught the watchdog's subshell spinning with its sleeper a zombie
// nobody collected. The reap must end within a bound however `wait`
// behaves, never spin, and leave the shell able to go on. `wait`
// shadowed to return 143 without collecting anything, and a child that
// outlives the reap, stand in for that bash.
test("a reap whose every wait is cut short still ends within its bound", () => {
  const img = image(EB25, "answers");
  try {
    const appRun = fs.readFileSync(path.join(img.app, "AppRun"), "utf8");
    const start = appRun.indexOf("ANI_GUI_SANDBOX=(--no-sandbox)\n");
    const end = appRun.indexOf("ani_gui_user_no_sandbox=0\n");
    assert.ok(start > 0 && end > start, "the probe's functions are in AppRun");
    const script = [
      "set -e",
      appRun.slice(start, end),
      // In a subshell, as two of the three callers are: the child is
      // the subshell's, not the outer shell's.
      "(",
      "  sleep 30 </dev/null >/dev/null 2>&1 & child=$!",
      '  echo "$child" > "$APPDIR/reap-child"',
      "  waits=0",
      "  wait() { waits=$((waits + 1)) ; return 143 ; }",
      '  ani_gui_reap "$child"',
      '  echo "$waits" > "$APPDIR/reap-ended"',
      '  kill -9 "$child"',
      ")",
    ].join("\n");
    const began = Date.now();
    const r = spawnSync("bash", ["-c", script], { env: img.env({}), encoding: "utf8", timeout: 10000, killSignal: "SIGKILL" });
    const tookMs = Date.now() - began;
    const childFile = path.join(img.app, "reap-child");
    if (fs.existsSync(childFile)) {
      const child = Number(fs.readFileSync(childFile, "utf8"));
      if (liveProcess(child)) process.kill(child, "SIGKILL");
    }
    assert.ok(fs.existsSync(path.join(img.app, "reap-ended")), `the reap did not end (status ${r.status}, signal ${r.signal}): ${r.stderr}`);
    // Its own child, still running: the reap kept at it to the bound
    // (the first wait and a hundred retries), and no further.
    assert.equal(fs.readFileSync(path.join(img.app, "reap-ended"), "utf8").trim(), "101", "the reap gave up on its own running child before its bound");
    assert.ok(tookMs < 5000, `the reap took ${tookMs} ms`);
    assert.equal(r.status, 0, r.stderr);
  } finally {
    fs.rmSync(img.root, { recursive: true, force: true });
  }
});

test("a stop after the probe has ended signals nothing but AppRun itself", async () => {
  const img = image(EB25, "answers");
  try {
    const appRun = fs.readFileSync(path.join(img.app, "AppRun"), "utf8");
    const start = appRun.indexOf("ANI_GUI_SANDBOX=(--no-sandbox)\n");
    const end = appRun.indexOf("ani_gui_user_no_sandbox=0\n");
    assert.ok(start > 0 && end > start, "the probe's functions are in AppRun");
    assert.match(appRun.slice(start, end), /^ani_gui_probe_starting=0$/m, "the cut carries the probe's state");
    const script = [
      `BIN=${JSON.stringify(path.join(img.app, "ani-gui"))}`,
      appRun.slice(start, end),
      'ani_gui_sandbox_starts || exit 3',
      'sleep 30 </dev/null >/dev/null 2>&1 & bystander=$!',
      'echo "$bystander" > "$APPDIR/bystander"',
      "ani_gui_probe_stop TERM",
    ].join("\n");
    const { run, alive } = await runWithBystander(img, script);
    assert.equal(run.signal, "SIGTERM", `status ${run.status}: ${run.stderr}`);
    assert.ok(alive, "the stop handler killed a process that was not the probe");
  } finally {
    fs.rmSync(img.root, { recursive: true, force: true });
  }
});

// The same handler, for a signal that lands while the probe job is
// being started, before the job exists: then `$!` still names
// AppRun's previous background job, if it has one, and that is not
// the probe's to signal. The state is set as the start sequence sets
// it at that moment, with a bystander as the previous job.
test("a stop before the probe job exists signals no earlier background job", async () => {
  const img = image(EB25, "answers");
  try {
    const appRun = fs.readFileSync(path.join(img.app, "AppRun"), "utf8");
    const start = appRun.indexOf("ANI_GUI_SANDBOX=(--no-sandbox)\n");
    const end = appRun.indexOf("ani_gui_user_no_sandbox=0\n");
    const script = [
      appRun.slice(start, end),
      "sleep 30 </dev/null >/dev/null 2>&1 & bystander=$!",
      'echo "$bystander" > "$APPDIR/bystander"',
      "ani_gui_probe_prev_bg=$!",
      "ani_gui_probe_starting=1",
      "ani_gui_probe_stop TERM",
    ].join("\n");
    const { run, alive } = await runWithBystander(img, script);
    assert.equal(run.signal, "SIGTERM", `status ${run.status}: ${run.stderr}`);
    assert.ok(alive, "the stop handler killed AppRun's earlier background job");
  } finally {
    fs.rmSync(img.root, { recursive: true, force: true });
  }
});
