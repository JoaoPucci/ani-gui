// The runner's side of going back to the search page: where a search
// stands, as the page saves it when the reader leaves, and bringing a
// saved search back — as it was when that is still trustworthy, or by
// asking Kitsu again up to the card the reader left at the top.

import { describe, expect, it, vi } from 'vitest';
import type { Config, KitsuAnimeRef, KitsuSearchPage } from '$lib/api';

const apiMock = vi.hoisted(() => ({
	availabilityBatch: vi.fn(),
	availabilityWarm: vi.fn(),
	checkAvailability: vi.fn(),
	altTitlesFromKitsu: vi.fn(() => []),
	yearFromKitsuRef: vi.fn(() => null)
}));
vi.mock('$lib/api', () => apiMock);

import { createSearchRunner, type SearchRunnerDeps } from './run-search';
import type { PagingPhase } from './paging';
import { captureSearch, FRESH_FOR_MS, type SearchSnapshot } from './snapshot';

function ref(id: string): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Title ${id}`,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

/** Kitsu's pages: twenty ids from each offset, `pages` of them. */
function kitsu(pages: number) {
	return vi.fn(async (_q: string, offset: number): Promise<KitsuSearchPage> => {
		const hits = Array.from({ length: 20 }, (_, i) => ref(String(offset + i + 1)));
		const next = offset + 20 < pages * 20 ? offset + 20 : null;
		return { hits, next_offset: next };
	});
}

async function drain(): Promise<void> {
	for (let i = 0; i < 20; i++) await Promise.resolve();
}

/** The runner's clock: searches start at the time the saved one was
 *  left, unless a scenario moves it. */
let clock = 1_000_000;

function harness(overrides: Partial<SearchRunnerDeps> = {}) {
	const results: KitsuAnimeRef[][] = [];
	const phases: PagingPhase['kind'][] = [];
	const reached: boolean[] = [];
	const deps: SearchRunnerDeps = {
		searchPage: kitsu(5),
		getConfig: vi.fn().mockResolvedValue({ mode: 'sub' } as unknown as Config),
		pickMode: (c) => (c as unknown as { mode: 'sub' | 'dub' } | null)?.mode ?? 'sub',
		filter: vi.fn().mockImplementation(async (items, _mode, emit) => emit(items)),
		onResults: (v) => results.push(v),
		onError: () => {},
		onBusy: () => {},
		onPhase: (p) => phases.push(p.kind),
		onReached: (found) => reached.push(found),
		now: () => clock,
		...overrides
	};
	return { deps, results, phases, reached, last: () => results.at(-1) ?? [] };
}

/** A search for "frieren" that loaded two pages, saved with card `21`
 *  at the top of the screen. */
async function savedSearch(now = 1_000_000): Promise<SearchSnapshot> {
	clock = now;
	const h = harness();
	const runner = createSearchRunner(h.deps);
	await runner.run('frieren', 'year');
	runner.loadMore();
	await drain();
	return captureSearch({
		q: 'frieren',
		order: 'year',
		subtypes: [],
		checkpoint: runner.checkpoint(),
		cards: [{ id: '21', top: -40, bottom: 240 }],
		now
	});
}

describe('checkpoint', () => {
	it('is nothing before a first page has settled', async () => {
		const runner = createSearchRunner(harness().deps);
		expect(runner.checkpoint()).toBeNull();
	});

	it('describes the settled pages: what shows, what was seen, where the next page starts', async () => {
		const h = harness({
			// The availability filter hides every third show.
			filter: vi
				.fn()
				.mockImplementation(async (items: KitsuAnimeRef[], _m, emit) =>
					emit(items.filter((x) => Number(x.id) % 3 !== 0))
				)
		});
		const runner = createSearchRunner(h.deps);
		await runner.run('frieren');
		runner.loadMore();
		await drain();
		const cp = runner.checkpoint()!;
		expect(cp.mode).toBe('sub');
		expect(cp.pages).toBe(2);
		expect(cp.next).toBe(40);
		expect(cp.phase).toBe('idle');
		expect(cp.seen).toHaveLength(40);
		expect(cp.hits.map((x) => x.id)).toEqual(h.last().map((x) => x.id));
		expect(cp.hits.some((x) => x.id === '3')).toBe(false);
	});

	it('leaves out a page still loading', async () => {
		let release: () => void = () => {};
		const searchPage = vi.fn(async (q: string, offset: number) => {
			if (offset === 20) await new Promise<void>((r) => (release = r));
			return kitsu(5)(q, offset);
		});
		const runner = createSearchRunner(harness({ searchPage }).deps);
		await runner.run('frieren');
		runner.loadMore();
		await drain();
		const cp = runner.checkpoint()!;
		expect(cp.pages).toBe(1);
		expect(cp.next).toBe(20);
		expect(cp.hits).toHaveLength(20);
		expect(cp.seen).toHaveLength(20);
		release();
		await drain();
		expect(runner.checkpoint()!.pages).toBe(2);
	});

	it('says the search has ended on its last page', async () => {
		const runner = createSearchRunner(harness({ searchPage: kitsu(1) }).deps);
		await runner.run('frieren');
		expect(runner.checkpoint()!.phase).toBe('end');
	});
});

describe('resume, with results still fresh', () => {
	it('shows them without asking Kitsu, and finds the card the reader left', async () => {
		const saved = await savedSearch();
		const h = harness();
		const runner = createSearchRunner(h.deps);
		await runner.resume(saved, { now: saved.capturedAt + 60_000, reload: false });

		expect(h.deps.searchPage).not.toHaveBeenCalled();
		expect(h.last().map((x) => x.id)).toEqual(saved.results!.hits.map((x) => x.id));
		expect(h.phases.at(-1)).toBe('idle');
		expect(h.reached).toEqual([true]);
	});

	it('goes on from the page after the saved ones, skipping shows already seen', async () => {
		const saved = await savedSearch();
		const searchPage = vi.fn(async () => ({
			hits: [ref('40'), ref('41')],
			next_offset: null
		}));
		const h = harness({ searchPage });
		const runner = createSearchRunner(h.deps);
		await runner.resume(saved, { now: saved.capturedAt, reload: false });
		runner.loadMore();
		await drain();

		expect(searchPage).toHaveBeenCalledWith('frieren', 40, 'year');
		expect(
			h
				.last()
				.map((x) => x.id)
				.slice(-2)
		).toEqual(['40', '41']);
		expect(h.last().filter((x) => x.id === '40')).toHaveLength(1);
		expect(runner.checkpoint()!.pages).toBe(3);
	});

	it('answers not found when no card was on screen', async () => {
		const saved = { ...(await savedSearch()), anchor: null };
		const h = harness();
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: false });
		expect(h.reached).toEqual([false]);
	});
});

describe('resume, asking again', () => {
	it('runs the saved search again once its results are an hour old, up to the card', async () => {
		const saved = await savedSearch();
		const h = harness();
		const runner = createSearchRunner(h.deps);
		await runner.resume(saved, { now: saved.capturedAt + FRESH_FOR_MS, reload: false });
		await drain();

		expect(vi.mocked(h.deps.searchPage).mock.calls).toEqual([
			['frieren', 0, 'year'],
			['frieren', 20, 'year']
		]);
		expect(h.last().some((x) => x.id === '21')).toBe(true);
		expect(h.reached).toEqual([true]);
		expect(h.phases.at(-1)).toBe('idle');
	});

	it('runs it again on a reload, however fresh', async () => {
		const saved = await savedSearch();
		const h = harness();
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: true });
		await drain();
		expect(h.deps.searchPage).toHaveBeenCalledWith('frieren', 0, 'year');
		expect(h.reached).toEqual([true]);
	});

	it('runs it again when the audio setting changed since', async () => {
		const saved = await savedSearch();
		const h = harness({
			getConfig: vi.fn().mockResolvedValue({ mode: 'dub' } as unknown as Config)
		});
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: false });
		await drain();
		expect(h.deps.searchPage).toHaveBeenCalledWith('frieren', 0, 'year');
	});

	it('stops at the pages the reader had when the card no longer shows', async () => {
		const saved = await savedSearch();
		const h = harness({
			filter: vi
				.fn()
				.mockImplementation(async (items: KitsuAnimeRef[], _m, emit) =>
					emit(items.filter((x) => x.id !== '21'))
				)
		});
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: true });
		await drain();
		expect(h.deps.searchPage).toHaveBeenCalledTimes(2);
		expect(h.reached).toEqual([false]);
		expect(h.phases.at(-1)).toBe('idle');
	});

	it('gives up on the card when a page fails, leaving the failure to retry', async () => {
		const saved = await savedSearch();
		const searchPage = vi.fn(async (q: string, offset: number) => {
			if (offset === 20) throw new Error('kitsu down');
			return kitsu(5)(q, offset);
		});
		const h = harness({ searchPage });
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: true });
		await drain();
		expect(h.reached).toEqual([false]);
		expect(h.phases.at(-1)).toBe('failed');
	});

	it('answers right away without a card to aim for', async () => {
		const saved = { ...(await savedSearch()), anchor: null };
		const h = harness();
		await createSearchRunner(h.deps).resume(saved, { now: saved.capturedAt, reload: true });
		await drain();
		expect(h.deps.searchPage).toHaveBeenCalledTimes(1);
		expect(h.reached).toEqual([false]);
	});
});

describe('resume against other runs', () => {
	it('replaces the run the page started for its address, which then sends nothing', async () => {
		const saved = await savedSearch();
		let answer: (c: Config) => void = () => {};
		const h = harness({
			getConfig: vi.fn(() => new Promise<Config>((r) => (answer = r)))
		});
		const runner = createSearchRunner(h.deps);
		const run = runner.run('frieren');
		const resumed = runner.resume(saved, { now: saved.capturedAt, reload: false });
		answer({ mode: 'sub' } as unknown as Config);
		await run;
		await resumed;
		expect(h.deps.searchPage).not.toHaveBeenCalled();
		expect(h.last()).toHaveLength(saved.results!.hits.length);
	});

	it('is itself replaced by a search started while the settings load', async () => {
		const saved = await savedSearch();
		let answer: (c: Config) => void = () => {};
		const h = harness({
			getConfig: vi.fn(() => new Promise<Config>((r) => (answer = r)))
		});
		const runner = createSearchRunner(h.deps);
		const resumed = runner.resume(saved, { now: saved.capturedAt, reload: false });
		const run = runner.run('himmel');
		answer({ mode: 'sub' } as unknown as Config);
		await resumed;
		await run;
		expect(h.reached).toEqual([]);
		expect(vi.mocked(h.deps.searchPage).mock.calls).toEqual([['himmel', 0, 'relevance']]);
	});
});

describe('the age of saved results', () => {
	it('is when the search fetched its first page', async () => {
		clock = 5_000;
		const runner = createSearchRunner(harness().deps);
		await runner.run('frieren');
		clock = 9_000;
		runner.loadMore();
		await drain();
		expect(runner.checkpoint()!.fetchedAt).toBe(5_000);
	});

	it('stays the saved one when the results come back as they were', async () => {
		const saved = await savedSearch();
		const runner = createSearchRunner(harness().deps);
		clock = saved.capturedAt + 50 * 60_000;
		await runner.resume(saved, { now: clock, reload: false });
		runner.loadMore();
		await drain();
		expect(runner.checkpoint()!.fetchedAt).toBe(saved.results!.fetchedAt);
	});

	it('is not renewed by going back and forth: past an hour the search is asked again', async () => {
		const first = await savedSearch();
		const h = harness();
		const runner = createSearchRunner(h.deps);
		clock = first.capturedAt + 50 * 60_000;
		await runner.resume(first, { now: clock, reload: false });
		expect(h.deps.searchPage).not.toHaveBeenCalled();
		const again = captureSearch({
			q: 'frieren',
			order: 'year',
			subtypes: [],
			checkpoint: runner.checkpoint(),
			cards: [{ id: '21', top: -40, bottom: 240 }],
			now: clock
		});
		clock = first.capturedAt + 70 * 60_000;
		await createSearchRunner(h.deps).resume(again, { now: clock, reload: false });
		await drain();
		expect(h.deps.searchPage).toHaveBeenCalledWith('frieren', 0, 'year');
	});
});
