import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	changeOf,
	entryNeedsReread,
	railNeedsReread,
	writeEnd,
	type WriteEnd
} from './list-changes';
import { SupersededError } from './save-when-allowed';
import type { Provider } from './types';

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];
const end = fc.constantFrom<WriteEnd>('landed', 'failed', 'dropped');
const ends = fc.record(
	{ anilist: end, mal: end, inhouse: end },
	{ requiredKeys: [] }
) as fc.Arbitrary<Partial<Record<Provider, WriteEnd>>>;

const source = fc.constantFrom('editor', 'progress' as const);
const trackers = fc.subarray(PROVIDERS);
/** Where a tracker sits in the fixed tracker order. */
const rank = (p: Provider) => PROVIDERS.indexOf(p);
const withEnd = (
	byTracker: Partial<Record<Provider, WriteEnd>>,
	p: Provider,
	e: WriteEnd | undefined
): Partial<Record<Provider, WriteEnd>> => {
	const next = { ...byTracker };
	if (e === undefined) delete next[p];
	else next[p] = e;
	return next;
};

describe('changeOf, by property', () => {
	it('names every tracker a request reached, and only trackers that had a write', () => {
		fc.assert(
			fc.property(ends, source, trackers, (byTracker, src, attempted) => {
				const change = changeOf('7', src, byTracker, attempted);
				const named = change?.providers ?? [];
				for (const p of PROVIDERS) {
					// A request that came back, either way, may have changed a list.
					if (byTracker[p] === 'landed' || byTracker[p] === 'failed') expect(named).toContain(p);
					// A tracker the write never went to changed nothing.
					if (byTracker[p] === undefined) expect(named).not.toContain(p);
				}
			})
		);
	});

	it('is null exactly when it would name nothing, and otherwise names each tracker once, in order', () => {
		fc.assert(
			fc.property(ends, source, trackers, (byTracker, src, attempted) => {
				const change = changeOf('7', src, byTracker, attempted);
				if (change === null) return;
				expect(change.providers.length).toBeGreaterThan(0);
				expect(new Set(change.providers).size).toBe(change.providers.length);
				const ranks = change.providers.map(rank);
				expect(ranks).toEqual([...ranks].sort((a, b) => a - b));
				expect(change.kitsuId).toBe('7');
				expect(change.source).toBe(src);
			})
		);
	});

	it('says which trackers took it, all of them among those it names', () => {
		fc.assert(
			fc.property(ends, source, trackers, (byTracker, src, attempted) => {
				const change = changeOf('7', src, byTracker, attempted);
				const landed = change?.landed ?? [];
				for (const p of PROVIDERS) expect(landed.includes(p)).toBe(byTracker[p] === 'landed');
				for (const p of landed) expect(change?.providers).toContain(p);
			})
		);
	});

	it('cannot be told about a drop with no request behind it', () => {
		fc.assert(
			fc.property(ends, source, trackers, (byTracker, src, attempted) => {
				// A dropped tracker no request went to reads the same as one
				// the write was never for.
				const withDrops = changeOf('7', src, byTracker, []);
				let withoutDrops = byTracker;
				for (const p of PROVIDERS)
					if (byTracker[p] === 'dropped') withoutDrops = withEnd(withoutDrops, p, undefined);
				expect(withDrops).toEqual(changeOf('7', src, withoutDrops, []));
				// And an editor write is told nothing by the attempts list.
				if (src === 'editor') expect(changeOf('7', src, byTracker, attempted)).toEqual(withDrops);
			})
		);
	});

	it('only grows as more of a progress write is known to have gone out', () => {
		fc.assert(
			fc.property(ends, trackers, trackers, (byTracker, some, more) => {
				const fewer = changeOf('7', 'progress', byTracker, some)?.providers ?? [];
				const all = changeOf('7', 'progress', byTracker, [...some, ...more])?.providers ?? [];
				for (const p of fewer) expect(all).toContain(p);
			})
		);
	});
});

const change = fc
	.tuple(fc.subarray(PROVIDERS, { minLength: 1 }), trackers, source)
	.map(([providers, tookIt, src]) => ({
		kitsuId: '7',
		providers,
		landed: providers.filter((p) => tookIt.includes(p)),
		source: src
	}));

describe('railNeedsReread, by property', () => {
	it('never rereads for trackers the row does not show', () => {
		fc.assert(
			fc.property(change, (c) => {
				expect(railNeedsReread(c, [])).toBe(false);
				const unshown = PROVIDERS.filter((p) => !c.providers.includes(p));
				expect(railNeedsReread(c, unshown)).toBe(false);
			})
		);
	});

	it('rereads whenever a shown tracker took the write', () => {
		fc.assert(
			fc.property(change, trackers, (c, shown) => {
				if (c.landed.some((p) => shown.includes(p))) expect(railNeedsReread(c, shown)).toBe(true);
			})
		);
	});

	it('stays wanted as the row shows more trackers', () => {
		fc.assert(
			fc.property(change, trackers, trackers, (c, shown, more) => {
				if (railNeedsReread(c, shown)) expect(railNeedsReread(c, [...shown, ...more])).toBe(true);
			})
		);
	});

	it('rereads for a progress write however its trackers ended, since a failed one can have cached earlier entries', () => {
		fc.assert(
			fc.property(change, trackers, trackers, (c, shown, tookIt) => {
				const progress = { ...c, source: 'progress' as const };
				const otherEnds = { ...progress, landed: c.providers.filter((p) => tookIt.includes(p)) };
				expect(railNeedsReread(progress, shown)).toBe(railNeedsReread(otherEnds, shown));
				if (c.providers.some((p) => shown.includes(p)))
					expect(railNeedsReread(progress, shown)).toBe(true);
			})
		);
	});

	it('ignores an editor write that failed, which left one entry as it was', () => {
		fc.assert(
			fc.property(change, trackers, (c, shown) => {
				const editor = { ...c, source: 'editor' as const };
				// Trackers that failed can be added or taken away; only
				// those that took the write decide.
				const onlyLanded = { ...editor, providers: editor.landed };
				expect(railNeedsReread(editor, shown)).toBe(railNeedsReread(onlyLanded, shown));
			})
		);
	});
});

describe('writeEnd, by property', () => {
	it('is landed for any answer, dropped for a superseded write, and failed for anything else thrown', () => {
		fc.assert(
			fc.property(fc.anything(), (value) => {
				expect(writeEnd({ status: 'fulfilled', value })).toBe('landed');
				expect(writeEnd({ status: 'rejected', reason: value })).toBe('failed');
			})
		);
		expect(writeEnd({ status: 'rejected', reason: new SupersededError() })).toBe('dropped');
	});
});

describe('entryNeedsReread, by property', () => {
	it("is wanted exactly for a progress write of the page's own show", () => {
		fc.assert(
			fc.property(
				fc.constantFrom('1', '2', '3'),
				fc.option(fc.constantFrom('1', '2', '3', ''), { nil: null }),
				fc.constantFrom('editor', 'progress' as const),
				(changed, onPage, source) => {
					const wanted = entryNeedsReread(
						{ kitsuId: changed, providers: ['anilist'], landed: ['anilist'], source },
						onPage
					);
					expect(wanted).toBe(source === 'progress' && onPage === changed);
				}
			)
		);
	});
});
