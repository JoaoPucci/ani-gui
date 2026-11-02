/**
 * The launch's remote flags, as the renderer sees them.
 *
 * The backend decides them within two seconds of starting and never
 * changes them in the session (backend/src/flags). The Electron main
 * process asks for them before it opens the window and hands them to
 * the preload, so they are on `window.aniGui.flags` before the first
 * page renders — this module only reads them. A window opened without
 * them, and a browser-only dev run, has none: every flag then takes
 * its default, which the feature reading it owns.
 */

/** Every flag the window was opened with, by name. */
export type LaunchFlags = Readonly<Record<string, boolean | string>>;

type FlagsWindow = { aniGui?: { flags?: LaunchFlags } } | undefined;

/** The window's launch flags; an empty set when it has none. */
export function launchFlags(win: FlagsWindow = globalThis.window): LaunchFlags {
	void win;
	return {};
}
