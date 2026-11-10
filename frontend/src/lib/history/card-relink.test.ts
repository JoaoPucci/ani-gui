// Home's guessed cards, read again after a link: whether a card's guess
// still stands, and the rows matched again once it does not.

import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { createCardRelink, type CardRelinkDeps } from './card-relink';

const row = (id: string, kitsu_id?: string): HistoryEntry => ({
	id,
	title: id,
	ep_no: '1',
	...(kitsu_id ? { kitsu_id } : {})
});
const ref = (id: string) => ({ id }) as unknown as KitsuAnimeRef;
const flush = () => new Promise((r) => setTimeout(r, 0));

/** A Home whose history is `rows`, and whose backend lists `listed` next. */
function home(rows: HistoryEntry[], listed: HistoryEntry[][], over: Partial<CardRelinkDeps> = {}) {
	let history = rows;
	const settled = new Set<string>();
	const published: [string, string | null][] = [];
	// Publications and forgettings, in the order they happened.
	const events: string[] = [];
	const deps: CardRelinkDeps = {
		offerStands: async () => true,
		settled,
		current: () => history,
		list: vi.fn(async () => listed.shift() ?? history),
		order: (r) => r,
		setHistory: (r) => {
			history = r;
		},
		resolveMatch: async (h) => ref(h.kitsu_id ?? `guess-${h.id}`),
		publish: (id, m) => {
			published.push([id, m?.id ?? null]);
			events.push(`${id} is ${m?.id ?? 'nothing'}`);
		},
		forget: (id) => events.push(`${id} forgets`),
		...over
	};
	return {
		relink: createCardRelink(deps),
		deps,
		settled,
		published,
		events,
		history: () => history
	};
}

describe('whether a guessed card still stands', () => {
	it('stands while its link offer does, and nothing is read again', async () => {
		const h = home([row('a')], []);
		expect(await h.relink.stillGuessed('a', 'g')).toBe(true);
		expect(h.settled.has('a')).toBe(false);
		expect(h.deps.list).not.toHaveBeenCalled();
	});

	it('settles a row whose offer is gone, and reads the cards again', async () => {
		const h = home([row('a')], [[row('a', '5')]], { offerStands: async () => false });
		expect(await h.relink.stillGuessed('a', 'g')).toBe(false);
		expect(h.settled.has('a')).toBe(true);
		await flush();
		expect(h.history()).toEqual([row('a', '5')]);
		expect(h.published).toEqual([['a', '5']]);
	});

	it('settles the row even when the cards cannot be read again, and reads again next time', async () => {
		// The history read fails: the settling stands, nothing is left
		// unhandled, and the next read goes ahead.
		let fail = true;
		const h = home([row('a')], [], {
			offerStands: async () => false,
			list: async () => {
				if (fail) throw new Error('down');
				return [row('a', '5')];
			}
		});
		expect(await h.relink.stillGuessed('a', 'g')).toBe(false);
		expect(h.settled.has('a')).toBe(true);
		await flush();
		expect(h.history()).toEqual([row('a')]);
		fail = false;
		await h.relink.reread();
		expect(h.history()).toEqual([row('a', '5')]);
	});

	it('asks the offer for the row and the guess it was shown as', async () => {
		const offerStands = vi.fn(async () => true);
		const h = home([row('a')], [], { offerStands });
		await h.relink.stillGuessed('a', 'g');
		expect(offerStands).toHaveBeenCalledWith('a', 'g');
	});

	it('answers no for a check a later one overtook, settling nothing and reading nothing', async () => {
		const h = home([row('a')], [], { offerStands: async () => null });
		expect(await h.relink.stillGuessed('a', 'g')).toBe(false);
		await flush();
		expect(h.settled.has('a')).toBe(false);
		expect(h.deps.list).not.toHaveBeenCalled();
	});
});

