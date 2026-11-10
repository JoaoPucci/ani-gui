import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { homeRows, type HomeRowsInput, type WatchLaterInput } from './rows';

/** Everything settled with something in it: every row shown. */
const settled: HomeRowsInput = {
	history: ['h1'],
	watchLater: { accountsKnown: true, signedIn: true, cards: ['w1'], failed: false },
	trending: { items: ['t1'], failed: false },
	topRated: { items: ['r1'], failed: false }
};
const wl = (over: Partial<WatchLaterInput>): HomeRowsInput => ({
	...settled,
	watchLater: { ...settled.watchLater, ...over }
});
const kinds = (input: HomeRowsInput) => {
	const rows = homeRows(input);
	return [rows.continue, rows.watchLater, rows.trending, rows.topRated];
};

describe('homeRows', () => {
	it('shows every row once everything has landed', () => {
		expect(kinds(settled)).toEqual(['shown', 'shown', 'shown', 'shown']);
	});

	it('draws nothing below Continue watching while the history read is out', () => {
		// Continue watching has no placeholder: most viewers who have
		// watched nothing never see one. A row drawn below it would be
		// pushed down when it turns out to be there.
		expect(kinds({ ...settled, history: null })).toEqual(['held', 'held', 'held', 'held']);
	});

	it('leaves Continue watching out when there is no history', () => {
		expect(kinds({ ...settled, history: [] })).toEqual(['absent', 'shown', 'shown', 'shown']);
	});

	it('draws nothing below Watch later until the saved sign-ins have been read', () => {
		expect(kinds(wl({ accountsKnown: false, cards: null }))).toEqual([
			'shown',
			'held',
			'held',
			'held'
		]);
	});

	it('leaves Watch later out for a viewer with no sign-in', () => {
		expect(kinds(wl({ signedIn: false, cards: [] }))).toEqual([
			'shown',
			'absent',
			'shown',
			'shown'
		]);
	});

	it('holds a skeleton for Watch later while a signed-in list loads, and the rows below with it', () => {
		// Not the "nothing planned" row: that is what the list says once
		// it has loaded, not while it loads.
		expect(kinds(wl({ cards: null }))).toEqual(['shown', 'skeleton', 'skeleton', 'skeleton']);
	});

	it("shows a signed-in viewer's empty list as the row's empty state", () => {
		expect(kinds(wl({ cards: [] }))[1]).toBe('shown');
	});

	it("shows a signed-in viewer's failed list as the row's failed state", () => {
		expect(kinds(wl({ cards: null, failed: true }))[1]).toBe('shown');
	});

	it('keeps Top rated a skeleton while Trending, above it, is still loading', () => {
		expect(kinds({ ...settled, trending: { items: null, failed: false } }).slice(2)).toEqual([
			'skeleton',
			'skeleton'
		]);
	});

	it('drops Trending when it fails, and shows Top rated in its place', () => {
		expect(kinds({ ...settled, trending: { items: null, failed: true } }).slice(2)).toEqual([
			'absent',
			'shown'
		]);
	});

	it('drops Trending when nothing is left in it', () => {
		// The row lists what the hero does not; a short list can leave
		// it nothing.
		expect(kinds({ ...settled, trending: { items: [], failed: false } }).slice(2)).toEqual([
			'absent',
			'shown'
		]);
	});
});

describe('homeRows, as the page loads in any order', () => {
	// One event per thing the page waits on. Each moves one input from
	// its starting state to its final one; the Watch Later list cannot
	// land before the sign-ins are read.
	type Event = 'history' | 'accounts' | 'list' | 'trending' | 'topRated';
	type Load = { items: string[] | null; failed: boolean };
	interface Finals {
		history: string[];
		signedIn: boolean;
		list: { cards: string[] | null; failed: boolean };
		trending: Load;
		topRated: Load;
	}
	const finals: fc.Arbitrary<Finals> = fc.record({
		history: fc.constantFrom<string[]>([], ['h1']),
		signedIn: fc.boolean(),
		list: fc.constantFrom<{ cards: string[] | null; failed: boolean }>(
			{ cards: [], failed: false },
			{ cards: ['w1'], failed: false },
			{ cards: null, failed: true }
		),
		trending: fc.constantFrom<Load>(
			{ items: [], failed: false },
			{ items: ['t1'], failed: false },
			{ items: null, failed: true }
		),
		topRated: fc.constantFrom<Load>(
			{ items: [], failed: false },
			{ items: ['r1'], failed: false },
			{ items: null, failed: true }
		)
	});
	const events: Event[] = ['history', 'accounts', 'list', 'trending', 'topRated'];
	const run = fc
		.tuple(
			finals,
			fc.shuffledSubarray(events, { minLength: events.length, maxLength: events.length })
		)
		.filter(([, order]) => order.indexOf('accounts') < order.indexOf('list'));

	function views(f: Finals, order: Event[]) {
		const done = new Set<Event>();
		const seen = [kinds(build(f, done))];
		for (const e of order) {
			done.add(e);
			seen.push(kinds(build(f, done)));
		}
		return seen;
	}
	function build(f: Finals, done: Set<Event>): HomeRowsInput {
		const accounts = done.has('accounts');
		// Signed out, the row's load finds nothing at once.
		const listLanded = accounts && (!f.signedIn || done.has('list'));
		return {
			history: done.has('history') ? f.history : null,
			watchLater: {
				accountsKnown: accounts,
				signedIn: accounts && f.signedIn,
				cards: listLanded ? (f.signedIn ? f.list.cards : []) : null,
				failed: listLanded && f.signedIn && f.list.failed
			},
			trending: done.has('trending') ? f.trending : { items: null, failed: false },
			topRated: done.has('topRated') ? f.topRated : { items: null, failed: false }
		};
	}
	const drawn = (k: string) => k === 'skeleton' || k === 'shown';

	it('never moves a row once it is shown, nor draws a row above one already drawn', () => {
		fc.assert(
			fc.property(run, ([f, order]) => {
				const seen = views(f, order);
				seen.forEach((view, k) => {
					view.forEach((slot, i) => {
						if (!drawn(slot)) return;
						for (const later of seen.slice(k + 1)) {
							expect(later.slice(0, i).filter(drawn).length).toBeLessThanOrEqual(
								view.slice(0, i).filter(drawn).length
							);
							if (slot === 'shown') expect(later.slice(0, i + 1)).toEqual(view.slice(0, i + 1));
						}
					});
				});
			})
		);
	});

	it('ends with nothing held and no skeleton left', () => {
		fc.assert(
			fc.property(run, ([f, order]) => {
				const last = views(f, order).at(-1)!;
				expect(last.every((k) => k === 'shown' || k === 'absent')).toBe(true);
			})
		);
	});
});
