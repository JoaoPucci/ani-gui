"use strict";

/**
 * What the dev launcher runs, computed per platform so the package
 * script can stay shell-neutral. pnpm executes package scripts
 * through cmd.exe on Windows regardless of the invoking terminal,
 * and cmd.exe reads a POSIX env-prefix (`VAR=x cmd`) as a command
 * name — so the environment is built here instead of in the script.
 */

/**
 * Environment additions for a dev launch.
 *
 * `ELECTRON_DEV=1` selects the dev profile (see `dev-profile.cjs`)
 * and points the window at the Vite server. On Linux the Ozone hint
 * must be in the environment before Electron's early Ozone init —
 * the packaged build gets it from the afterPack launcher wrapper,
 * dev gets it here. `auto` is session-aware (native Wayland on a
 * Wayland session, X11 otherwise); a hint the user exported wins,
 * an empty one counts as unset.
 *
 * @param {NodeJS.Platform} platform
 * @param {{ELECTRON_OZONE_PLATFORM_HINT?: string}} env
 * @returns {Record<string, string>}
 */
function devEnv(platform, env = {}) {
  const out = { ELECTRON_DEV: "1" };
  if (platform === "linux") {
    out.ELECTRON_OZONE_PLATFORM_HINT = env.ELECTRON_OZONE_PLATFORM_HINT || "auto";
  }
  return out;
}

/**
 * Electron flags for a dev launch. `--enable-wayland-ime` keeps IME
 * composition working under native Wayland; Chromium does not infer
 * it from the Ozone hint. Both flags are Linux display-server
 * concerns and stay off the other platforms.
 *
 * @param {NodeJS.Platform} platform
 * @returns {string[]}
 */
function devFlags(platform) {
  return platform === "linux" ? ["--no-sandbox", "--enable-wayland-ime"] : [];
}

/**
 * The deps fetcher a dev launch runs first, or null. Windows stages
 * the bundled deps into `backend/target/<profile>/bin` so
 * `resolve_bundled_bin` finds them — its PATH has no impersonating
 * transport, and without one the provider rejects playback. The
 * Linux dev loop is unchanged: a usable transport is typically on
 * PATH, and `fetch:linux-deps` remains a manual step.
 *
 * @param {NodeJS.Platform} platform
 * @returns {string | null}
 */
function depsFetchScript(platform) {
  return platform === "win32" ? "fetch-windows-deps.mjs" : null;
}

module.exports = { devEnv, devFlags, depsFetchScript };
