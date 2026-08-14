/**
 * What the diagnostics page shows for the backend's boot sweep.
 *
 * Versions before 0.12 kept their own copy of the shell script under
 * the cache root and refreshed it on launch. Resolution is native now,
 * so that copy is dead weight the app deletes at boot — and deleting a
 * user's file without saying so is the part worth avoiding, hence a
 * block on this page rather than a silent `remove_file`.
 *
 * The interesting judgement is when *not* to show it. Almost every
 * launch has nothing to report, and a section that is permanently
 * empty trains people to ignore the page.
 */

export interface LegacySweepView {
	/** Whether the page renders the block at all. */
	visible: boolean;
	/** Paths to list under it. Empty whenever `visible` is false. */
	paths: string[];
}

/**
 * Decide the block's state from `AppInfo.removed_legacy_paths`.
 *
 * `undefined` covers a backend that predates the field. Packaged
 * builds ship both halves together so it shouldn't happen there, but
 * a development run against a stale debug binary hits it routinely,
 * and iterating `undefined` would take the whole page down.
 */
export function legacySweepView(removed: string[] | undefined | null): LegacySweepView {
	const paths = removed ?? [];
	return { visible: paths.length > 0, paths };
}
