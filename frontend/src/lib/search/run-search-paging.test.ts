// The runner's infinite-scroll half: pages after the first, appended
// through the same availability filter, with the paging phase the
// page renders its bottom-of-grid state from.

import { describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef, KitsuSearchPage } from '$lib/api';

vi.mock('$lib/api', () => ({}));

import { createSearchRunner, type SearchRunnerDeps } from './run-search';
import { MAX_EMPTY_PAGES, type PagingPhase } from './paging';

function ref(id: string): KitsuAnimeRef {
	return { id, canonical_title: `Title ${id}` } as KitsuAnimeRef;
}
const ids = (list: KitsuAnimeRef[]) => list.map((h) => h.id);

function page(hitIds: string[], next: number | null): KitsuSearchPage {
	return { hits: hitIds.map(ref), next_offset: next };
}

function defer<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

async function drain(): Promise<void> {
	for (let i = 0; i < 20; i++) await Promise.resolve();
}

/** A runner whose Kitsu answers from `pages` by offset (query-blind
 *  unless `byQuery` is given) and whose filter keeps every hit except
 *  the ids in `unavailable`. */
function harness(opts: {
	pages?: Record<number, KitsuSearchPage | Error>;
	searchPage?: SearchRunnerDeps['searchPage'];
	unavailable?: Set<string>;
	filter?: SearchRunnerDeps['filter'];
}) {
	const results: KitsuAnimeRef[][] = [];
	const phases: PagingPhase[] = [];
	const errors: unknown[] = [];
	const filtered: string[][] = [];
	const searchPage =
		opts.searchPage ??
		vi.fn(async (_q: string, offset: number) => {
			const p = opts.pages?.[offset];
			if (!p) throw new Error(`no page at ${offset}`);
			if (p instanceof Error) throw p;
			return p;
		});
	const deps: SearchRunnerDeps = {
		searchPage,
		getConfig: async () => null,
		pickMode: () => 'sub',
		filter:
			opts.filter ??
			(async (items, _mode, emit) => {
				filtered.push(ids(items));
				emit(items.filter((i) => !opts.unavailable?.has(i.id)));
			}),
		onResults: (v) => results.push(v),
		onError: (e) => errors.push(e),
		onBusy: () => {},
		onPhase: (p) => phases.push(p)
	};
	const runner = createSearchRunner(deps);
	return {
		runner,
		searchPage,
		results,
		phases,
		errors,
		filtered,
		latest: () => ids(results.at(-1) ?? []),
		phase: () => phases.at(-1)?.kind
	};
}

