"use strict";

/**
 * Whether Electron should resolve its data under the dev profile
 * (`ani-gui-dev`) rather than the installed app's (`ani-gui`).
 *
 * True when *either*:
 *  - `ELECTRON_DEV=1` — the dev launcher, which also exports
 *    `ANI_GUI_DEV=1` for the backend; or
 *  - `ANI_GUI_DEV` is set to a non-empty value — the documented way to
 *    point a packaged/release build at throwaway data (e.g. testing
 *    migrations). In that case `ELECTRON_DEV` is unset, but the Rust
 *    backend still switches to `ani-gui-dev` off `ANI_GUI_DEV`, so
 *    Electron must follow or its config (locale) + userData (OAuth
 *    tokens) would straddle the installed and dev profiles.
 *
 * Mirrors the backend's `ANI_GUI_DEV` check (`is_some_and(|v| !v.is_empty())`):
 * an empty string counts as unset.
 *
 * @param {{ELECTRON_DEV?: string, ANI_GUI_DEV?: string}} [env]
 * @returns {boolean}
 */
function isDevProfile(env = {}) {
  return env.ELECTRON_DEV === "1" || Boolean(env.ANI_GUI_DEV);
}

module.exports = { isDevProfile };
