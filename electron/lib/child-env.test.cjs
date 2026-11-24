"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fc = require("fast-check");

const { childEnv, launchSnapshot, withUserEnv } = require("./child-env.cjs");

// What Electron 33 leaves in its own environment on Ubuntu's GNOME
// session by the time main.js runs: XDG_CURRENT_DESKTOP rewritten to
// Unity with the session's value kept beside it, and CHROME_DESKTOP
// naming the app's desktop file (lib/browser/init.ts).
const LAUNCH = Object.freeze({
  HOME: "/home/me",
  PATH: "/usr/bin:/bin",
  XDG_SESSION_TYPE: "wayland",
  XDG_CURRENT_DESKTOP: "Unity",
  ORIGINAL_XDG_CURRENT_DESKTOP: "ubuntu:GNOME",
  CHROME_DESKTOP: "ani-gui.desktop",
});

// The same environment once Chromium's GTK start-up has run: it adds
// GDK_BACKEND (and, under X11, NO_AT_BRIDGE) after main.js's top level.
const NOW = Object.freeze({
  ...LAUNCH,
  GDK_BACKEND: "wayland",
  NO_AT_BRIDGE: "1",
});

const linux = (launch, now = launch) => childEnv({ platform: "linux", launch, now });

test("a child gets the session's desktop back, and no copy of it", () => {
  const env = linux(LAUNCH, NOW);
  assert.strictEqual(env.XDG_CURRENT_DESKTOP, "ubuntu:GNOME");
  assert.ok(!("ORIGINAL_XDG_CURRENT_DESKTOP" in env));
});

test("a session with no desktop name leaves the child with none", () => {
  // Electron copies an unset XDG_CURRENT_DESKTOP as the string "undefined"
  // and leaves XDG_CURRENT_DESKTOP itself unset.
  const launch = { HOME: "/home/me", ORIGINAL_XDG_CURRENT_DESKTOP: "undefined" };
  const env = linux(launch);
  assert.ok(!("XDG_CURRENT_DESKTOP" in env));
  assert.ok(!("ORIGINAL_XDG_CURRENT_DESKTOP" in env));
});

test("a desktop Electron did not rewrite is kept as it is", () => {
  // Electron 38 and later no longer rewrite it, nor keep a copy.
  const env = linux({ HOME: "/home/me", XDG_CURRENT_DESKTOP: "KDE" });
  assert.strictEqual(env.XDG_CURRENT_DESKTOP, "KDE");
});

test("the app's desktop file name never reaches a child", () => {
  assert.ok(!("CHROME_DESKTOP" in linux(LAUNCH, NOW)));
});

test("a variable added after main.js started is not passed on", () => {
  const env = linux(LAUNCH, NOW);
  assert.ok(!("GDK_BACKEND" in env));
  assert.ok(!("NO_AT_BRIDGE" in env));
});

test("a variable the user set and Chromium overwrote is passed on as the user set it", () => {
  const env = linux({ ...LAUNCH, GDK_BACKEND: "wayland" }, { ...NOW, GDK_BACKEND: "x11" });
  assert.strictEqual(env.GDK_BACKEND, "wayland");
});

test("a variable the toolkit took out after the snapshot stays out", () => {
  // GTK unsets DESKTOP_STARTUP_ID at start-up so children do not inherit
  // it; Chromium's Wayland platform takes XDG_ACTIVATION_TOKEN the same way.
  const launch = { ...LAUNCH, SOMETHING_CONSUMED: "x" };
  assert.ok(!("SOMETHING_CONSUMED" in linux(launch, NOW)));
});

test("the app's own launch's startup id and activation token never reach a child", () => {
  // They were the app's, are good once, and a stale one can keep a window
  // from coming forward. Even left in place, they are not passed on.
  const launch = { ...LAUNCH, DESKTOP_STARTUP_ID: "app_TIME1", XDG_ACTIVATION_TOKEN: "t" };
  const env = linux(launch, { ...NOW, DESKTOP_STARTUP_ID: "app_TIME1", XDG_ACTIVATION_TOKEN: "t" });
  assert.ok(!("DESKTOP_STARTUP_ID" in env));
  assert.ok(!("XDG_ACTIVATION_TOKEN" in env));
});

