"use strict";

const test = require("node:test");
const assert = require("node:assert");

const { devEnv, devFlags, depsFetchScript } = require("./dev-launch.cjs");

test("linux gets the Wayland-aware environment, defaulted to auto", () => {
  // `auto` is session-aware: native Wayland on a Wayland session,
  // X11 otherwise. It must be in the environment before Electron's
  // early Ozone init — the packaged build gets it from the afterPack
  // launcher, dev gets it here.
  assert.deepStrictEqual(devEnv("linux", {}), {
    ELECTRON_DEV: "1",
    ELECTRON_OZONE_PLATFORM_HINT: "auto",
  });
});

test("linux honors a hint the user already exported", () => {
  assert.deepStrictEqual(devEnv("linux", { ELECTRON_OZONE_PLATFORM_HINT: "x11" }), {
    ELECTRON_DEV: "1",
    ELECTRON_OZONE_PLATFORM_HINT: "x11",
  });
});

test("an empty exported hint falls back to auto", () => {
  assert.deepStrictEqual(devEnv("linux", { ELECTRON_OZONE_PLATFORM_HINT: "" }), {
    ELECTRON_DEV: "1",
    ELECTRON_OZONE_PLATFORM_HINT: "auto",
  });
});

test("non-linux platforms get only the dev-profile flag", () => {
  // The Ozone hint is a Linux display-server concern; exporting it
  // elsewhere is dead weight at best.
  for (const platform of ["win32", "darwin"]) {
    assert.deepStrictEqual(devEnv(platform, {}), { ELECTRON_DEV: "1" });
  }
});

test("linux keeps its launch flags exactly as the shell script had them", () => {
  assert.deepStrictEqual(devFlags("linux"), ["--no-sandbox", "--enable-wayland-ime"]);
});

test("non-linux platforms launch without extra flags", () => {
  for (const platform of ["win32", "darwin"]) {
    assert.deepStrictEqual(devFlags(platform), []);
  }
});

test("windows stages the bundled deps before launching", () => {
  // Without staging, `resolve_bundled_bin` finds no `<exe_dir>/bin`,
  // every spawn falls through to PATH, and Windows PATH has no
  // impersonating transport — the provider rejects playback.
  assert.strictEqual(depsFetchScript("win32"), "fetch-windows-deps.mjs");
});

test("linux and mac dev loops stage nothing", () => {
  // Unchanged from the shell-script era: a Linux dev's transport
  // comes from PATH or a manual fetch:linux-deps run.
  assert.strictEqual(depsFetchScript("linux"), null);
  assert.strictEqual(depsFetchScript("darwin"), null);
});

test("the dev package script is shell-neutral and runs the launcher", () => {
  // pnpm executes package scripts through cmd.exe on Windows, which
  // reads a POSIX env-prefix (`VAR=x cmd`) as a command name.
  const dev = require("../package.json").scripts.dev;
  assert.doesNotMatch(dev, /^\w+=/);
  assert.match(dev, /dev\.mjs/);
});
