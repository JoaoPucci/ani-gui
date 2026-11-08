/**
 * The show page's watch-order data: the franchise around one entry and
 * the watch history its progress comes from. Neither read is
 * load-bearing for the page: a franchise that cannot be read means no
 * strip (no error, no partial run), and a history that cannot be read
 * means a strip without progress.
 */
import type { HistoryRow } from './progress';
import type { FranchiseData } from './types';

/** Where the two reads come from: `$lib/api` on the page, stubs in tests. */
export interface WatchOrderSources {
	readonly franchise: (id: string) => Promise<FranchiseData>;
	readonly history: () => Promise<readonly HistoryRow[]>;
}

export interface WatchOrderData {
	/** Null when the franchise could not be read. */
	readonly franchise: FranchiseData | null;
	readonly rows: readonly HistoryRow[];
}

/** Both reads at once. Never rejects. */
export async function loadWatchOrder(
	id: string,
	sources: WatchOrderSources
): Promise<WatchOrderData> {
	const [franchise, rows] = await Promise.all([
		sources.franchise(id).catch(() => null),
		sources.history().catch(() => [])
	]);
	return { franchise, rows };
}