test("a variable named __proto__ is passed on like any other", () => {
  const launch = JSON.parse('{"__proto__":"x","HOME":"/home/me"}');
  const env = linux(launch);
  assert.ok(Object.prototype.hasOwnProperty.call(env, "__proto__"));
  assert.strictEqual(Object.getOwnPropertyDescriptor(env, "__proto__").value, "x");
  assert.strictEqual(Object.getPrototypeOf(env), Object.prototype);
});

test("the app's own variables, exported after the snapshot, are passed on", () => {
  const env = linux(LAUNCH, { ...NOW, ANI_GUI_DEV: "1" });
  assert.strictEqual(env.ANI_GUI_DEV, "1");
});

test("an app variable the app removed after the snapshot is not passed on", () => {
  const env = linux({ ...LAUNCH, ANI_GUI_X: "1" }, NOW);
  assert.ok(!("ANI_GUI_X" in env));
});

test("everything else the user launched with is passed on unchanged", () => {
  const env = linux(LAUNCH, NOW);
  assert.strictEqual(env.HOME, "/home/me");
  assert.strictEqual(env.PATH, "/usr/bin:/bin");
  assert.strictEqual(env.XDG_SESSION_TYPE, "wayland");
});

test("off Linux the child gets the app's environment itself, as before", () => {
  for (const platform of ["win32", "darwin"]) {
    const now = { ...NOW };
    assert.strictEqual(childEnv({ platform, launch: LAUNCH, now }), now, platform);
  }
});

test("building a child's environment changes neither input", () => {
  const launch = { ...LAUNCH };
  const now = { ...NOW, ANI_GUI_DEV: "1" };
  linux(launch, now);
  assert.deepStrictEqual(launch, LAUNCH);
  assert.deepStrictEqual(now, { ...NOW, ANI_GUI_DEV: "1" });
});

test("the snapshot is a frozen copy, not the live environment", () => {
  const live = { A: "1" };
  const snap = launchSnapshot(live);
  live.A = "2";
  live.B = "3";
  assert.deepStrictEqual({ ...snap }, { A: "1" });
  assert.ok(Object.isFrozen(snap));
});

// withUserEnv: the one window where the app's own environment holds the
// user's, for a native call (shell.openExternal, shell.openPath) that
// reads it and takes no environment of its own.

test("the open runs with the user's environment in place", () => {
  const target = { ...NOW };
  let seen;
  withUserEnv({ platform: "linux", target, launch: LAUNCH, open: () => (seen = { ...target }) });
  assert.deepStrictEqual(seen, linux(LAUNCH, NOW));
});

test("the app's environment is the same after an open as before it", () => {
  const target = { ...NOW };
  const result = withUserEnv({ platform: "linux", target, launch: LAUNCH, open: () => "opened" });
  assert.strictEqual(result, "opened");
  assert.deepStrictEqual(target, NOW);
});

test("an open that throws still leaves the app's environment as it was", () => {
  const target = { ...NOW };
  assert.throws(
    () =>
      withUserEnv({
        platform: "linux",
        target,
        launch: LAUNCH,
        open: () => {
          throw new Error("no browser");
        },
      }),
    /no browser/,
  );
  assert.deepStrictEqual(target, NOW);
});

test("an open that rejects later has its environment back before it settles", async () => {
  const target = { ...NOW };
  const pending = withUserEnv({
    platform: "linux",
    target,
    launch: LAUNCH,
    open: () => Promise.reject(new Error("xdg-open failed")),
  });
  assert.deepStrictEqual(target, NOW);
  await assert.rejects(pending, /xdg-open failed/);
  assert.deepStrictEqual(target, NOW);
});

test("off Linux an open never touches the app's environment", () => {
  for (const platform of ["win32", "darwin"]) {
    const target = { ...NOW };
    let seen;
    withUserEnv({ platform, target, launch: LAUNCH, open: () => (seen = { ...target }) });
    assert.deepStrictEqual(seen, NOW, platform);
    assert.deepStrictEqual(target, NOW, platform);
  }
});

