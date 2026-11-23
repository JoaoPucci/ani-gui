import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';

import { RECENT_LIMIT, mergeRecents, parseStoredRecents } from './recent-searches';

describe('mergeRecents', () => {
	it('prepends a brand-new query', () => {
		expect(mergeRecents(['demon slayer'], 'jojo')).toEqual(['jojo', 'demon slayer']);
	});

	it('promotes an existing query to the front (no duplicate)', () => {
		expect(mergeRecents(['naruto', 'demon slayer', 'jojo'], 'demon slayer')).toEqual([
			'demon slayer',
			'naruto',
			'jojo'
		]);
	});

	it('replaces an entry that differs from the query only in case or spacing', () => {
		expect(mergeRecents(['one piece', 'jojo'], 'One  Piece')).toEqual(['One  Piece', 'jojo']);
	});

	it('collapses entries in the existing list that are the same search', () => {
		expect(mergeRecents(['Jojo', 'jojo ', 'bleach'], 'naruto')).toEqual([
			'naruto',
			'Jojo',
			'bleach'
		]);
	});

	it('caps the result at `max`', () => {
		const existing = ['a', 'b', 'c', 'd', 'e'];
		expect(mergeRecents(existing, 'f', 5)).toEqual(['f', 'a', 'b', 'c', 'd']);
	});

	it('uses the public RECENT_LIMIT default when max is omitted', () => {
		// Document the default — if it changes, this test catches the
		// drift between code and any UI copy that mentions "last 5".
		const existing = Array.from({ length: 10 }, (_, i) => `q${i}`);
		const got = mergeRecents(existing, 'fresh');
		expect(got).toHaveLength(RECENT_LIMIT);
		expect(got[0]).toBe('fresh');
	});
});

describe('parseStoredRecents', () => {
	it('returns [] for null input (no key present)', () => {
		expect(parseStoredRecents(null)).toEqual([]);
	});

	it('returns [] for malformed JSON', () => {
		expect(parseStoredRecents('not-json')).toEqual([]);
	});

	it('returns [] when the payload is not an array', () => {
		expect(parseStoredRecents('{"foo":"bar"}')).toEqual([]);
	});

	it('drops non-string entries defensively', () => {
		expect(parseStoredRecents('["jojo", 42, null, "naruto"]')).toEqual(['jojo', 'naruto']);
	});

	it('caps at max', () => {
		const raw = JSON.stringify(['a', 'b', 'c', 'd', 'e', 'f']);
		expect(parseStoredRecents(raw, 3)).toEqual(['a', 'b', 'c']);
	});

	it('lists a repeated entry once, at its first place', () => {
		const raw = JSON.stringify(['naruto', 'jojo', 'naruto', 'bleach']);
		expect(parseStoredRecents(raw)).toEqual(['naruto', 'jojo', 'bleach']);
	});

	it('counts entries that differ only in case or spacing as one search, as the backend does', () => {
		const raw = JSON.stringify(['One Piece', 'one piece', '  ONE   PIECE ']);
		expect(parseStoredRecents(raw)).toEqual(['One Piece']);
	});

	it('drops empty and blank entries and trims the rest', () => {
		expect(parseStoredRecents(JSON.stringify(['', '   ', ' jojo  ']))).toEqual(['jojo']);
	});

	it('does not let repeats use up the cap', () => {
		const raw = JSON.stringify(['a', 'a', 'A', 'b', 'c', 'd', 'e', 'f']);
		expect(parseStoredRecents(raw, 5)).toEqual(['a', 'b', 'c', 'd', 'e']);
	});
});

