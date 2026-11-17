// The Linux launcher (`ani-gui`, written by build-resources/after-pack.cjs
// from lib/linux-launcher.cjs) run for real, with a stand-in for the app's
// binary (fixtures/app-stub.py) that records what reached it.
//
// Launched off a terminal — from the desktop, into a pipe or a file — the
// launcher becomes the app, as it always has. Launched from a terminal, the
// app holds none of it: its output goes through the launcher to the
// terminal, and the launcher hands it one SIGTERM however the terminal
// stops it. That is what keeps a closed terminal from aborting the app as
// it exits (Node puts a terminal it holds back as it found it, and on a
// terminal that has hung up that step aborts).
//
// The terminal is e2e/fixtures/terminal/on-a-terminal.py: the launcher
// leads the terminal's session there, as a shell's foreground job leads
// its process group. Every process signalled here is one this file
// started, by the pid it recorded or the group it made.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, execFileSync } = require("node:child_process");

const { launcherScript } = require("./linux-launcher.cjs");

const STUB = path.join(__dirname, "fixtures", "app-stub.py");
const TERMINAL = path.join(__dirname, "..", "e2e", "fixtures", "terminal", "on-a-terminal.py");

const linux = process.platform === "linux";
function hasPython() {
  try {
    execFileSync("python3", ["-c", "import pty"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}
const skip = !linux ? "the launcher is Linux's" : !hasPython() ? "needs python3 for the stub and the terminal" : false;

const scratch = [];
test.after(() => {
  for (const dir of scratch) fs.rmSync(dir, { recursive: true, force: true });
});

/** A packed tree: the launcher and the stub under the binary's name. */
function packedTree() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-launcher-"));
  scratch.push(dir);
  fs.writeFileSync(path.join(dir, "ani-gui"), launcherScript("ani-gui"), { mode: 0o755 });
  fs.chmodSync(path.join(dir, "ani-gui"), 0o755);
  fs.copyFileSync(STUB, path.join(dir, "ani-gui.bin"));
  fs.chmodSync(path.join(dir, "ani-gui.bin"), 0o755);
  return { dir, launcher: path.join(dir, "ani-gui"), events: path.join(dir, "events.jsonl") };
}

/** What the stub has recorded so far. */
function events(file) {
  let text = "";
  try {
    text = fs.readFileSync(file, "utf8");
  } catch {
    return [];
  }
  return text
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => JSON.parse(l));
}
const startOf = (file) => events(file).find((e) => e.start)?.start;
const signalsOf = (file) => events(file).filter((e) => e.signal).map((e) => e.signal);
const exitOf = (file) => events(file).find((e) => "exit" in e)?.exit;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, check, ms = 10_000) {
  const deadline = Date.now() + ms;
  for (;;) {
    const value = check();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(20);
  }
}

function stubEnv(tree, extra = {}) {
  const env = { ...process.env };
  delete env.ELECTRON_OZONE_PLATFORM_HINT;
  Object.assign(env, { STUB_EVENTS: tree.events }, extra);
  for (const [k, v] of Object.entries(extra)) if (v === undefined) delete env[k];
  return env;
}

/** Kills the stub by its recorded pid if it is still running. */
function stopStub(tree) {
  const start = startOf(tree.events);
  if (!start || exitOf(tree.events) !== undefined) return;
  try {
    process.kill(start.pid, "SIGKILL");
  } catch {
    /* already gone */
  }
}

/**
 * The launcher on a terminal of its own. Returns the fixture's process,
 * the launcher's pid, what the terminal showed (`screen()`), and the
 * launcher's ending (`ended()`: {exit} or {signal}).
 */
