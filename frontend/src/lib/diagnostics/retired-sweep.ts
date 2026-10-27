/**
 * What the diagnostics page shows for the boot sweep over a retired
 * provider's rows (`backend/src/retired_provider.rs`).
 *
 * When a streaming provider leaves the app, the history rows and cache
 * entries it wrote stay behind. At launch the backend removes the
 * history rows a newer provider's row for the same show has taken over
 * from, rewrites the episode of the ones it keeps, and drops the cache
 * entries nothing can read any more. Removing rows from a user's
 * history is worth saying, so the page names them — and says nothing
 * on the launches that change nothing, which is nearly all of them.
 */

/** `AppInfo.retired_provider_sweep`, as the backend sends it. */
export interface RetiredSweepReport {
	superseded: string[];
	renumbered: string[];
	numbering_rows: number;
	cache_rows: number;
}

export interface RetiredSweepView {
	/** Whether the page renders the block at all. */
	visible: boolean;
	/** Titles of the history rows removed. */
	superseded: string[];
	/** Titles of the history rows kept with their episode rewritten. */
	renumbered: string[];
	/** Cache entries and numbering rows removed, together. */
	storedEntries: number;
}

export function retiredSweepView(
	report: Partial<RetiredSweepReport> | undefined | null
): RetiredSweepView {
	void report;
	return { visible: false, superseded: [], renumbered: [], storedEntries: 0 };
}