describe('createSearchRunner paging', () => {
	it('settles idle after a first page with more behind it, and appends the next on demand', async () => {
		const h = harness({ pages: { 0: page(['1', '2'], 20), 20: page(['3'], 40) } });
		await h.runner.run('dragon ball');
		expect(h.phases.map((p) => p.kind)).toEqual(['loading', 'idle']);
		expect(h.latest()).toEqual(['1', '2']);

		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenLastCalledWith('dragon ball', 20);
		expect(h.phases.map((p) => p.kind)).toEqual(['loading', 'idle', 'loading-more', 'idle']);
		expect(h.latest()).toEqual(['1', '2', '3']);
	});

	it('the last page ends the search, and asking for more then fetches nothing', async () => {
		const h = harness({ pages: { 0: page(['1'], 20), 20: page(['2'], null) } });
		await h.runner.run('q');
		h.runner.loadMore();
		await drain();
		expect(h.phase()).toBe('end');
		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenCalledTimes(2);
		expect(h.latest()).toEqual(['1', '2']);
	});

	it('a failed page keeps what is shown, reports failed, and asking again retries the same offset', async () => {
		const fail = new Error('kitsu down');
		const pages: Record<number, KitsuSearchPage | Error> = { 0: page(['1'], 20), 20: fail };
		const h = harness({ pages });
		await h.runner.run('q');
		h.runner.loadMore();
		await drain();
		expect(h.phases.at(-1)).toEqual({ kind: 'failed', error: fail });
		expect(h.errors).toEqual([]);
		expect(h.latest()).toEqual(['1']);

		pages[20] = page(['2'], null);
		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenLastCalledWith('q', 20);
		expect(h.phase()).toBe('end');
		expect(h.latest()).toEqual(['1', '2']);
	});

	it('drops a show an earlier page already carried, before it reaches the filter', async () => {
		const h = harness({ pages: { 0: page(['1', '2'], 20), 20: page(['2', '3'], null) } });
		await h.runner.run('q');
		h.runner.loadMore();
		await drain();
		expect(h.filtered).toEqual([['1', '2'], ['3']]);
		expect(h.latest()).toEqual(['1', '2', '3']);
	});

	it('a page that filters to nothing loads the next one by itself', async () => {
		const h = harness({
			pages: { 0: page(['1'], 20), 20: page(['x'], 40), 40: page(['68'], null) },
			unavailable: new Set(['x'])
		});
		await h.runner.run('dragon ball');
		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenCalledTimes(3);
		expect(h.latest()).toEqual(['1', '68']);
		expect(h.phase()).toBe('end');
	});

	it('stops fetching on its own after MAX_EMPTY_PAGES empty pages, and keeps looking when asked', async () => {
		const pages: Record<number, KitsuSearchPage> = { 0: page(['1'], 20) };
		const unavailable = new Set<string>();
		for (let o = 20; o <= 400; o += 20) {
			pages[o] = page([`x${o}`], o + 20);
			unavailable.add(`x${o}`);
		}
		const h = harness({ pages, unavailable });
		await h.runner.run('q');
		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenCalledTimes(1 + MAX_EMPTY_PAGES);
		expect(h.phase()).toBe('held');

		h.runner.loadMore();
		await drain();
		expect(h.searchPage).toHaveBeenCalledTimes(1 + 2 * MAX_EMPTY_PAGES);
		expect(h.phase()).toBe('held');
	});

	it('an empty first page with more behind it keeps looking before settling', async () => {
		const h = harness({
			pages: { 0: page(['x'], 20), 20: page(['2'], null) },
			unavailable: new Set(['x'])
		});
		await h.runner.run('q');
		await drain();
		expect(h.latest()).toEqual(['2']);
		expect(h.phase()).toBe('end');
	});

	it('asking for more while a page is in flight starts no second request', async () => {
		const next = defer<KitsuSearchPage>();
		const searchPage = vi
			.fn()
			.mockResolvedValueOnce(page(['1'], 20))
			.mockReturnValueOnce(next.promise);
		const h = harness({ searchPage });
		await h.runner.run('q');
		h.runner.loadMore();
		h.runner.loadMore();
		await drain();
		expect(searchPage).toHaveBeenCalledTimes(2);
		next.resolve(page(['2'], null));
		await drain();
		expect(h.latest()).toEqual(['1', '2']);
	});

	it('asking for more before any search does nothing', () => {
		const h = harness({ pages: {} });
		h.runner.loadMore();
		expect(h.searchPage).not.toHaveBeenCalled();
	});

	it('a new query silences the previous query’s page in flight and starts over', async () => {
		const stale = defer<KitsuSearchPage>();
		const searchPage = vi.fn(async (q: string, offset: number) => {
			if (q === 'a' && offset === 0) return page(['a1'], 20);
			if (q === 'a') return stale.promise;
			return page(['b1'], 20);
		});
		const h = harness({ searchPage });
		await h.runner.run('a');
		h.runner.loadMore();
		await drain();
		await h.runner.run('b');
		const phasesBefore = h.phases.length;
		const resultsBefore = h.results.length;

		stale.resolve(page(['a2'], null));
		await drain();
		expect(h.results.length).toBe(resultsBefore);
		expect(h.phases.length).toBe(phasesBefore);
		expect(h.latest()).toEqual(['b1']);
		expect(h.phase()).toBe('idle');

		// And b pages from its own start, not from where a stopped.
		h.runner.loadMore();
		await drain();
		expect(searchPage).toHaveBeenLastCalledWith('b', 20);
	});

	it('a stale page failure does not mark the new query failed', async () => {
		const stale = defer<KitsuSearchPage>();
		const searchPage = vi.fn(async (q: string, offset: number) => {
			if (q === 'a' && offset === 20) return stale.promise;
			return page([`${q}1`], 20);
		});
		const h = harness({ searchPage });
		await h.runner.run('a');
		h.runner.loadMore();
		await drain();
		await h.runner.run('b');
		stale.reject(new Error('late'));
		await drain();
		expect(h.phase()).toBe('idle');
	});

	it('a late prune on an earlier page updates that page alone', async () => {
		const emits: ((v: KitsuAnimeRef[]) => void)[] = [];
		const h = harness({
			pages: { 0: page(['1', '2'], 20), 20: page(['3'], null) },
			filter: async (items, _mode, emit) => {
				emits.push(emit);
				emit(items);
			}
		});
		await h.runner.run('q');
		h.runner.loadMore();
		await drain();
		expect(h.latest()).toEqual(['1', '2', '3']);

		emits[0]([ref('1')]);
		expect(h.latest()).toEqual(['1', '3']);
		// A page pruned after its first render does not re-run the
		// phase decision: the search stays where it settled.
		expect(h.phase()).toBe('end');
	});
});