describe('mergeRecents (properties)', () => {
	// `mergeRecents` is purely "prepend, dedupe, cap" — the component
	// owns trimming and skipping empty queries before calling. These
	// properties pin the contract at THIS layer, not the one above.
	const anyQuery = fc.string({ minLength: 1, maxLength: 20 });
	const anyExisting = fc.array(fc.string({ minLength: 1, maxLength: 20 }), { maxLength: 20 });

	it('result length never exceeds max', () => {
		fc.assert(
			fc.property(
				anyExisting,
				anyQuery,
				fc.integer({ min: 1, max: 10 }),
				(existing, query, max) => mergeRecents(existing, query, max).length <= max
			)
		);
	});

	it('the query is always the first element of the result', () => {
		fc.assert(
			fc.property(
				anyExisting,
				anyQuery,
				(existing, query) => mergeRecents(existing, query, RECENT_LIMIT)[0] === query
			)
		);
	});

	it('result has no duplicate entries (query is deduped against existing)', () => {
		fc.assert(
			fc.property(anyExisting, anyQuery, (existing, query) => {
				const out = mergeRecents(existing, query, RECENT_LIMIT);
				return new Set(out).size === out.length;
			})
		);
	});

	it('every element of the result either is the query or appeared in existing', () => {
		fc.assert(
			fc.property(anyExisting, anyQuery, (existing, query) => {
				const out = mergeRecents(existing, query, RECENT_LIMIT);
				const existingSet = new Set(existing);
				return out.every((x) => x === query || existingSet.has(x));
			})
		);
	});
});

/** The search a recent entry stands for: case aside, with its spacing
 *  collapsed, which the backend's search key ignores too. Two recent
 *  entries with the same key are the same search. */
const searchKey = (q: string) => q.trim().toLowerCase().split(/\s+/).join(' ');

describe('recent searches with repeats (properties)', () => {
	// A pool small enough that repeats, case variants and spacing
	// variants are common, mixed with the other shapes a corrupted
	// stored value can hold.
	const entry = fc.oneof(
		fc.constantFrom('naruto', 'Naruto', ' naruto ', 'one piece', 'One  Piece', 'jojo', 'bleach'),
		fc.constantFrom('', '   '),
		fc.string({ maxLength: 6 }),
		fc.integer(),
		fc.constant(null),
		fc.constant({ q: 'naruto' })
	);
	const stored = fc.array(entry, { maxLength: 30 });
	const maxes = fc.integer({ min: 1, max: 10 });

	it('reads back no search twice, and no blank or untrimmed entry', () => {
		fc.assert(
			fc.property(stored, maxes, (list, max) => {
				const out = parseStoredRecents(JSON.stringify(list), max);
				expect(new Set(out.map(searchKey)).size).toBe(out.length);
				for (const q of out) {
					expect(q).toBe(q.trim());
					expect(q.length).toBeGreaterThan(0);
				}
			})
		);
	});

	it('keeps each search as its first entry, in stored order, dropping only repeats and the cap', () => {
		fc.assert(
			fc.property(stored, maxes, (list, max) => {
				const out = parseStoredRecents(JSON.stringify(list), max);
				const strings = list
					.filter((x): x is string => typeof x === 'string')
					.map((x) => x.trim())
					.filter((x) => x.length > 0);
				const firstAt = out.map((q) => strings.findIndex((x) => searchKey(x) === searchKey(q)));
				out.forEach((q, i) => expect(q).toBe(strings[firstAt[i]]));
				expect(firstAt).toEqual([...firstAt].sort((a, b) => a - b));
				const distinct = new Set(strings.map(searchKey)).size;
				expect(out.length).toBe(Math.min(max, distinct));
			})
		);
	});

	it('a new search never leaves two entries for the same search', () => {
		fc.assert(
			fc.property(stored, fc.constantFrom('naruto', 'NARUTO', 'one  piece', 'jojo'), (list, q) => {
				const out = mergeRecents(parseStoredRecents(JSON.stringify(list)), q);
				expect(out[0]).toBe(q);
				expect(new Set(out.map(searchKey)).size).toBe(out.length);
			})
		);
	});
});

describe('parseStoredRecents (properties)', () => {
	it('always returns a string array regardless of input shape', () => {
		fc.assert(
			fc.property(fc.anything(), (raw) => {
				const out = parseStoredRecents(JSON.stringify(raw));
				return Array.isArray(out) && out.every((s) => typeof s === 'string');
			})
		);
	});

	it('malformed JSON inputs return an empty array, not throw', () => {
		fc.assert(
			fc.property(fc.string({ maxLength: 50 }), (raw) => {
				const out = parseStoredRecents(raw);
				return Array.isArray(out);
			})
		);
	});
});
