// Restart, from the splash's stuck state, starts the whole app again:
// a new process, launched the way the person launched this one, once
// this one has exited and let go of the single-instance lock.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const fc = require("fast-check");

const { LAUNCHER_SWITCHES, WAIT_THEN_EXEC, relaunch, relaunchPlan, userArgs } = require("./relaunch.cjs");

const appImageEnv = {
  APPIMAGE: "/home/me/Apps/ani-gui-0.18.0-x86_64.AppImage",
  APPDIR: "/tmp/.mount_ani-guXYZ",
  OWD: "/home/me",
  HOME: "/home/me",
};
const appImageExec = "/tmp/.mount_ani-guXYZ/ani-gui.bin";

test("the launcher's own switch is the AppImage wrapper's; AppRun's sandbox switch is decided per launch", () => {
  // electron/build-resources/after-pack.cjs puts --enable-wayland-ime
  // first. AppRun (lib/appimage-tree.cjs) passes --no-sandbox only when
  // the sandbox probe got no answer, or when the person passed it: it is
  // not a switch every launch carries, so the restart keeps it.
  assert.deepEqual(LAUNCHER_SWITCHES, ["--enable-wayland-ime"]);
  const wrapper = fs.readFileSync(path.join(__dirname, "..", "build-resources", "after-pack.cjs"), "utf-8");
  assert.match(wrapper, /exec "\$HERE\/\$\{exe\}\.bin" --enable-wayland-ime "\$@"/);
  const tree = fs.readFileSync(path.join(__dirname, "appimage-tree.cjs"), "utf-8");
  assert.match(tree, /exec "\$BIN" "\$\{ANI_GUI_SANDBOX\[@\]\}" "\$\{args\[@\]\}"/);
  assert.doesNotMatch(tree, /exec "\$BIN" --no-sandbox/);
});

test("in the AppImage, a --no-sandbox the launch ran with is kept for the restart", () => {
  // Whether the probe's fallback added it or the person passed it, the
  // new AppRun reads it as the person's own and starts without the
  // sandbox, as this launch did, without probing again.
  for (const rest of [[], ["--some-user-flag"]]) {
    const plan = relaunchPlan({
      platform: "linux",
      env: appImageEnv,
      execPath: appImageExec,
      argv: [appImageExec, "--enable-wayland-ime", "--no-sandbox", ...rest],
      pid: 7,
    });
    assert.deepEqual(plan.args.slice(5), ["--no-sandbox", ...rest]);
  }
  // A sandboxed launch carries none, and its restart probes afresh.
  const sandboxed = relaunchPlan({
    platform: "linux",
    env: appImageEnv,
    execPath: appImageExec,
    argv: [appImageExec, "--enable-wayland-ime", "--some-user-flag"],
    pid: 7,
  });
  assert.deepEqual(sandboxed.args.slice(5), ["--some-user-flag"]);
});

test("in the AppImage, the relaunch waits for this process and then runs the AppImage file itself", () => {
  const plan = relaunchPlan({
    platform: "linux",
    env: appImageEnv,
    execPath: appImageExec,
    argv: [appImageExec, "--enable-wayland-ime", "--some-user-flag"],
    pid: 4242,
  });
  assert.equal(plan.kind, "spawn");
  assert.equal(plan.command, "/bin/sh");
  assert.deepEqual(plan.args, [
    "-c",
    WAIT_THEN_EXEC,
    "ani-gui-restart",
    "4242",
    appImageEnv.APPIMAGE,
    "--some-user-flag",
  ]);
  // The mount's own directory is the dying one's: the new AppImage
  // gets its own.
  assert.equal(plan.env.APPDIR, undefined);
  assert.equal(plan.env.HOME, "/home/me");
  assert.equal(plan.env.APPIMAGE, appImageEnv.APPIMAGE);
});

test("everywhere else the plan is Electron's own relaunch (this binary, these arguments)", () => {
  const cases = [
    // The .deb, through its wrapper's environment and switches.
    { platform: "linux", env: { HOME: "/h" }, execPath: "/opt/ani-gui/ani-gui.bin" },
    // The Windows installer's per-user install.
    {
      platform: "win32",
      env: { USERPROFILE: "C:\\Users\\me" },
      execPath: "C:\\Users\\me\\AppData\\Local\\Programs\\ani-gui\\ani-gui.exe",
    },
    // pnpm dev.
    { platform: "linux", env: { ELECTRON_DEV: "1" }, execPath: "/repo/electron/node_modules/electron/dist/electron" },
    // An APPIMAGE leaked into the environment by another AppImage (an
    // editor's terminal) while this process runs from somewhere else.
    { platform: "linux", env: appImageEnv, execPath: "/opt/ani-gui/ani-gui.bin" },
    { platform: "linux", env: { APPIMAGE: appImageEnv.APPIMAGE }, execPath: appImageExec },
    // Not Linux, whatever the environment says.
    { platform: "win32", env: appImageEnv, execPath: appImageExec },
    { platform: "darwin", env: appImageEnv, execPath: appImageExec },
  ];
  for (const c of cases) {
    assert.deepEqual(relaunchPlan({ ...c, argv: [c.execPath, "--x"], pid: 1 }), { kind: "relaunch" }, JSON.stringify(c));
  }
});

