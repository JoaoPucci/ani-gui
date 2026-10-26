import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	heldPending,
	listingKey,
	NO_SHOW_PENDINGS,
	pendingUnder,
	settlePending,
	type Listing,
	type ShowPendings
} from './editor-pendings';
import type { PendingEdit } from './list-entry-pending';

const ids = fc.constantFrom('A', 'B');
const listing: fc.Arbitrary<Listing | undefined> = fc.option(
	fc.dictionary(
		fc.constantFrom('anilist', 'mal'),
		fc.record({ user: fc.constantFrom('u1', 'u2') }),
		{
			maxKeys: 2
		}
	),
	{ nil: undefined }
);
const pending: fc.Arbitrary<PendingEdit | null> = fc.option(
	fc.record({
		kitsuId: ids,
		intendedStatus: fc.constantFrom('paused' as const, 'dropped' as const),
		intendedProgress: fc.nat(24)
	}),
	{ nil: null }
);
/** Writes settling, each under a listing, each leaving a retry or none. */
const run = fc.array(fc.tuple(ids, listing, pending), { maxLength: 10 });

function replay(steps: [string, Listing | undefined, PendingEdit | null][]): ShowPendings {
	return steps.reduce(
		(ps, [id, listed, p]) => settlePending(ps, id, listed, () => p),
		NO_SHOW_PENDINGS
	);
}

/** A listing as sorted `[tracker, user]` pairs. */
const flat = (l: Listing) =>
	Object.keys(l)
		.sort()
		.map((k) => [k, l[k].user]);

describe('listingKey, by property', () => {
	it('names a listing by its trackers and users, whatever their order', () => {
		fc.assert(
			fc.property(listing, (l) => {
				if (!l) return;
				const reversed = Object.fromEntries(Object.entries(l).reverse());
				expect(listingKey(reversed)).toBe(listingKey(l));
			})
		);
	});

	it('tells listings apart exactly when their trackers or users differ', () => {
		fc.assert(
			fc.property(listing, listing, (a, b) => {
				const same =
					a === undefined || b === undefined
						? a === b
						: JSON.stringify(flat(a)) === JSON.stringify(flat(b));
				expect(listingKey(a) === listingKey(b)).toBe(same);
			})
		);
	});
});

describe('settlePending, by property', () => {
	it("sets the settled listing's retry and leaves every other listing's as it was", () => {
		fc.assert(
			fc.property(
				run,
				ids,
				listing,
				pending,
				listing,
				ids,
				(steps, id, listed, p, other, otherId) => {
					const before = replay(steps);
					const after = settlePending(before, id, listed, () => p);
					expect(pendingUnder(after, id, listed)).toEqual(p);
					if (otherId !== id || listingKey(other) !== listingKey(listed)) {
						expect(pendingUnder(after, otherId, other)).toEqual(
							pendingUnder(before, otherId, other)
						);
					}
				}
			)
		);
	});

	it('hands the settled listing its own retry, and nothing else', () => {
		fc.assert(
			fc.property(run, ids, listing, (steps, id, listed) => {
				const before = replay(steps);
				let seen: PendingEdit | null | undefined;
				settlePending(before, id, listed, (prev) => {
					seen = prev;
					return prev;
				});
				expect(seen).toEqual(pendingUnder(before, id, listed));
			})
		);
	});

	it('never changes the map it is given', () => {
		fc.assert(
			fc.property(run, ids, listing, pending, (steps, id, listed, p) => {
				const before = replay(steps);
				const copy = new Map([...before].map(([k, v]) => [k, [...v]]));
				settlePending(before, id, listed, () => p);
				expect(new Map([...before].map(([k, v]) => [k, [...v]]))).toEqual(copy);
			})
		);
	});
});

describe('heldPending, by property', () => {
	it('is the newest retry whose listing holds', () => {
		fc.assert(
			fc.property(run, ids, fc.constantFrom('u1', 'u2'), (steps, id, holder) => {
				const ps = replay(steps);
				const holds = (l?: Listing) => !l || Object.values(l).some((a) => a.user === holder);
				// The model: replay the run, then take the last retry left for the
				// show whose listing holds.
				const live = new Map<string, { listed?: Listing; pending: PendingEdit }>();
				for (const [sid, l, p] of steps) {
					if (sid !== id) continue;
					const key = listingKey(l);
					live.delete(key);
					if (p) live.set(key, { listed: l, pending: p });
				}
				const newest = [...live.values()].reverse().find((e) => holds(e.listed));
				expect(heldPending(ps, id, holds)).toEqual(newest?.pending ?? null);
			})
		);
	});
});