const name = fc.oneof(
  fc.constantFrom(
    "XDG_CURRENT_DESKTOP",
    "ORIGINAL_XDG_CURRENT_DESKTOP",
    "CHROME_DESKTOP",
    "GDK_BACKEND",
    "NO_AT_BRIDGE",
    "ANI_GUI_DEV",
    "HOME",
  ),
  fc.stringMatching(/^[A-Z_]{1,8}$/),
);
const envArb = fc.dictionary(name, fc.string({ maxLength: 6 }), { maxKeys: 8, noNullPrototype: true });

test("property: whatever the environments, an open leaves the app's exactly as it was", () => {
  fc.assert(
    fc.property(envArb, envArb, fc.boolean(), (launch, now, throws) => {
      const target = { ...now };
      try {
        withUserEnv({
          platform: "linux",
          target,
          launch,
          open: () => {
            if (throws) throw new Error("x");
          },
        });
      } catch {
        // the property is about the environment, not the open
      }
      assert.deepStrictEqual(target, now);
    }),
  );
});

test("property: a Linux child holds only what the user launched with or the app exported", () => {
  fc.assert(
    fc.property(envArb, envArb, (launch, now) => {
      const env = linux(launch, now);
      assert.ok(!("CHROME_DESKTOP" in env));
      assert.ok(!("ORIGINAL_XDG_CURRENT_DESKTOP" in env));
      for (const [key, value] of Object.entries(env)) {
        if (key.startsWith("ANI_GUI_")) {
          assert.strictEqual(value, now[key], key);
        } else if (key === "XDG_CURRENT_DESKTOP" && "ORIGINAL_XDG_CURRENT_DESKTOP" in launch) {
          assert.strictEqual(value, launch.ORIGINAL_XDG_CURRENT_DESKTOP);
        } else {
          assert.strictEqual(value, launch[key], key);
          assert.ok(key in now, `${key} was taken out after the snapshot`);
        }
      }
    }),
  );
});

// The AppImage. Its runtime (type2-runtime, which the repack puts in
// place of electron-builder's own; both do the same here) exports APPIMAGE, APPDIR, OWD and ARGV0 before AppRun runs,
// and AppRun puts the mount ahead of PATH, XDG_DATA_DIRS, LD_LIBRARY_PATH
// and GSETTINGS_SCHEMA_DIR, all before main.js takes its snapshot. A
// browser started with that environment loads the image's old
// libraries and searches its data first. The environment AppRun builds
// is taken from the AppRun the image ships: electron-builder 26's,
// prepared by the repack (lib/appimage-tree.cjs), its exports run in
// bash.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { prepareTree } = require("./appimage-tree.cjs");

const MOUNT = "/tmp/.mount_ani-guAbC123";
const IMAGE_EXEC = `${MOUNT}/ani-gui.bin`;
const APPRUN_LISTS = ["PATH", "XDG_DATA_DIRS", "LD_LIBRARY_PATH", "GSETTINGS_SCHEMA_DIR"];

