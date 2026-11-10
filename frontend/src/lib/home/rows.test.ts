import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	homeRows,
	keepShown,
	type HomeRows,
	type HomeRowsInput,
	type WatchLaterInput
} from './rows';

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

describe('keepShown', () => {
	const all = (kind: HomeRows['trending']): HomeRows => ({
		continue: kind,
		watchLater: kind,
		trending: kind,
		topRated: kind
	});

	it('keeps a row shown when a row arriving late above it would take it back to a skeleton', () => {
		// The sign-ins counted as known once the keyring notice went up;
		// a sign-in read after that brings Watch later in loading above
		// rows already on screen.
		const before: HomeRows = {
			continue: 'absent',
			watchLater: 'absent',
			trending: 'shown',
			topRated: 'shown'
		};
		const next: HomeRows = {
			continue: 'absent',
			watchLater: 'skeleton',
			trending: 'skeleton',
			topRated: 'skeleton'
		};
		expect(keepShown(before, next)).toEqual({ ...next, trending: 'shown', topRated: 'shown' });
	});

	it('keeps a shown row shown over a held one too', () => {
		expect(keepShown(all('shown'), all('held'))).toEqual(all('shown'));
	});

	it('lets a shown row go when it is dropped', () => {
		expect(keepShown(all('shown'), all('absent'))).toEqual(all('absent'));
	});

	it('changes nothing on the first draw', () => {
		expect(keepShown(null, all('skeleton'))).toEqual(all('skeleton'));
	});
});

describe('the page drawn through keepShown, with a sign-in read after the keyring notice', () => {
	// The sign-ins count as known once the keyring notice goes up (the
	// row loads with none: an empty list), and a sign-in can be read
	// after that; its list then lands. The other rows land in any order
	// around them.
	type Event = 'history' | 'notice' | 'signIn' | 'list' | 'trending' | 'topRated';
	type Load = { items: string[] | null; failed: boolean };
	const load = (item: string) =>
		fc.constantFrom<Load>(
			{ items: [], failed: false },
			{ items: [item], failed: false },
			{ items: null, failed: true }
		);
	const finals = fc.record({
		history: fc.constantFrom<string[]>([], ['h1']),
		list: fc.constantFrom<{ cards: string[] | null; failed: boolean }>(
			{ cards: [], failed: false },
			{ cards: ['w1'], failed: false },
			{ cards: null, failed: true }
		),
		trending: load('t1'),
		topRated: load('r1')
	});
	const events: Event[] = ['history', 'notice', 'signIn', 'list', 'trending', 'topRated'];
	const run = fc
		.tuple(
			finals,
			fc.shuffledSubarray(events, { minLength: events.length, maxLength: events.length })
		)
		.filter(
			([, o]) =>
				o.indexOf('notice') < o.indexOf('signIn') && o.indexOf('signIn') < o.indexOf('list')
		);
	type Finals = typeof finals extends fc.Arbitrary<infer F> ? F : never;

	function build(f: Finals, done: Set<Event>): HomeRowsInput {
		const signedIn = done.has('signIn');
		return {
			history: done.has('history') ? f.history : null,
			watchLater: {
				accountsKnown: done.has('notice'),
				signedIn,
				// Signed out, the load finds nothing at once; signed in, the
				// list for the old set does not count until the new one lands.
				cards: !done.has('notice') ? null : !signedIn ? [] : done.has('list') ? f.list.cards : null,
				failed: done.has('list') && f.list.failed
			},
			trending: done.has('trending') ? f.trending : { items: null, failed: false },
			topRated: done.has('topRated') ? f.topRated : { items: null, failed: false }
		};
	}
	function views(f: Finals, order: Event[]) {
		const done = new Set<Event>();
		let drawn = keepShown(null, homeRows(build(f, done)));
		const seen = [{ drawn, input: build(f, done) }];
		for (const e of order) {
			done.add(e);
			const input = build(f, done);
			drawn = keepShown(drawn, homeRows(input));
			seen.push({ drawn, input });
		}
		return seen;
	}
	const ROWS = ['continue', 'watchLater', 'trending', 'topRated'] as const;

	it('never takes a shown row back to a skeleton or an empty slot', () => {
		fc.assert(
			fc.property(run, ([f, order]) => {
				const seen = views(f, order);
				seen.forEach(({ drawn }, k) => {
					for (const row of ROWS) {
						if (drawn[row] !== 'shown') continue;
						for (const later of seen.slice(k + 1)) {
							expect(['shown', 'absent']).toContain(later.drawn[row]);
						}
					}
				});
			})
		);
	});

	it('keeps a catalogue or history row shown only while it has something in it', () => {
		fc.assert(
			fc.property(run, ([f, order]) => {
				for (const { drawn, input } of views(f, order)) {
					if (drawn.continue === 'shown') expect(input.history?.length).toBeGreaterThan(0);
					if (drawn.trending === 'shown') expect(input.trending.items?.length).toBeGreaterThan(0);
					if (drawn.topRated === 'shown') expect(input.topRated.items?.length).toBeGreaterThan(0);
				}
			})
		);
	});

	it('ends as the page would be drawn from scratch', () => {
		fc.assert(
			fc.property(run, ([f, order]) => {
				const last = views(f, order).at(-1)!;
				expect(last.drawn).toEqual(homeRows(last.input));
			})
		);
	});
});