async function onTerminal(tree, { env = {}, args = [] } = {}) {
  const fixture = spawn("python3", [TERMINAL, tree.launcher, ...args], {
    env: stubEnv(tree, env),
    stdio: ["pipe", "pipe", "pipe"],
  });
  fixture.stdin.on("error", () => {});
  let out = "";
  let screen = "";
  fixture.stdout.on("data", (c) => (out += c));
  fixture.stderr.on("data", (c) => (screen += c));
  const reports = () =>
    out
      .split("\n")
      .filter((l) => l.trim())
      .map((l) => JSON.parse(l));
  const pid = await until("the launcher's pid", () => reports().find((r) => "pid" in r)?.pid);
  return {
    fixture,
    pid,
    screen: () => screen.replace(/\r\n/g, "\n"),
    ended: () => reports().find((r) => !("pid" in r)),
    send: (command) => fixture.stdin.write(`${command}\n`),
    close: async () => {
      if (fixture.exitCode === null) {
        fixture.stdin.write("kill\n");
        fixture.stdin.end();
        await new Promise((r) => fixture.once("exit", r));
      }
      stopStub(tree);
    },
  };
}

const usr1 = os.constants.signals.SIGUSR1;

test("off a terminal, the launcher becomes the app, with the hint and its switch before the person's arguments", { skip }, async () => {
  const tree = packedTree();
  const child = spawn(tree.launcher, ["--some-flag"], {
    env: stubEnv(tree, { STUB_MODE: "exit", STUB_EXIT: "4" }),
    stdio: ["pipe", "pipe", "pipe"],
  });
  let out = "";
  child.stdout.on("data", (c) => (out += c));
  const [code] = await new Promise((r) => child.once("exit", (...a) => r(a)));
  const start = startOf(tree.events);
  assert.equal(start.pid, child.pid, "the launcher execs the app: one process");
  assert.deepEqual(start.argv, ["--enable-wayland-ime", "--some-flag"]);
  assert.equal(start.hint, "auto");
  assert.deepEqual(start.tty, [false, false, false]);
  assert.equal(code, 4);
  assert.equal(out, "out 1\nout 2\nout 3\n");
});

test("a platform hint the person exported is kept", { skip }, async () => {
  const tree = packedTree();
  const child = spawn(tree.launcher, [], {
    env: stubEnv(tree, { STUB_MODE: "exit", ELECTRON_OZONE_PLATFORM_HINT: "x11" }),
    stdio: "ignore",
  });
  await new Promise((r) => child.once("exit", r));
  assert.equal(startOf(tree.events).hint, "x11");
});

test("on a terminal, the app holds none of it, and its output reaches the terminal in order", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree, { env: { STUB_LINES: "400" }, args: ["--some-flag"] });
  try {
    const start = await until("the app's start", () => startOf(tree.events));
    assert.deepEqual(start.tty, [false, false, false], "none of the app's stdio is the terminal");
    assert.equal(start.stdin, "/dev/null");
    assert.notEqual(start.pid, term.pid, "the launcher stays, beside the app");
    assert.notEqual(start.sid, term.pid, "the app is outside the terminal's session");
    assert.deepEqual(start.argv, ["--enable-wayland-ime", "--some-flag"]);
    assert.equal(start.hint, "auto");
    await until("the app's output on the terminal", () => term.screen().includes("err 400\n"));
    const lines = term.screen().split("\n");
    const outs = lines.filter((l) => l.startsWith("out "));
    const errs = lines.filter((l) => l.startsWith("err "));
    assert.deepEqual(outs, Array.from({ length: 400 }, (_, i) => `out ${i + 1}`));
    assert.deepEqual(errs, Array.from({ length: 400 }, (_, i) => `err ${i + 1}`));
  } finally {
    await term.close();
  }
});

test("closing the terminal hands the app one SIGTERM, and it quits cleanly", { skip }, async () => {
  const tree = packedTree();
  // More than a pipe holds, so the app is still writing once the relay
  // has gone with the terminal.
  const term = await onTerminal(tree, { env: { STUB_BYE_LINES: "20000" } });
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    term.send("hangup");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    assert.equal(exitOf(tree.events), 0);
    // The relay ended at its first write to the closed terminal: the
    // app's writes fail as a pipe's do, never as a dead terminal's.
    const failures = events(tree.events).filter((e) => e.write_error).map((e) => e.write_error);
    assert.deepEqual(failures, ["EPIPE"]);
  } finally {
    await term.close();
  }
});

