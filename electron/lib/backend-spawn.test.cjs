// The backend must end when the Electron process that spawned it is
// gone, however that process died. The mechanism is a stdin pipe the
// main process holds open and never writes to or closes: when the main
// process dies the OS closes its end, and the backend — told to watch
// by ANI_GUI_PARENT_STDIN — reads end of file and shuts down. So the
// spawn has to hand the backend that pipe and that flag, on every
// platform, without losing the rest of the environment.

const test = require("node:test");
const assert = require("node:assert/strict");

const { backendSpawnOptions } = require("./backend-spawn.cjs");

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