describe('reading the cards again', () => {
	it('puts the history in Continue order and publishes the rows whose show changed', async () => {
		const h = home([row('a'), row('b'), row('c', '9')], [[row('a', '5'), row('b'), row('c')]], {
			order: (r) => [...r].reverse()
		});
		await h.relink.reread();
		expect(h.history().map((r) => r.id)).toEqual(['c', 'b', 'a']);
		await flush();
		expect(h.published.sort()).toEqual([
			['a', '5'],
			['c', 'guess-c']
		]);
	});

	it('publishes no show for a row whose match failed, as the first load does', async () => {
		const h = home([row('a')], [[row('a', '5')]], {
			resolveMatch: async () => Promise.reject(new Error('down'))
		});
		await h.relink.reread();
		await flush();
		expect(h.published).toEqual([['a', null]]);
	});

	it('reads again one at a time, each against the history the one before left', async () => {
		// Two links close together: the second's read compares with the
		// history the first left, so the row is matched again both times.
		const h = home([row('a')], [[row('a', '5')], [row('a')]]);
		await Promise.all([h.relink.reread(), h.relink.reread()]);
		await flush();
		expect(h.history()).toEqual([row('a')]);
		expect(h.published).toEqual([
			['a', '5'],
			['a', 'guess-a']
		]);
	});

	it('reads again when a remove or clear landed while its list was out', async () => {
		// The first list answers with the rows as they were, after a clear
		// landed meanwhile; the next one with what the clear left.
		let writes = 0;
		const h = home([row('a'), row('b')], [[row('a'), row('b')], []], {
			writes: () => writes
		});
		const list = h.deps.list;
		let first = true;
		h.deps.list = vi.fn(async () => {
			const rows = await list();
			if (first) writes++;
			first = false;
			return rows;
		});
		await h.relink.reread();
		expect(h.history()).toEqual([]);
		expect(h.deps.list).toHaveBeenCalledTimes(2);
	});

	it('matches nothing again from a read a remove or clear overtook', async () => {
		// The overtaken list still holds b, recorded as another show: b was
		// removed meanwhile, so nothing is matched or published for it.
		let writes = 0;
		const h = home([row('a'), row('b')], [[row('a'), row('b', '7')], [row('a')]], {
			writes: () => writes
		});
		const list = h.deps.list;
		let first = true;
		h.deps.list = vi.fn(async () => {
			const rows = await list();
			if (first) writes++;
			first = false;
			return rows;
		});
		await h.relink.reread();
		await flush();
		expect(h.history()).toEqual([row('a')]);
		expect(h.published).toEqual([]);
	});

	it('publishes nothing for a row a remove took away while its match was out', async () => {
		let answer!: (m: KitsuAnimeRef) => void;
		const h = home([row('a'), row('b')], [[row('a'), row('b', '7')]], {
			resolveMatch: () => new Promise((r) => (answer = r))
		});
		await h.relink.reread();
		// b is removed while its match is out.
		h.deps.setHistory([row('a')]);
		answer(ref('7'));
		await flush();
		expect(h.published).toEqual([]);
		expect(h.events).toEqual([]);
	});

	it('keeps reading after a read that failed', async () => {
		let fail = true;
		const h = home([row('a')], [[row('a', '5')]], {
			list: async () => {
				if (fail) {
					fail = false;
					throw new Error('down');
				}
				return [row('a', '5')];
			}
		});
		await expect(h.relink.reread()).rejects.toThrow('down');
		await h.relink.reread();
		expect(h.history()).toEqual([row('a', '5')]);
	});
});

describe("a row matched to another show forgets its former show's", () => {
	it('cap before its new match is published, and only for the rows matched again', async () => {
		// The cap a click pinned, the count a probe found: both were the
		// former show's.
		const h = home([row('a'), row('b')], [[row('a', '5'), row('b')]]);
		await h.relink.reread();
		await flush();
		expect(h.events).toEqual(['a forgets', 'a is 5']);
	});

	it('cap when its match failed too', async () => {
		const h = home([row('a')], [[row('a', '5')]], {
			resolveMatch: async () => Promise.reject(new Error('down'))
		});
		await h.relink.reread();
		await flush();
		expect(h.events).toEqual(['a forgets', 'a is nothing']);
	});
});

