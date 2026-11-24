/**
 * fast-check arbitraries for the watch-order modules' properties: runs
 * of a few entries with clashing and missing dates, and links that may
 * name strangers, repeat, or point an entry at itself, so a property
 * covers what a malformed payload could carry.
 */
import fc from 'fast-check';
import type { FranchiseData, RunLink, WatchEntry } from '../types';

const DATES = ['2001-04-01', '2004-10-05', '2004-10-05', '2012-07-01', '2030-01-01'];

export function entryArb(id: string): fc.Arbitrary<WatchEntry> {
	return fc.record({
		id: fc.constant(id),
		title: fc.constantFrom('A', 'A: B', 'A: C', `T${id}`),
		subtype: fc.constantFrom('TV', 'movie', 'OVA', null),
		start: fc.option(fc.constantFrom(...DATES), { nil: null }),
		episodes: fc.option(fc.integer({ min: 1, max: 30 }), { nil: null }),
		status: fc.constantFrom('finished', 'current', null),
		poster: fc.constant(null),
		cover: fc.constant(null)
	});
}

/** `count` entries with distinct numeric ids. */
export function entriesArb(min = 1, max = 7): fc.Arbitrary<WatchEntry[]> {
	return fc
		.uniqueArray(fc.integer({ min: 1, max: 60 }), { minLength: min, maxLength: max })
		.chain((ids) => fc.tuple(...ids.map((i) => entryArb(String(i)))));
}

/** Links between members of `ids`, which must not be empty. */
export function linksArb(ids: readonly string[], max = 10): fc.Arbitrary<RunLink[]> {
	const id = fc.constantFrom(...ids);
	return fc.array(fc.tuple(id, id), { maxLength: max });
}

/** A payload of any shape the parser could hand over, malformed links and parents included. */
export function franchiseArb(): fc.Arbitrary<FranchiseData> {
	return entriesArb(0, 7).chain((entries) => {
		const ids = entries.map((e) => e.id);
		const anyId = fc.constantFrom(...ids, '999', '0');
		return fc.record({
			anchor: anyId,
			entries: fc.constant(entries),
			links: fc.array(fc.tuple(anyId, anyId), { maxLength: 10 }),
			versions: fc.constant([]),
			films: fc.constant([]),
			sides: fc.constant([]),
			recaps: fc.constant([]),
			parents: fc.dictionary(anyId, fc.array(anyId, { maxLength: 3 }), { maxKeys: 2 }),
			complete: fc.boolean()
		});
	});
}
