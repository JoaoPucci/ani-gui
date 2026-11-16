// Quitting when the session's end is announced (Linux): logind's
// PrepareForShutdown through powerMonitor's `shutdown`, and
// gnome-session's SessionOver through a `gdbus monitor` watcher that
// never outlives the app.

const test = require("node:test");
const assert = require("node:assert/strict");
const { EventEmitter, once } = require("node:events");
const { PassThrough } = require("node:stream");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  GNOME_SESSION,
  SESSION_WATCHER,
  isSessionOver,
  lineSplitter,
  watchSessionEnd,
} = require("./session-end.cjs");

const SESSION_OVER = "/org/gnome/SessionManager: org.gnome.SessionManager.SessionOver ()";

/** A powerMonitor, a spawn and a watcher child, all fake. */
function rig({ spawnThrows = false } = {}) {
  const powerMonitor = new EventEmitter();
  const child = new EventEmitter();
  child.stdin = new PassThrough();
  child.stdout = new PassThrough();
  const spawned = [];
  const endings = [];
  const errors = [];
  const handle = watchSessionEnd({
    platform: "linux",
    powerMonitor,
    spawn: (cmd, args, options) => {
      if (spawnThrows) throw new Error("EAGAIN");
      spawned.push({ cmd, args, options });
      return child;
    },
    env: { PATH: "/usr/bin" },
    onEnding: (reason) => endings.push(reason),
    logError: (...args) => errors.push(args),
  });
  const shutdown = () => {
    let prevented = false;
    powerMonitor.emit("shutdown", { preventDefault: () => (prevented = true) });
    return prevented;
  };
  return { powerMonitor, child, spawned, endings, errors, handle, shutdown };
}

test("isSessionOver matches gdbus monitor's line for SessionOver only", () => {
  assert.equal(isSessionOver(SESSION_OVER), true);
  assert.equal(isSessionOver(`  ${SESSION_OVER}\r`), true);
  assert.equal(isSessionOver("/org/gnome/SessionManager: org.gnome.SessionManager.SessionRunning ()"), false);
  assert.equal(isSessionOver("The name org.gnome.SessionManager is owned by :1.12"), false);
  assert.equal(isSessionOver("/org/other: org.gnome.SessionManager.SessionOver ()"), false);
  assert.equal(isSessionOver(undefined), false);
});

test("lineSplitter hands over whole lines and keeps a partial one", () => {
  const lines = [];
  const feed = lineSplitter((line) => lines.push(line));
  feed("a\nb");
  feed("c\n\nd");
  assert.deepEqual(lines, ["a", "bc", ""]);
  feed(Buffer.from("\n"));
  assert.deepEqual(lines, ["a", "bc", "", "d"]);
});

test("off Linux nothing is listened for", () => {
  const powerMonitor = new EventEmitter();
  let spawned = 0;
  for (const platform of ["win32", "darwin"]) {
    const handle = watchSessionEnd({
      platform,
      powerMonitor,
      spawn: () => (spawned += 1),
      env: {},
      onEnding: () => assert.fail("no ending off Linux"),
      logError: () => {},
    });
    handle.stop();
  }
  assert.equal(powerMonitor.listenerCount("shutdown"), 0);
  assert.equal(spawned, 0);
});

test("a shutdown announcement quits the app and holds the shutdown for it", () => {
  const { endings, shutdown } = rig();
  assert.equal(shutdown(), true, "preventDefault keeps Electron's delay inhibitor held");
  assert.deepEqual(endings, ["shutdown"]);
});

/**
 * Electron's powerMonitor as of 33 (lib/browser/api/power-monitor.ts,
 * unchanged on main): the native monitor is created on the first
 * listener and, on Linux, told whether a `shutdown` listener exists
 * before that first listener has been added. Only a `shutdown`
 * listener added later goes through the path that counts it.
 */
function electronPowerMonitor() {
  const pm = new EventEmitter();
  pm.listening = [];
  pm.once("newListener", () => {
    pm.listening.push(pm.listenerCount("shutdown") > 0);
    pm.on("newListener", (event) => {
      if (event === "shutdown") pm.listening.push(pm.listenerCount("shutdown") + 1 > 0);
    });
  });
  return pm;
}

test("Electron is told of the shutdown listener, so it holds the inhibitor", () => {
  const powerMonitor = electronPowerMonitor();
  watchSessionEnd({
    platform: "linux",
    powerMonitor,
    spawn: () => {
      throw new Error("no watcher here");
    },
    env: {},
    onEnding: () => {},
    logError: () => {},
  });
  assert.equal(powerMonitor.listening.at(-1), true);
});