test("the mount is compared by its real path, so a symlinked /tmp still finds the AppImage", () => {
  const real = { "/tmp/.mount_ani-guXYZ": "/private/tmp/.mount_ani-guXYZ" };
  const plan = relaunchPlan({
    platform: "linux",
    env: appImageEnv,
    execPath: "/private/tmp/.mount_ani-guXYZ/ani-gui.bin",
    argv: ["/private/tmp/.mount_ani-guXYZ/ani-gui.bin"],
    pid: 9,
    realpath: (p) => real[p] ?? p,
  });
  assert.equal(plan.kind, "spawn");
  // A path that cannot be resolved is compared as it is.
  const unresolved = relaunchPlan({
    platform: "linux",
    env: appImageEnv,
    execPath: appImageExec,
    argv: [appImageExec],
    pid: 9,
    realpath: () => {
      throw new Error("ENOENT");
    },
  });
  assert.equal(unresolved.kind, "spawn");
});

test("a path only beside the mount is not inside it", () => {
  const plan = relaunchPlan({
    platform: "linux",
    env: appImageEnv,
    execPath: "/tmp/.mount_ani-guXYZ-other/ani-gui.bin",
    argv: [],
    pid: 1,
  });
  assert.deepEqual(plan, { kind: "relaunch" });
});

test("property: only the launcher's own leading switches are dropped from the person's arguments", () => {
  const arg = fc.oneof(fc.constantFrom(...LAUNCHER_SWITCHES, "--no-sandbox", "--x", "-v", ""), fc.string());
  fc.assert(
    fc.property(fc.array(arg, { maxLength: 6 }), (rest) => {
      assert.deepEqual(userArgs(["bin", ...LAUNCHER_SWITCHES, ...rest]), rest);
    }),
  );
  // An argv without them (a launch that bypassed the wrapper) keeps all.
  fc.assert(
    fc.property(fc.array(arg, { maxLength: 6 }), (rest) => {
      fc.pre(!LAUNCHER_SWITCHES.includes(rest[0]));
      assert.deepEqual(userArgs(["bin", ...rest]), rest);
    }),
  );
});

test("Electron's relaunch is asked for once, with nothing changed", () => {
  const calls = [];
  relaunch({ kind: "relaunch" }, { app: { relaunch: (...a) => calls.push(a) }, spawn: () => assert.fail() });
  assert.deepEqual(calls, [[]]);
});

test("the waiter is spawned detached, silent and let go, with its own environment", () => {
  const calls = [];
  let unref = 0;
  const child = { unref: () => (unref += 1), on: (e) => calls.push(`on:${e}`) };
  const plan = { kind: "spawn", command: "/bin/sh", args: ["-c", "x"], env: { A: "1" } };
  relaunch(plan, {
    app: { relaunch: () => assert.fail() },
    spawn: (cmd, args, opts) => (calls.push([cmd, args, opts]), child),
    exists: () => true,
    logError: () => {},
  });
  assert.deepEqual(calls[0], ["/bin/sh", ["-c", "x"], { detached: true, stdio: "ignore", env: { A: "1" } }]);
  assert.ok(calls.includes("on:error"), "an error after the spawn is logged, not thrown");
  assert.equal(unref, 1);
});

test("a waiter that cannot run says so before anything quits", () => {
  assert.throws(
    () =>
      relaunch(
        { kind: "spawn", command: "/bin/sh", args: [], env: {} },
        { app: {}, spawn: () => assert.fail(), exists: () => false, logError: () => {} },
      ),
    /\/bin\/sh/,
  );
});

test("the waiter runs its command only once the process it waits on has exited", async (t) => {
  if (process.platform === "win32") return t.skip("POSIX shell");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-relaunch-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const out = path.join(dir, "ran");
  // Stands in for the app: alive for a while, then gone.
  const app = spawn(process.execPath, ["-e", "setTimeout(() => {}, 600)"], { stdio: "ignore" });
  const exited = new Promise((resolve) => app.on("exit", () => resolve(Date.now())));
  const target = [process.execPath, "-e", `require("fs").writeFileSync(${JSON.stringify(out)}, String(Date.now()))`, "with space", "$HOME"];
  const plan = {
    kind: "spawn",
    command: "/bin/sh",
    args: ["-c", WAIT_THEN_EXEC, "ani-gui-restart", String(app.pid), ...target],
    env: process.env,
  };
  // Not detached here, so the test can wait on it.
  const waiter = spawn(plan.command, plan.args, { stdio: "ignore", env: plan.env });
  const done = new Promise((resolve) => waiter.on("exit", resolve));
  const gone = await exited;
  assert.equal(await done, 0);
  const ran = Number(fs.readFileSync(out, "utf-8"));
  assert.ok(ran >= gone, `ran at ${ran}, the app went at ${gone}`);
});
