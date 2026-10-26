import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	heldPending,
	listingKey,
	listingsCompatible,
	NO_SHOW_PENDINGS,
	settlePending,
	type Listing,
	type ShowPendings
} from './editor-pendings';
import type { PendingEdit } from './list-entry-pending';

const ids = fc.constantFrom('A', 'B');
const listing: fc.Arbitrary<Listing | undefined> = fc.option(
	fc.dictionary(
		fc.constantFrom('anilist', 'mal', 'inhouse'),
		fc.record({ user: fc.constantFrom<string | null>('u1', 'u2', null) }),
		{ maxKeys: 3 }
	),
	{ nil: undefined }
);

/** The rule, written out apart from the code: the same listing, or no
 *  shared tracker under another user and one shared under the same. */
function compatible(a?: Listing, b?: Listing): boolean {
	if (a === undefined || b === undefined) return a === b;
	if (listingKey(a) === listingKey(b)) return true;
	const shared = Object.keys(a).filter((p) => p in b);
	const same = (p: string) => a[p].user !== null && a[p].user === b[p].user;
	return shared.every((p) => a[p].user === b[p].user) && shared.some(same);
}
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

describe('listingsCompatible, by property', () => {
	it('is the rule: the same listing, or no tracker switched and one still shared', () => {
		fc.assert(
			fc.property(listing, listing, (a, b) => {
				expect(listingsCompatible(a, b)).toBe(compatible(a, b));
			})
		);
	});

	it('is symmetric, and takes in every listing as itself', () => {
		fc.assert(
			fc.property(listing, listing, (a, b) => {
				expect(listingsCompatible(a, b)).toBe(listingsCompatible(b, a));
				expect(listingsCompatible(a, a)).toBe(true);
			})
		);
	});
});

describe('settlePending, by property', () => {
	it('replaces every retry the settling listing is compatible with, and leaves every other one', () => {
		fc.assert(
			fc.property(run, ids, listing, pending, (steps, id, listed, p) => {
				const before = replay(steps);
				const after = settlePending(before, id, listed, () => p);
				const kept = (before.get(id) ?? []).filter((e) => !compatible(listed, e.listed));
				const want = p === null ? kept : [...kept, { pending: p, listed }];
				expect(after.get(id) ?? []).toEqual(want);
				for (const other of ['A', 'B'].filter((o) => o !== id)) {
					expect(after.get(other)).toEqual(before.get(other));
				}
			})
		);
	});

	it('hands the settlement the newest retry it is compatible with', () => {
		fc.assert(
			fc.property(run, ids, listing, (steps, id, listed) => {
				const before = replay(steps);
				let seen: PendingEdit | null | undefined;
				settlePending(before, id, listed, (prev) => {
					seen = prev;
					return prev;
				});
				const newest = [...(before.get(id) ?? [])]
					.reverse()
					.find((e) => compatible(listed, e.listed));
				expect(seen).toEqual(newest?.pending ?? null);
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
				const live: { listed?: Listing; pending: PendingEdit }[] = [];
				for (const [sid, l, p] of steps) {
					if (sid !== id) continue;
					const kept = live.filter((e) => !compatible(l, e.listed));
					live.length = 0;
					live.push(...kept);
					if (p) live.push({ listed: l, pending: p });
				}
				const newest = [...live].reverse().find((e) => holds(e.listed));
				expect(heldPending(ps, id, holds)).toEqual(newest?.pending ?? null);
			})
		);
	});
});
