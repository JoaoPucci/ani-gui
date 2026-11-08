/**
 * The launch's remote flags, as the renderer sees them.
 *
 * The backend decides them within two seconds of starting its refresh
 * and never changes them in the session (backend/src/flags). The Electron main
 * process asks for them before it opens the window and hands them to
 * the preload, so they are on `window.aniGui.flags` before the first
 * page renders — this module only reads them.
 *
 * Flags are default OFF, and any read failure means OFF. A feature
 * reads its flag through `flagOn`, which is on only for exactly `true`:
 * a window opened without flags, a browser-only dev run, a backend
 * that never answered and a flag the set does not hold all read off.
 */

/** Every flag the window was opened with, by name. */
export type LaunchFlags = Readonly<Record<string, boolean>>;

type FlagsWindow = { aniGui?: { flags?: LaunchFlags } } | undefined;

/** The window's launch flags; an empty set when it has none. */
const NONE: LaunchFlags = Object.freeze({});

export function launchFlags(win: FlagsWindow = globalThis.window): LaunchFlags {
	return win?.aniGui?.flags ?? NONE;
}

/** Whether flag `name` is on for this launch: only when the window was
 *  opened with exactly `true` for it. Anything else, a missing flag or
 *  a missing set included, is off. */
export function flagOn(name: string, win: FlagsWindow = globalThis.window): boolean {
	return launchFlags(win)[name] === true;
}