test("Ctrl-C hands the app one SIGTERM, and its last lines still reach the terminal", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    // What the terminal does on Ctrl-C: SIGINT to its foreground group.
    process.kill(-term.pid, "SIGINT");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    // The terminal's own report of them can come after the launcher's
    // ending is reported.
    await until("the app's last lines on the terminal", () => /bye 3\n/.test(term.screen()) && /bye-err 3\n/.test(term.screen()));
  } finally {
    await term.close();
  }
});

test("a SIGTERM to the launcher's group reaches the app once", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    process.kill(-term.pid, "SIGTERM");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
  } finally {
    await term.close();
  }
});

test("a stop that reaches the app as well as the launcher is not passed on a second time", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    const start = await until("the app to be ready", () => events(tree.events).some((e) => e.ready) && startOf(tree.events));
    // A session's scope stopped, or a kill by name: every process gets one.
    process.kill(term.pid, "SIGTERM");
    process.kill(start.pid, "SIGTERM");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    assert.equal(exitOf(tree.events), 0);
  } finally {
    await term.close();
  }
});

test("a second stop signal to the launcher is not passed on", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    process.kill(term.pid, "SIGHUP");
    await sleep(50);
    process.kill(term.pid, "SIGTERM");
    await sleep(50);
    process.kill(term.pid, "SIGINT");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
  } finally {
    await term.close();
  }
});

test("the launcher ends with the app's exit status", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree, { env: { STUB_MODE: "exit", STUB_EXIT: "3" } });
  try {
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 3 });
    assert.match(term.screen(), /out 3\n/);
  } finally {
    await term.close();
  }
});

test("an app that dies by a signal ends the launcher with the shell's status for it", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree, { env: { STUB_MODE: "die" } });
  try {
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 128 + usr1 });
  } finally {
    await term.close();
  }
});

test("a launcher killed outright takes the app down with one SIGTERM, not an orphan", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    term.send("kill");
    await until("the app's exit", () => exitOf(tree.events) !== undefined, 15_000);
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    assert.equal(exitOf(tree.events), 0);
  } finally {
    await term.close();
  }
});

/** A PATH with what the launcher and the stub need, less `missing`. */
function pathWithout(tree, missing) {
  const bin = path.join(tree.dir, "bin");
  fs.mkdirSync(bin);
  for (const tool of ["env", "python3", "bash", "cat", "dirname", "readlink", "sleep", "setsid", "setpriv"]) {
    if (missing.includes(tool)) continue;
    let real;
    try {
      // python3 may be a version manager's shim (pyenv), a script that
      // needs tools left out of this PATH: link the interpreter itself.
      real =
        tool === "python3"
          ? execFileSync("python3", ["-c", "import sys; print(sys.executable)"], { encoding: "utf8" }).trim()
          : execFileSync("sh", ["-c", `command -v ${tool}`], { encoding: "utf8" }).trim();
    } catch {
      continue;
    }
    fs.symlinkSync(real, path.join(bin, tool));
  }
  return bin;
}

test("without setsid, the launcher becomes the app as it always has", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree, { env: { PATH: pathWithout(tree, ["setsid"]), STUB_MODE: "exit" } });
  try {
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.equal(startOf(tree.events).pid, term.pid);
    assert.deepEqual(term.ended(), { exit: 0 });
  } finally {
    await term.close();
  }
});

test("without setpriv, the app is still off the terminal", { skip }, async () => {
  const tree = packedTree();
  const term = await onTerminal(tree, { env: { PATH: pathWithout(tree, ["setpriv"]) } });
  try {
    const start = await until("the app to be ready", () => events(tree.events).some((e) => e.ready) && startOf(tree.events));
    assert.deepEqual(start.tty, [false, false, false]);
    term.send("hangup");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
  } finally {
    await term.close();
  }
});