/** A match answered when the test says, one per ask. */
function heldMatches() {
	const asks: { entry: HistoryEntry; answer: () => void }[] = [];
	const resolveMatch = (entry: HistoryEntry) =>
		new Promise<KitsuAnimeRef | null>((resolve) => {
			asks.push({ entry, answer: () => resolve(ref(entry.kitsu_id ?? `guess-${entry.id}`)) });
		});
	return { asks, resolveMatch };
}

describe('what was asked for a row before it was matched again', () => {
	it('is told apart: a view taken before a read names the rows it matched again', async () => {
		const h = home([row('a'), row('b')], [[row('a', '5'), row('b')]]);
		const fromLoad = h.relink.since();
		expect(fromLoad('a')).toBe(true);
		await h.relink.reread();
		// The first load's answers for `a` are for the guess it started with.
		expect(fromLoad('a')).toBe(false);
		expect(fromLoad('b')).toBe(true);
		// A view taken now holds.
		expect(h.relink.since()('a')).toBe(true);
	});

	it("does not publish an earlier rematch's answer over a later one's", async () => {
		// The first link's match lands after the second's.
		const held = heldMatches();
		const h = home([row('a')], [[row('a', '5')], [row('a')]], {
			resolveMatch: held.resolveMatch
		});
		await h.relink.reread();
		await h.relink.reread();
		expect(held.asks.map((a) => a.entry.kitsu_id ?? null)).toEqual(['5', null]);
		held.asks[1].answer();
		await flush();
		held.asks[0].answer();
		await flush();
		expect(h.published).toEqual([['a', 'guess-a']]);
		expect(h.events).toEqual(['a forgets', 'a is guess-a']);
	});

	it('publishes only the latest rematch of a row, whatever order the answers land in', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc
					.integer({ min: 1, max: 5 })
					.chain((n) =>
						fc.tuple(fc.constant(n), fc.shuffledSubarray([...Array(n).keys()], { minLength: n }))
					),
				async ([n, order]) => {
					const held = heldMatches();
					// Each read finds the row recorded as another show.
					const listed = [...Array(n).keys()].map((i) => [row('a', String(i + 1))]);
					const h = home([row('a')], listed, { resolveMatch: held.resolveMatch });
					for (let i = 0; i < n; i++) await h.relink.reread();
					for (const i of order) {
						held.asks[i].answer();
						await flush();
					}
					expect(h.published).toEqual([['a', String(n)]]);
				}
			),
			{ numRuns: 50 }
		);
	});
});

describe('a view taken while a row is being matched again', () => {
	it('answers that the row changed, before and after the match lands', async () => {
		// A click on the card mid-rematch is a click on the show the row is
		// leaving: its cap lookup, landing after, is the former show's.
		const held = heldMatches();
		const h = home([row('a'), row('b')], [[row('a', '5'), row('b')]], {
			resolveMatch: held.resolveMatch
		});
		await h.relink.reread();
		const click = h.relink.since();
		expect(click('a')).toBe(false);
		expect(click('b')).toBe(true);
		held.asks[0].answer();
		await flush();
		expect(click('a')).toBe(false);
		// A view taken once the match landed holds.
		expect(h.relink.since()('a')).toBe(true);
	});

	it('answers for any row: changed exactly while it was or has since been matched again', async () => {
		await fc.assert(
			fc.asyncProperty(fc.boolean(), fc.boolean(), async (answerFirst, rereadAfter) => {
				const held = heldMatches();
				const h = home([row('a')], [[row('a', '5')], [row('a')]], {
					resolveMatch: held.resolveMatch
				});
				await h.relink.reread();
				if (answerFirst) {
					held.asks[0].answer();
					await flush();
				}
				const view = h.relink.since();
				if (rereadAfter) await h.relink.reread();
				for (const a of held.asks) a.answer();
				await flush();
				expect(view('a')).toBe(!(rereadAfter || !answerFirst));
			})
		);
	});
});
