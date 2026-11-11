import { describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import { createLiveSearch, type LiveSearchDeps, type LiveSearchView } from './live-search';

const hit = (id: string) => ({ id, canonical_title: `Show ${id}` }) as unknown as KitsuAnimeRef;
const flush = () => new Promise((r) => setTimeout(r, 0));

/** A search held until the case answers it, with the signal it went out with. */
interface Held {
	q: string;
	signal: AbortSignal;
	answer(hits: KitsuAnimeRef[]): void;
	fail(e: unknown): void;
}

function harness(overrides: Partial<LiveSearchDeps> = {}) {
	let query = '';
	const views: LiveSearchView[] = [];
	const held: Held[] = [];
	const timers: Array<{ run: () => void; cancelled: boolean }> = [];
	const filterSignals: AbortSignal[] = [];
	const live = createLiveSearch({
		search: (q, signal) =>
			new Promise((resolve, reject) => {
				signal.addEventListener('abort', () =>
					reject(new DOMException('The operation was aborted.', 'AbortError'))
				);
				held.push({ q, signal, answer: resolve, fail: reject });
			}),
		filter: async (hits, signal) => {
			filterSignals.push(signal);
			return hits;
		},
		currentQuery: () => query,
		changed: (v) => views.push(v),
		debounceMs: 250,
		minChars: 2,
		maxHits: 2,
		schedule: (run) => {
			const t = { run, cancelled: false };
			timers.push(t);
			return t;
		},
		cancel: (t) => ((t as { cancelled: boolean }).cancelled = true),
		...overrides
	});
	/** The viewer types `q`: the field holds it, and the search is scheduled. */
	const type = (q: string) => {
		query = q;
		live.schedule(q);
	};
	/** The typing settles: every pending pause ends. */
	const settle = () => {
		for (const t of timers.splice(0)) if (!t.cancelled) t.run();
	};
	return {
		live,
		views,
		held,
		timers,
		filterSignals,
		type,
		settle,
		setQuery: (q: string) => (query = q)
	};
}

describe('createLiveSearch', () => {
	it('clears the dropdown for a query too short to search, and searches nothing', () => {
		const h = harness();
		h.type('f');
		h.settle();
		expect(h.held).toEqual([]);
		expect(h.live.view).toEqual({ results: null, busy: false, error: false });
	});

	it('searches once the typing settles, and shows its hits, at most maxHits', async () => {
		const h = harness();
		h.type('frieren');
		expect(h.held).toEqual([]);
		h.settle();
		expect(h.live.view.busy).toBe(true);
		h.held[0].answer([hit('1'), hit('2'), hit('3')]);
		await flush();
		expect(h.live.view).toEqual({ results: [hit('1'), hit('2')], busy: false, error: false });
	});

	it('sends no search for a query typed past before its pause ended', () => {
		const h = harness();
		h.type('fri');
		h.type('frieren');
		h.settle();
		expect(h.held.map((s) => s.q)).toEqual(['frieren']);
	});

	it('cancels the search out when the next keystroke comes, and shows nothing about it', async () => {
		const h = harness();
		h.type('fri');
		h.settle();
		h.type('frieren');
		expect(h.held[0].signal.aborted).toBe(true);
		await flush();
		expect(h.live.view.error).toBe(false);
		expect(h.live.view.results).toBeNull();
	});

	it('drops an answer for a query the field no longer holds', async () => {
		const h = harness();
		h.type('fri');
		h.settle();
		h.setQuery('frie');
		h.held[0].answer([hit('1')]);
		await flush();
		expect(h.live.view.results).toBeNull();
		expect(h.live.view.busy).toBe(false);
	});

	it('drops filtered hits when the field moved on while the filter ran', async () => {
		let typedOn = () => {};
		const h = harness({
			filter: async (hits) => {
				typedOn();
				return hits;
			}
		});
		typedOn = () => h.setQuery('frieren s');
		h.type('frieren');
		h.settle();
		h.held[0].answer([hit('1')]);
		await flush();
		expect(h.live.view.results).toBeNull();
	});

	it('hands the search its signal to the filter too', async () => {
		const h = harness();
		h.type('frieren');
		h.settle();
		h.held[0].answer([hit('1')]);
		await flush();
		expect(h.filterSignals).toEqual([h.held[0].signal]);
	});

	it('shows a failed search as a failure with no hits', async () => {
		const h = harness();
		h.type('frieren');
		h.settle();
		h.held[0].fail({ kind: 'http', status: 503 });
		await flush();
		expect(h.live.view).toEqual({ results: [], busy: false, error: true });
	});

	it('on close, cancels the search out and the one waiting, and drops hits of a query typed past', async () => {
		const h = harness();
		h.type('fri');
		h.settle();
		h.held[0].answer([hit('1')]);
		await flush();
		h.type('frie');
		h.settle();
		h.type('frieren');
		h.live.close();
		expect(h.held[1].signal.aborted).toBe(true);
		expect(h.timers.every((t) => t.cancelled)).toBe(true);
		h.settle();
		await flush();
		expect(h.held).toHaveLength(2);
		expect(h.live.view).toEqual({ results: null, busy: false, error: false });
	});

	it('on close, keeps the hits of the query the field still holds', async () => {
		const h = harness();
		h.type('frieren');
		h.settle();
		h.held[0].answer([hit('1')]);
		await flush();
		h.live.close();
		expect(h.live.view.results).toEqual([hit('1')]);
	});

	it('cancels the search out when the layout goes', () => {
		const h = harness();
		h.type('frieren');
		h.settle();
		h.live.destroy();
		expect(h.held[0].signal.aborted).toBe(true);
	});

	it('runs on the real timer when none is given', async () => {
		vi.useFakeTimers();
		try {
			const search = vi.fn(async () => [] as KitsuAnimeRef[]);
			const live = createLiveSearch({
				search,
				filter: async (hits) => hits,
				currentQuery: () => 'frieren',
				changed: () => {},
				debounceMs: 250,
				minChars: 2,
				maxHits: 5
			});
			live.schedule('frieren');
			live.schedule('frieren');
			await vi.advanceTimersByTimeAsync(300);
			expect(search).toHaveBeenCalledTimes(1);
		} finally {
			vi.useRealTimers();
		}
	});
});