test("a launcher whose own file goes away while the app runs still hands it the stop and ends with its status, its output still held", { skip }, async () => {
  // The AppImage's launcher sits on its FUSE mount, which can go before
  // the launcher is done with it; bash reads a script as it runs it.
  const tree = packedTree();
  // Something still holds the app's output as it exits (a helper, a
  // restarted app), so the launcher waits on its relays after the app.
  const term = await onTerminal(tree, { env: { STUB_EXIT: "5", STUB_HOLD_MS: "1000" } });
  try {
    await until("the app to be ready", () => events(tree.events).some((e) => e.ready));
    fs.truncateSync(tree.launcher, 0);
    // A stop that leaves the terminal, and so the relays, in place.
    process.kill(term.pid, "SIGTERM");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    assert.equal(exitOf(tree.events), 5);
    assert.deepEqual(term.ended(), { exit: 5 });
  } finally {
    await term.close();
  }
});

/** The launcher on a terminal, with some of its streams sent elsewhere
 *  first, as `ani-gui >log 2>&1` does: a shell that redirects and then
 *  becomes the launcher. */
async function onTerminalRedirected(tree, redirect) {
  const shell = path.join(tree.dir, "redirected");
  fs.writeFileSync(shell, `#!/bin/bash\nexec "${tree.launcher}" ${redirect}\n`, { mode: 0o755 });
  fs.chmodSync(shell, 0o755);
  return onTerminal({ ...tree, launcher: shell });
}

test("with only stdin on the terminal, the app's output goes where it was sent and it still holds no terminal", { skip }, async () => {
  const tree = packedTree();
  const log = path.join(tree.dir, "log");
  const term = await onTerminalRedirected(tree, `>"${log}" 2>&1`);
  try {
    const start = await until("the app to be ready", () => events(tree.events).some((e) => e.ready) && startOf(tree.events));
    assert.deepEqual(start.tty, [false, false, false]);
    assert.equal(start.stdin, "/dev/null");
    term.send("hangup");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    const text = fs.readFileSync(log, "utf8");
    assert.match(text, /out 3\n/);
    assert.match(text, /err 3\n/);
    assert.doesNotMatch(term.screen(), /out 1|err 1/);
  } finally {
    await term.close();
  }
});

test("with stdout sent to a file, the app's stdout goes there and only its stderr is relayed to the terminal", { skip }, async () => {
  const tree = packedTree();
  const log = path.join(tree.dir, "log");
  const term = await onTerminalRedirected(tree, `>"${log}"`);
  try {
    const start = await until("the app to be ready", () => events(tree.events).some((e) => e.ready) && startOf(tree.events));
    assert.deepEqual(start.tty, [false, false, false]);
    await until("the app's stderr on the terminal", () => term.screen().includes("err 3\n"));
    assert.doesNotMatch(term.screen(), /out 1/);
    process.kill(-term.pid, "SIGINT");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    const text = fs.readFileSync(log, "utf8");
    assert.match(text, /out 3\n/);
    assert.match(text, /bye 3\n/);
    assert.doesNotMatch(text, /err 1/);
  } finally {
    await term.close();
  }
});

test("a stop that reaches the launcher twice and then the app is not passed on on top of the app's own", { skip }, async () => {
  // One stop reaching the launcher more than once and the app as well,
  // a moment apart: a SIGHUP to the launcher, a SIGINT to its group,
  // then a SIGTERM to the app. The launcher's pause before it decides
  // must outlast them, or it sends the app a SIGTERM on top of the one
  // it has already had, which kills it outright.
  const tree = packedTree();
  const term = await onTerminal(tree);
  try {
    const start = await until("the app to be ready", () => events(tree.events).some((e) => e.ready) && startOf(tree.events));
    process.kill(term.pid, "SIGHUP");
    await sleep(50);
    process.kill(-term.pid, "SIGINT");
    await sleep(50);
    process.kill(start.pid, "SIGTERM");
    await until("the launcher's ending", () => term.ended(), 15_000);
    assert.deepEqual(term.ended(), { exit: 0 });
    assert.deepEqual(signalsOf(tree.events), ["TERM"]);
    assert.equal(exitOf(tree.events), 0);
  } finally {
    await term.close();
  }
});
