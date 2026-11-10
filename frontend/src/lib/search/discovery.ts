/**
 * The discovery rows the search page shows while nothing has been
 * asked: what each row's request has come to, and what the page draws
 * for it.
 *
 * The rows are fetched side by side and each lands when it lands, so
 * the order they answer in is not the order they are drawn in.
 */

/** Where one row's request stands: still out, answered (after the
 *  availability filter has had its say), or failed. */
export type RowLoad<T> =
	| { kind: 'loading' }
	| { kind: 'ready'; items: readonly T[] }
	| { kind: 'failed' };

/** What the page draws in one row's place: a skeleton strip, the row,
 *  or nothing at all. */
export type RowSlot<T> =
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
 * already final. */
export function discoveryView<T>(rows: readonly RowLoad<T>[]): DiscoveryView<T> {
	let aboveSettled = true;
	const slots = rows.map((row): RowSlot<T> => {
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
