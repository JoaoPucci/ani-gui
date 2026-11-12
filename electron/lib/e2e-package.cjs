"use strict";

// The end-to-end suite's package: the release's Linux package in every
// respect but two, which keep a test-key build apart from a release.
// Inside, the tree is the release's (name, id, version); what tells it
// apart is where it comes from and where it lands.
//
// - Its backend is the one `pnpm run build:backend:e2e` builds, with
//   the `test-run-key` cargo feature, into backend/target-e2e: a test
//   run's flags file signed by the fixture test-run key may then turn
//   a flag on, which is how the suite runs every flag in each state.
//   No release configuration reads that directory.
// - It is written to dist-e2e as an unpacked tree only: no AppImage,
//   no .deb, nothing installable, beside nothing a release writes.
//
// electron-builder.e2e.cjs hands this to electron-builder in place of
// package.json's `build` (`pnpm run package:e2e`, with `--linux dir`;
// a target given on the command line would override `dir`). Only that
// script names the configuration or the backend build, and no release
// script runs it (tests/arch/test_run_key.sh). The e2e workflow builds
// the release-shaped package in a job of its own and checks that its
// backend holds no test-run key.

const TRIPLE = "x86_64-unknown-linux-musl";

/** Where the release's Linux package takes its backend from. */
const RELEASE_BACKEND = `../backend/target/${TRIPLE}/release/ani-gui-backend`;

/** Where the e2e package takes its backend from. */
const E2E_BACKEND = `../backend/target-e2e/${TRIPLE}/release/ani-gui-backend`;

/** The e2e package's output directory. */
const E2E_OUTPUT = "dist-e2e";

/** The e2e package's configuration, from the release's `build`. Throws
 *  when the release's Linux backend is not one resource read from
 *  where this expects, so the e2e package cannot quietly carry the
 *  release backend or two. */
function e2eConfig(build) {
  const resources = build.linux?.extraResources ?? [];
  const backend = resources.filter((r) => r.to === "ani-gui-backend");
  if (backend.length !== 1 || backend[0].from !== RELEASE_BACKEND) {
    throw new Error(
      `the release's Linux backend is not one resource from ${RELEASE_BACKEND}: ${JSON.stringify(backend)}`,
    );
  }
  return {
    ...build,
    directories: { ...build.directories, output: E2E_OUTPUT },
    linux: {
      ...build.linux,
      target: ["dir"],
      extraResources: resources.map((r) =>
        r.to === "ani-gui-backend" ? { ...r, from: E2E_BACKEND } : r,
      ),
    },
  };
}

module.exports = { E2E_BACKEND, E2E_OUTPUT, RELEASE_BACKEND, e2eConfig };
