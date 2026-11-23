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
