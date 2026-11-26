// The environment an e2e launch gives the packaged app. A test run's
// flags file is named by the launch's own option and nothing else: a
// value inherited from the shell that runs the suite would switch the
// app's flags for a spec that never asked, so every launch drops it,
// the Playwright launch and the spawned one alike (e2e/launch.ts).

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");

const { appEnv } = require("./e2e-launch-env.cjs");

const ORIGIN = "http://127.0.0.1:9";

test("a flags file inherited from the shell does not reach the app", () => {
  const env = appEnv({ PATH: "/bin", ANI_GUI_TEST_FLAGS_FILE: "/tmp/shell.json" }, { proxyOrigin: ORIGIN });
  assert.equal("ANI_GUI_TEST_FLAGS_FILE" in env, false);
  assert.equal(env.PATH, "/bin");
});

test("the launch's own flags file is the one the app reads", () => {
  const env = appEnv(
    { ANI_GUI_TEST_FLAGS_FILE: "/tmp/shell.json" },
    { proxyOrigin: ORIGIN, flagsFile: "/tmp/spec.json" }
  );
  assert.equal(env.ANI_GUI_TEST_FLAGS_FILE, "/tmp/spec.json");
});

test("every proxy variable names the refusing proxy, and loopback goes direct", () => {
  const env = appEnv({ http_proxy: "http://elsewhere" }, { proxyOrigin: ORIGIN });
  for (const name of ["http_proxy", "https_proxy", "all_proxy"]) {
    assert.equal(env[name], ORIGIN);
    assert.equal(env[name.toUpperCase()], ORIGIN);
  }
  assert.equal(env.no_proxy, "127.0.0.1,localhost,::1");
  assert.equal(env.NO_PROXY, "127.0.0.1,localhost,::1");
});

test("a test run's environment is set over the inherited one", () => {
  const env = appEnv({ ANI_GUI_UNATTENDED: "0" }, { proxyOrigin: ORIGIN });
  assert.equal(env.ANI_GUI_UNATTENDED, "1");
});

test("whatever the shell holds, only the launch's option names a flags file", () => {
  fc.assert(
    fc.property(
      fc.dictionary(fc.string({ minLength: 1, maxLength: 12 }), fc.string({ maxLength: 12 })),
      fc.option(fc.string({ maxLength: 12 }), { nil: undefined }),
      fc.option(fc.string({ maxLength: 12 }), { nil: undefined }),
      (shell, inherited, flagsFile) => {
        const env = { ...shell };
        if (inherited !== undefined) env.ANI_GUI_TEST_FLAGS_FILE = inherited;
        const out = appEnv(env, { proxyOrigin: ORIGIN, flagsFile });
        assert.equal(out.ANI_GUI_TEST_FLAGS_FILE, flagsFile);
      }
    )
  );
});

test("an onboarding release inherited from the shell does not reach the app", () => {
  const env = appEnv({ ANI_GUI_TEST_ONBOARDING_VERSION: "0.19.0" }, { proxyOrigin: ORIGIN });
  assert.equal("ANI_GUI_TEST_ONBOARDING_VERSION" in env, false);
});

test("the launch's own onboarding release is the one the app counts tips at", () => {
  const env = appEnv(
    { ANI_GUI_TEST_ONBOARDING_VERSION: "0.30.0" },
    { proxyOrigin: ORIGIN, onboardingVersion: "0.19.0" }
  );
  assert.equal(env.ANI_GUI_TEST_ONBOARDING_VERSION, "0.19.0");
});
