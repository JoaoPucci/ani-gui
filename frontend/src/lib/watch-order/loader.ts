/**
 * The pages' watch-order data: the franchise around one entry and the
 * history rows its progress comes from. Neither read is load-bearing
 * for the page: a franchise that cannot be read means no strip (no
 * error, no partial run), and rows that cannot be read mean a strip
 * without progress.
 *
 * The rows are the ones the franchise's entries resume from, as the
 * backend's resume lookup names them (`history_claim::names`): a row
 * by the id it recorded while Kitsu still has that entry, else by its
 * stored mapping, else, for a row whose recorded entry Kitsu deleted,
 * by the title match Home stored for it. So the strip counts a row
 * where the entry's own page would resume from it, toward the entry
 * that named it. One row can be named for several entries: a row whose
 * recorded entry is gone, by a title match stored for each cour.
 *
 * Both reads belong to one visit of one page, and `signal` ends them
 * when it ends. The franchise read holds its response open while the
 * backend walks the franchise, several rounds of Kitsu reads when
 * Kitsu is slow, on the API lane the page's own details, settings and
 * history share (`$lib/backend/lanes`). Kept after the viewer left, a
 * few such reads would hold all of that lane's connections and queue
 * the next page behind them. A visit that ended answers nothing, as a
 * franchise that cannot be read does: not a strip without progress,
 * whose rows were cut off rather than unreadable.
 */
import type { HistoryRow } from './progress';
import type { FranchiseData } from './types';

/** Where the reads come from: `./sources` on the pages, stubs in tests. */
export interface WatchOrderSources {
	readonly franchise: (id: string, signal?: AbortSignal) => Promise<FranchiseData>;
	/** Each of `ids` with the history row it resumes from, or null. */
	readonly named: (
		ids: readonly string[],
		signal?: AbortSignal
	) => Promise<Record<string, HistoryRow | null>>;
}

export interface WatchOrderData {
	/** Null when the franchise could not be read. */
	readonly franchise: FranchiseData | null;
	readonly rows: readonly HistoryRow[];
}

/** Every id the franchise names. */
function idsOf(f: FranchiseData): string[] {
	const all = [...f.entries, ...f.versions, ...f.films, ...f.sides, ...f.recaps];
	return [...new Set(all.map((e) => e.id))];
}

/** What a visit that ended, or a franchise that cannot be read, answers. */
const NOTHING: WatchOrderData = { franchise: null, rows: [] };

/** The franchise, then its entries' rows, until `signal` ends the visit. Never rejects. */
export async function loadWatchOrder(
	id: string,
	sources: WatchOrderSources,
	signal?: AbortSignal
): Promise<WatchOrderData> {
	if (signal?.aborted) return NOTHING;
	const franchise = await sources.franchise(id, signal).catch(() => null);
	if (!franchise || signal?.aborted) return NOTHING;
	const named = await sources.named(idsOf(franchise), signal).catch(() => ({}));
	if (signal?.aborted) return NOTHING;
	const rows = Object.entries(named).flatMap(([kitsu_id, row]) =>
		row ? [{ kitsu_id, ep_no: row.ep_no }] : []
	);
	return { franchise, rows };
}
