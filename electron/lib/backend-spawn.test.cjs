// The backend must end when the Electron process that spawned it is
// gone, however that process died. The mechanism is a stdin pipe the
// main process holds open and never writes to or closes: when the main
// process dies the OS closes its end, and the backend — told to watch
// by ANI_GUI_PARENT_STDIN — reads end of file and shuts down. So the
// spawn has to hand the backend that pipe and that flag, on every
// platform, without losing the rest of the environment.

const test = require("node:test");
const assert = require("node:assert/strict");

const { backendSpawnOptions, launchBackend } = require("./backend-spawn.cjs");

for (const platform of ["linux", "win32", "darwin"]) {
  test(`the backend gets a parent pipe on stdin and the flag to watch it (${platform})`, () => {
    const opts = backendSpawnOptions({ platform, env: { PATH: "/bin", ANI_GUI_DEV: "1" } });
    assert.equal(opts.stdio[0], "pipe");
    assert.deepEqual(opts.stdio.slice(1), ["pipe", "pipe"]);
    assert.equal(opts.env.ANI_GUI_PARENT_STDIN, "1");
    assert.equal(opts.env.PATH, "/bin");
    assert.equal(opts.env.ANI_GUI_DEV, "1");
  });
}

test("the backend runs in its own process group where there are process groups", () => {
  assert.equal(backendSpawnOptions({ platform: "linux", env: {} }).detached, true);
  assert.equal(backendSpawnOptions({ platform: "darwin", env: {} }).detached, true);
  assert.equal(backendSpawnOptions({ platform: "win32", env: {} }).detached, false);
});

test("the caller's environment is not modified", () => {
  const env = { PATH: "/bin" };
  backendSpawnOptions({ platform: "linux", env });
  assert.deepEqual(env, { PATH: "/bin" });
});

// The quit path can stop only a backend it has a handle on, and the
// handshake can take minutes on a first run (lib/backend-handshake.cjs).
// A quit in that time — or a boot that fails during it — must find the
// child already tracked, or it leaves the backend running.

/** A launch whose spawn returns `child` and whose handshake waits on the test. */
function pendingLaunch(child) {
  const tracked = [];
  let finish;
  let fail;
  const launched = launchBackend({
    spawn: () => child,
    bin: "/opt/ani-gui/ani-gui-backend",
    platform: "linux",
    env: {},
    track: (c) => tracked.push(c),
    handshake: () =>
      new Promise((resolve, reject) => {
        finish = resolve;
        fail = reject;
      }),
  });
  return { tracked, launched, finish: (v) => finish(v), fail: (e) => fail(e) };
}

test("the backend is tracked from the moment it is spawned, not once it answers", async () => {
  const child = { pid: 4242 };
  const launch = pendingLaunch(child);
  await Promise.resolve();
  assert.deepEqual(launch.tracked, [child]);
  launch.finish({ apiBase: "http://127.0.0.1:1", internalSecret: "s" });
  assert.deepEqual(await launch.launched, {
    child,
    apiBase: "http://127.0.0.1:1",
    internalSecret: "s",
  });
  assert.deepEqual(launch.tracked, [child], "tracked once");
});

test("a failed handshake leaves the backend tracked for the boot's stop", async () => {
  const child = { pid: 4242 };
  const launch = pendingLaunch(child);
  launch.fail(new Error("backend did not complete its handshake"));
  await assert.rejects(launch.launched, /handshake/);
  assert.deepEqual(launch.tracked, [child]);
});