test("SessionOver from the watcher quits the app", () => {
  const { child, endings } = rig();
  child.stdout.write("The name org.gnome.SessionManager is owned by :1.5\n");
  assert.deepEqual(endings, []);
  child.stdout.write(`${SESSION_OVER.slice(0, 20)}`);
  child.stdout.write(`${SESSION_OVER.slice(20)}\n`);
  assert.deepEqual(endings, ["logout"]);
});

test("the session ends once, however many announcements arrive", () => {
  const { child, endings, shutdown } = rig();
  child.stdout.write(`${SESSION_OVER}\n`);
  shutdown();
  child.stdout.write(`${SESSION_OVER}\n`);
  assert.deepEqual(endings, ["logout"]);
});

test("the watcher runs gdbus monitor on gnome-session, in the env given", () => {
  const { spawned } = rig();
  assert.equal(spawned.length, 1);
  const [{ cmd, args, options }] = spawned;
  assert.equal(cmd, SESSION_WATCHER.cmd);
  assert.deepEqual(args, SESSION_WATCHER.args);
  assert.deepEqual(options.env, { PATH: "/usr/bin" });
  assert.deepEqual(options.stdio, ["pipe", "pipe", "ignore"]);
  assert.deepEqual(args.slice(-2), [GNOME_SESSION.service, GNOME_SESSION.path]);
});

test("stop closes the watcher's input, once", () => {
  const { child, handle } = rig();
  let ended = 0;
  child.stdin.on("finish", () => (ended += 1));
  handle.stop();
  handle.stop();
  return new Promise((resolve) =>
    setImmediate(() => {
      assert.equal(ended, 1);
      resolve();
    }),
  );
});

test("a watcher that cannot start is logged, and shutdown still works", () => {
  const { errors, endings, shutdown, handle } = rig({ spawnThrows: true });
  assert.equal(errors.length, 1);
  handle.stop();
  shutdown();
  assert.deepEqual(endings, ["shutdown"]);
});

test("a watcher that fails to spawn is logged", () => {
  const { child, errors } = rig();
  child.emit("error", new Error("ENOENT"));
  assert.equal(errors.length, 1);
});

// The watcher's shell itself, run for real against a stand-in gdbus.

function fakeGdbusDir(script) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "session-watch-"));
  if (script !== null) {
    fs.writeFileSync(path.join(dir, "gdbus"), `#!/bin/sh\n${script}\n`, { mode: 0o755 });
  }
  return dir;
}

function runWatcher(dir) {
  return spawn(SESSION_WATCHER.cmd, SESSION_WATCHER.args, {
    env: { PATH: `${dir}:/usr/bin:/bin` },
    stdio: ["pipe", "pipe", "ignore"],
  });
}

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

test("the watcher passes gdbus monitor's output through, with gnome-session's names", async (t) => {
  const dir = fakeGdbusDir('echo "$*"; exec sleep 30');
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const watcher = runWatcher(dir);
  const [chunk] = await once(watcher.stdout, "data");
  assert.equal(
    String(chunk).trim(),
    `monitor --session --dest ${GNOME_SESSION.service} --object-path ${GNOME_SESSION.path}`,
  );
  watcher.stdin.end();
  const [code] = await once(watcher, "exit");
  assert.equal(code, 0);
});

test("the watcher ends its gdbus when its input closes", async (t) => {
  const dir = fakeGdbusDir('echo "$$"; exec sleep 30');
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const watcher = runWatcher(dir);
  const [chunk] = await once(watcher.stdout, "data");
  const gdbusPid = Number(String(chunk).trim());
  assert.ok(alive(gdbusPid));
  watcher.stdin.end();
  await once(watcher, "exit");
  for (let i = 0; i < 100 && alive(gdbusPid); i += 1) await new Promise((r) => setTimeout(r, 10));
  assert.equal(alive(gdbusPid), false);
});

test("without gdbus the watcher exits at once", async (t) => {
  const dir = fakeGdbusDir(null);
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const watcher = spawn(SESSION_WATCHER.cmd, SESSION_WATCHER.args, {
    // No gdbus anywhere on this PATH.
    env: { PATH: dir },
    stdio: ["pipe", "pipe", "ignore"],
  });
  const [code] = await once(watcher, "exit");
  assert.equal(code, 0);
});
