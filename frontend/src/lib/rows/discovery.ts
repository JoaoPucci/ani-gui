/**
 * Rows of cards stacked down a page, each loaded on its own: what each
 * row's request has come to, and what the page draws for it. The search
 * page's discovery rows, shown while nothing has been asked, are one
 * such stack (loadDiscoveryRows loads them); the home page's rows are
 * another ($lib/home/rows).
 *
 * The rows are fetched side by side and each lands when it lands, so
 * the order they answer in is not the order they are drawn in.
 */

/** Where one row's request stands: not yet known to exist, still out,
 *  answered (after the availability filter has had its say), or
 *  failed. */
export type RowLoad<T> =
	| { kind: 'pending' }
	| { kind: 'loading' }
	| { kind: 'ready'; items: readonly T[] }
	| { kind: 'failed' };

/** What the page draws in one row's place: nothing yet, a skeleton
 *  strip, the row, or nothing at all. */
export type RowSlot<T> =
	| { kind: 'held' }
	| { kind: 'skeleton' }
	| { kind: 'shown'; items: readonly T[] }
	| { kind: 'absent' };

export interface DiscoveryView<T> {
	/** One slot per row, in the order the rows are drawn. */
	slots: RowSlot<T>[];
	/** Every row has settled and none has anything to show: the page
	 *  says so instead of leaving the space blank. */
	nothing: boolean;
}

export const LOADING: RowLoad<never> = { kind: 'loading' };
export const PENDING: RowLoad<never> = { kind: 'pending' };

/** A row's request, as a load that never rejects. */
export function settleRow<T>(request: Promise<readonly T[]>): Promise<RowLoad<T>> {
	return request.then(
		(items): RowLoad<T> => ({ kind: 'ready', items }),
		(): RowLoad<T> => ({ kind: 'failed' })
	);
}

/** What the page draws for `rows`, given in the order they are drawn.
 *
 * A row is shown only once every row above it has settled, so nothing
 * that is already on screen is ever pushed down by a row landing above
 * it: until then it keeps a skeleton in its place, the same as a row
 * still loading. A row that failed, or that the availability filter
 * emptied, is dropped. Dropping one can move only skeletons, never a
 * shown row, because a row is shown only when everything above it is
 * already final.
 *
 * A row not yet known to exist has no skeleton to hold its place, so
 * nothing in it or below it is drawn until it is known: drawn earlier,
 * a row below would be pushed down when it turned out to be there. */
export function discoveryView<T>(rows: readonly RowLoad<T>[]): DiscoveryView<T> {
	let aboveSettled = true;
	let abovePending = false;
	const slots = rows.map((row): RowSlot<T> => {
		if (abovePending || row.kind === 'pending') {
			abovePending = true;
			aboveSettled = false;
			return { kind: 'held' };
		}
		if (row.kind === 'loading') {
			aboveSettled = false;
			return { kind: 'skeleton' };
		}
		if (row.kind === 'failed' || row.items.length === 0) return { kind: 'absent' };
		return aboveSettled ? { kind: 'shown', items: row.items } : { kind: 'skeleton' };
	});
	const nothing = aboveSettled && slots.every((slot) => slot.kind === 'absent');
	return { slots, nothing };
}

/** What {@link loadDiscoveryRows} needs from the page. */
export interface DiscoveryLoad<T, C> {
	/** The settings the page already holds, or null when it has none. */
	config: () => C | null;
	/** Read the settings from the backend. */
	readConfig: () => Promise<C>;
	/** Keep settings that were read, for this load and the next. */
	keepConfig: (config: C) => void;
	/** The sub/dub mode to filter for, given the settings or null. */
	pickMode: (config: C | null) => 'sub' | 'dub';
	/** One request per row, in drawing order, filtered for `mode`. */
	rows: readonly ((mode: 'sub' | 'dub') => Promise<readonly T[]>)[];
	/** Called once per row as it settles, with the row's index. */
	onRow: (index: number, row: RowLoad<T>) => void;
}

/**
 * Ask for every discovery row. Also the notice's retry, which is
 * offered only once every row has settled, so no earlier answer is
 * still on its way.
 *
 * The settings are resolved first so the availability filter sees the
 * user's actual sub/dub mode; started in parallel, a row could be
 * filtered with the no-settings default. When they could not be read,
 * they are asked again on the retry: they come from the same local
 * backend as the rows, so a page that reached the notice has usually
 * lost them too. A request that fails settles as a failed row, and
 * the promise resolves once every row has settled.
 */
export async function loadDiscoveryRows<T, C>(load: DiscoveryLoad<T, C>): Promise<void> {
	let config = load.config();
	if (config === null) {
		try {
			config = await load.readConfig();
			load.keepConfig(config);
		} catch {
			// Still unreadable: pickMode falls back to its default.
		}
	}
	const mode = load.pickMode(config);
	await Promise.all(
		load.rows.map((request, index) =>
			settleRow(request(mode)).then((row) => load.onRow(index, row))
		)
	);
}