/** The export lines of the AppRun the image ships. */
const shippedExports = (() => {
  const fixtures = path.join(__dirname, "fixtures");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-child-env-"));
  try {
    fs.copyFileSync(path.join(fixtures, "AppRun.electron-builder-26"), path.join(dir, "AppRun"));
    fs.copyFileSync(path.join(fixtures, "ani-gui.desktop"), path.join(dir, "ani-gui.desktop"));
    prepareTree(dir);
    const lines = fs
      .readFileSync(path.join(dir, "AppRun"), "utf8")
      .split("\n")
      .filter((l) => l.startsWith("export "));
    assert.strictEqual(lines.length, 4, "AppRun exports the four path lists");
    return lines.join("\n");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
})();

/** What the runtime and AppRun make of the user's environment `user`. */
function launchedFromImage(user) {
  const report =
    `for v in ${APPRUN_LISTS.join(" ")}; do ` +
    `if [ -n "\${!v+x}" ]; then printf '%s=%s\\0' "$v" "\${!v}"; fi; done`;
  const runtime = {
    APPIMAGE: "/home/me/Apps/ani-gui.AppImage",
    APPDIR: MOUNT,
    OWD: "/home/me",
    ARGV0: "ani-gui.AppImage",
  };
  const out = spawnSync("/bin/bash", ["-c", `${shippedExports}\n${report}`], {
    env: { ...user, ...runtime },
    encoding: "utf8",
  });
  assert.strictEqual(out.status, 0, out.stderr);
  const env = { ...user, ...runtime };
  for (const entry of out.stdout.split("\0").filter(Boolean)) {
    const at = entry.indexOf("=");
    env[entry.slice(0, at)] = entry.slice(at + 1);
  }
  return env;
}

const fromImage = (launch, now = launch) =>
  childEnv({ platform: "linux", launch, now, execPath: IMAGE_EXEC, realpath: (p) => p });

test("from the AppImage, a child gets the user's path lists back, without the image's", () => {
  const user = {
    HOME: "/home/me",
    PATH: "/home/me/bin:/usr/bin:/bin",
    XDG_DATA_DIRS: "/usr/share/ubuntu:/usr/local/share/:/usr/share/",
    LD_LIBRARY_PATH: "/opt/mine/lib",
    GSETTINGS_SCHEMA_DIR: "/home/me/schemas",
  };
  const launch = launchedFromImage(user);
  assert.ok(launch.LD_LIBRARY_PATH.startsWith(MOUNT), "the condition: AppRun put the mount first");
  assert.deepStrictEqual(fromImage(launch, { ...launch, GDK_BACKEND: "wayland" }), user);
});

test("from the AppImage, a path list the user never set is not passed on", () => {
  const user = { HOME: "/home/me", PATH: "/usr/bin:/bin" };
  const launch = launchedFromImage(user);
  for (const name of ["XDG_DATA_DIRS", "LD_LIBRARY_PATH", "GSETTINGS_SCHEMA_DIR"]) {
    assert.ok(name in launch, `the condition: AppRun set ${name}`);
  }
  assert.deepStrictEqual(fromImage(launch), user);
});

test("from the AppImage, the runtime's own variables never reach a child", () => {
  // An Electron app started from the app (an editor, through Open
  // folder) would read $APPIMAGE as its own and update the wrong file.
  const env = fromImage(launchedFromImage({ HOME: "/home/me", PATH: "/usr/bin" }));
  for (const name of ["APPIMAGE", "APPDIR", "OWD", "ARGV0"]) assert.ok(!(name in env), name);
});

test("from a portable AppImage, a child gets the user's home, not the image's", () => {
  // With a `<image>.home` or `<image>.config` directory beside it (or
  // --appimage-portable-home / -config), the runtime points HOME and
  // XDG_CONFIG_HOME at them. A browser started with that would start
  // in the portable home with a fresh profile. The user's HOME is the
  // account's (getpwuid); the XDG_CONFIG_HOME the runtime replaced is
  // not known, so the child gets none, and its default ($HOME/.config).
  const launch = launchedFromImage({ HOME: "/home/me", PATH: "/usr/bin" });
  launch.HOME = `${launch.APPIMAGE}.home`;
  launch.XDG_CONFIG_HOME = `${launch.APPIMAGE}.config`;
  const env = childEnv({
    platform: "linux",
    launch,
    now: launch,
    execPath: IMAGE_EXEC,
    realpath: (p) => p,
    accountHome: () => "/home/me",
  });
  assert.deepStrictEqual(env, { HOME: "/home/me", PATH: "/usr/bin" });
});

test("the app's own backend keeps a portable AppImage's home, where the app keeps its data", () => {
  // The portable home and config are the person's choice for the app:
  // the backend keeps its config, cache, history and downloads there.
  // Only programs that are not the app get the account's home.
  const launch = launchedFromImage({ HOME: "/home/me", PATH: "/usr/bin" });
  launch.HOME = `${launch.APPIMAGE}.home`;
  launch.XDG_CONFIG_HOME = `${launch.APPIMAGE}.config`;
  const env = childEnv({
    platform: "linux",
    launch,
    now: launch,
    execPath: IMAGE_EXEC,
    realpath: (p) => p,
    accountHome: () => "/home/me",
    keepPortable: true,
  });
  assert.deepStrictEqual(env, { HOME: launch.HOME, XDG_CONFIG_HOME: launch.XDG_CONFIG_HOME, PATH: "/usr/bin" });
});

test("from a portable AppImage, an account with no home on record keeps the portable one", () => {
  // os.userInfo throws for a uid with no passwd entry (a container's
  // arbitrary uid). A child is better started in the portable home than
  // with none, and a link still opens.
  const launch = launchedFromImage({ HOME: "/home/me", PATH: "/usr/bin" });
  launch.HOME = `${launch.APPIMAGE}.home`;
  launch.XDG_CONFIG_HOME = `${launch.APPIMAGE}.config`;
  const env = childEnv({
    platform: "linux",
    launch,
    now: launch,
    execPath: IMAGE_EXEC,
    realpath: (p) => p,
    accountHome: () => {
      throw new Error("uv_os_get_passwd returned ENOENT");
    },
  });
  assert.deepStrictEqual(env, { HOME: launch.HOME, XDG_CONFIG_HOME: launch.XDG_CONFIG_HOME, PATH: "/usr/bin" });
});

test("a HOME or XDG_CONFIG_HOME the user set elsewhere is theirs, from the AppImage too", () => {
  const launch = launchedFromImage({ HOME: "/srv/me", XDG_CONFIG_HOME: "/srv/me/cfg", PATH: "/usr/bin" });
  const env = childEnv({
    platform: "linux",
    launch,
    now: launch,
    execPath: IMAGE_EXEC,
    realpath: (p) => p,
    accountHome: () => "/home/me",
  });
  assert.deepStrictEqual(env, { HOME: "/srv/me", XDG_CONFIG_HOME: "/srv/me/cfg", PATH: "/usr/bin" });
});

test("an AppImage's variables another program leaked in are passed on as the user launched with them", () => {
  // Another AppImage (an editor's terminal) leaks its own mount; this
  // process does not run from it, so nothing there is the app's to take out.
  const leaked = launchedFromImage({ HOME: "/home/me", PATH: "/usr/bin" });
  const execPath = "/opt/ani-gui/ani-gui.bin";
  const env = childEnv({ platform: "linux", launch: leaked, now: leaked, execPath });
  assert.deepStrictEqual(env, leaked);
});

test("property: from the AppImage, whatever path lists the user set, a child gets exactly them", () => {
  const component = fc.constantFrom(
    "/usr/bin",
    "/bin",
    "/opt/x/lib",
    "/usr/share/",
    "/usr/share/gnome",
    "/usr/local/share/",
    "",
  );
  // A list set but empty is one AppRun's ${VAR:+:...} cannot tell from
  // unset, so it is not a case anyone could rebuild.
  const joined = fc
    .array(component, { minLength: 1, maxLength: 4 })
    .map((c) => c.join(":"))
    .filter((v) => v !== "");
  const list = fc.option(joined, { nil: undefined });
  fc.assert(
    // PATH is always given: bash, which runs AppRun, gives a shell with
    // no PATH a default of its own, and AppRun exports that as the
    // user's. No desktop starts a program without one.
    fc.property(joined, list, list, list, (p, d, l, g) => {
      const user = { HOME: "/home/me" };
      for (const [name, value] of [
        ["PATH", p],
        ["XDG_DATA_DIRS", d],
        ["LD_LIBRARY_PATH", l],
        ["GSETTINGS_SCHEMA_DIR", g],
      ]) {
        if (value !== undefined) user[name] = value;
      }
      assert.deepStrictEqual(fromImage(launchedFromImage(user)), user);
    }),
    { numRuns: 60 },
  );
});

test("the AppRun fixture carries the exports the installed electron-builder writes", () => {
  // The tests above build the launch from the fixture's exports; a
  // builder that writes others makes them describe an image nobody ships.
  const builder = path.dirname(require.resolve("electron-builder/package.json"));
  const lib = path.dirname(require.resolve("app-builder-lib/package.json", { paths: [builder] }));
  const template = fs.readFileSync(path.join(lib, "out", "targets", "appimage", "appImageUtil.js"), "utf8");
  const written = template
    .split("\n")
    .filter((l) => l.startsWith("export "))
    .map((l) => l.replaceAll("\\$", "$"));
  const fixture = fs.readFileSync(path.join(__dirname, "fixtures", "AppRun.electron-builder-26"), "utf8");
  assert.deepStrictEqual(
    fixture.split("\n").filter((l) => l.startsWith("export ")),
    written,
  );
  assert.strictEqual(written.length, 4);
});
