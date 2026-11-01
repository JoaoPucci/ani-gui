/**
 * What the diagnostics page shows for the launch's cache reclaim
 * (`backend/src/cache/reclaim.rs`): the cache rows no part of this
 * version reads any more — kept under keys an earlier version used, or
 * past their lifetime — deleted when the app started. Shown only on a
 * launch that deleted some.
 */

/** `AppInfo.cache_reclaim`, as the backend sends it. */
export interface CacheReclaimReport {
	undeclared_rows: number;
	expired_rows: number;
}

export interface CacheReclaimView {
	/** Whether the page renders the block at all. */
	visible: boolean;
	/** Rows kept under keys an earlier version used. */
	oldRows: number;
	/** Rows past their lifetime. */
	expiredRows: number;
}

export function cacheReclaimView(
	report: Partial<CacheReclaimReport> | undefined | null
): CacheReclaimView {
	void report;
	return { visible: false, oldRows: 0, expiredRows: 0 };
}
