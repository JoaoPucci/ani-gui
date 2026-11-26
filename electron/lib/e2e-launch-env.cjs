// The environment an e2e launch gives the packaged app
// (e2e/launch.ts, both launches): what the suite's shell holds, every
// proxy variable pointed at the refusing proxy so no request leaves
// the machine, and a test run's own variables over all of it.
//
// A test run's flags file is named by the launch's `flagsFile` option
// and by nothing else. A value inherited from the shell that runs the
// suite is dropped: it would switch the flags of a spec that never
// asked for any.
//
// Test infrastructure only; nothing in the app requires it.

const FLAGS_FILE = "ANI_GUI_TEST_FLAGS_FILE";

// The release a test-run backend counts the feature tips' windows at
// (backend/src/onboarding/mod.rs), named by the launch's
// `onboardingVersion` alone, for the same reason as the flags file.
const ONBOARDING_VERSION = "ANI_GUI_TEST_ONBOARDING_VERSION";

/** What every test launch sets, whatever the spec passes: a test run
 *  (`ANI_GUI_UNATTENDED=1`). A boot that fails then exits instead of
 *  waiting on a dialog nobody reads (lib/boot-failure.cjs), and the
 *  backend takes its flags from the test (`ANI_GUI_TEST_FLAGS_FILE`, or
 *  the defaults) and never asks GitHub for the live file, so a flipped
 *  flag or a slow CDN cannot change a test's result
 *  (backend/src/flags/launch.rs). */
const TEST_RUN_ENV = Object.freeze({ ANI_GUI_UNATTENDED: "1" });

/** The proxy settings every test launch carries, pointing at `origin`:
 *  the backend's HTTP clients (reqwest) and the provider transport
 *  (curl) both read them, in either case. Loopback is exempt, so the
 *  backend still reaches a spec's own local servers (a media fixture,
 *  an upstream that never answers) and the renderer its backend. */
function proxyEnv(origin) {
  const env = {};
  for (const name of ["http_proxy", "https_proxy", "all_proxy"]) {
    env[name] = origin;
    env[name.toUpperCase()] = origin;
  }
  const loopback = "127.0.0.1,localhost,::1";
  env.no_proxy = loopback;
  env.NO_PROXY = loopback;
  return env;
}

/** The app's environment: `env` without a flags file or an onboarding
 *  release, the proxy at `proxyOrigin`, a test run's variables, and
 *  `flagsFile` and `onboardingVersion` when they are given. */
function appEnv(env, { proxyOrigin, flagsFile, onboardingVersion } = {}) {
  const { [FLAGS_FILE]: _dropped, [ONBOARDING_VERSION]: _droppedToo, ...rest } = env ?? {};
  return {
    ...rest,
    ...(flagsFile === undefined ? {} : { [FLAGS_FILE]: flagsFile }),
    ...(onboardingVersion === undefined ? {} : { [ONBOARDING_VERSION]: onboardingVersion }),
    ...proxyEnv(proxyOrigin),
    ...TEST_RUN_ENV,
  };
}

module.exports = { appEnv, TEST_RUN_ENV };
