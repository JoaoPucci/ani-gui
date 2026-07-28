import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Config, KitsuAnimeRef } from '$lib/api';

const apiMock = vi.hoisted(() => ({
	availabilityBatch: vi.fn(),
	availabilityWarm: vi.fn(),
	checkAvailability: vi.fn(),
	altTitlesFromKitsu: vi.fn(() => []),
	yearFromKitsuRef: vi.fn(() => null)
}));
vi.mock('$lib/api', () => apiMock);

import { createSearchRunner, type SearchRunnerDeps } from './run-search';
import { filterAvailableProgressive } from '$lib/availability/progressive';

function ref(id: string, overrides: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
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
		cover_image: null,
		...overrides
	};
}

function defer<T>(): {
	promise: Promise<T>;
	resolve: (v: T) => void;
	reject: (e: unknown) => void;
} {
	let resolveFn!: (v: T) => void;
	let rejectFn!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolveFn = res;
		rejectFn = rej;
	});
	return { promise, resolve: resolveFn, reject: rejectFn };
}

interface Harness {
	deps: SearchRunnerDeps;
	results: KitsuAnimeRef[][];
	errors: unknown[];
	busyLog: boolean[];
}

function makeHarness(overrides: Partial<SearchRunnerDeps> = {}): Harness {
	const results: KitsuAnimeRef[][] = [];
	const errors: unknown[] = [];
	const busyLog: boolean[] = [];
	const deps: SearchRunnerDeps = {
		kitsuSearch: vi.fn().mockResolvedValue([]),
		getConfig: vi.fn().mockResolvedValue(null),
		pickMode: () => 'sub',
		filter: vi.fn().mockImplementation(async (items, _mode, emit) => emit(items)),
		onResults: (v) => results.push(v),
		onError: (e) => errors.push(e),
		onBusy: (b) => busyLog.push(b),
		...overrides
	};
	return { deps, results, errors, busyLog };
}

describe('createSearchRunner (generation guard)', () => {
	it('drops emissions from a superseded run of the SAME query (A → B → A)', async () => {
		// The page's previous guard compared query text, so navigating
		// A → B → A made the FIRST A-run's late emissions look current
		// again: they overwrote the new A-run's results and cleared its
		// spinner early. A generation token distinguishes the two runs
		// of the same text.
		const emits: ((visible: KitsuAnimeRef[]) => void)[] = [];
		const h = makeHarness({
			filter: vi.fn().mockImplementation((_items, _mode, emit) => {
				emits.push(emit);
				return new Promise<void>(() => {}); // probes never settle
			})
		});
		const runner = createSearchRunner(h.deps);

		void runner.run('a');
		await Promise.resolve();
		await Promise.resolve();
		void runner.run('b');
		await Promise.resolve();
		await Promise.resolve();
		void runner.run('a');
		await Promise.resolve();
		await Promise.resolve();
		expect(emits).toHaveLength(3);

		const stale = ref('stale');
		const fresh = ref('fresh');
		emits[0]([stale]); // first A-run — superseded twice over
		expect(h.results).toEqual([]);
		emits[2]([fresh]); // current A-run
		expect(h.results).toEqual([[fresh]]);
	});

	it('a superseded run cannot clear the current run busy state', async () => {
		const emits: ((visible: KitsuAnimeRef[]) => void)[] = [];
		const h = makeHarness({
			filter: vi.fn().mockImplementation((_items, _mode, emit) => {
				emits.push(emit);
				return new Promise<void>(() => {});
			})
		});
		const runner = createSearchRunner(h.deps);

		void runner.run('a');
		await Promise.resolve();
		await Promise.resolve();
		void runner.run('a');
		await Promise.resolve();
		await Promise.resolve();

		// Two runs started: busy went true twice, never false yet.
		expect(h.busyLog).toEqual([true, true]);
		emits[0]([ref('stale')]);
		// The stale run must not flip busy off while run 2 still loads.
		expect(h.busyLog).toEqual([true, true]);
		emits[1]([ref('fresh')]);
		expect(h.busyLog).toEqual([true, true, false]);
	});

	it('a superseded run rejection does not surface as the current run error', async () => {
		const firstSearch = defer<KitsuAnimeRef[]>();
		const kitsuSearch = vi
			.fn()
			.mockReturnValueOnce(firstSearch.promise)
			.mockResolvedValueOnce([ref('ok')]);
		const h = makeHarness({ kitsuSearch });
		const runner = createSearchRunner(h.deps);

		const run1 = runner.run('a');
		const run2 = runner.run('b');
		firstSearch.reject(new Error('kitsu down'));
		await run1;
		await run2;

		expect(h.errors).toEqual([]);
		expect(h.results).toEqual([[ref('ok')]]);
	});

	it('caches config only after a successful load, reused across runs', async () => {
		const getConfig = vi.fn().mockResolvedValue({ mode: 'dub' } as unknown as Config);
		const modes: string[] = [];
		const h = makeHarness({
			getConfig,
			pickMode: (c) => ((c as unknown as { mode: 'dub' })?.mode === 'dub' ? 'dub' : 'sub'),
			filter: vi.fn().mockImplementation(async (items, mode, emit) => {
				modes.push(mode);
				emit(items);
			})
		});
		const runner = createSearchRunner(h.deps);
		await runner.run('a');
		await runner.run('b');
		expect(getConfig).toHaveBeenCalledTimes(1);
		expect(modes).toEqual(['dub', 'dub']);
	});

	it('a getConfig rejection falls back to null for THIS run and retries on the next', async () => {
		// Codex P2 #3664593085 — a deeplink's first run can race the
		// mount's settings load and lose. Permanently caching that
		// failure pins a DUB user to the 'sub' fallback for the whole
		// mount; a later run must ask again and pick up the config
		// that has since loaded.
		const getConfig = vi
			.fn()
			.mockRejectedValueOnce(new Error('settings down'))
			.mockResolvedValueOnce({ mode: 'dub' } as unknown as Config);
		const modes: string[] = [];
		const h = makeHarness({
			getConfig,
			pickMode: (c) => ((c as unknown as { mode: 'dub' })?.mode === 'dub' ? 'dub' : 'sub'),
			kitsuSearch: vi.fn().mockResolvedValue([ref('ok')]),
			filter: vi.fn().mockImplementation(async (items, mode, emit) => {
				modes.push(mode);
				emit(items);
			})
		});
		const runner = createSearchRunner(h.deps);
		await runner.run('a');
		expect(h.errors).toEqual([]);
		await runner.run('b');
		expect(getConfig).toHaveBeenCalledTimes(2);
		expect(modes).toEqual(['sub', 'dub']);
	});

	it('surfaces the current run error and ends busy', async () => {
		const h = makeHarness({
			kitsuSearch: vi.fn().mockRejectedValue(new Error('kitsu down'))
		});
		const runner = createSearchRunner(h.deps);
		await runner.run('a');
		expect(h.errors).toHaveLength(1);
		expect(h.busyLog).toEqual([true, false]);
	});
});

describe('acceptance: progressive search rendering (runner + real filter)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.checkAvailability.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	function runnerWithRealFilter(hits: KitsuAnimeRef[]) {
		const h = makeHarness({
			kitsuSearch: vi.fn().mockResolvedValue(hits),
			filter: (items, mode, emit) => filterAvailableProgressive(items, mode, emit)
		});
		return { ...h, runner: createSearchRunner(h.deps) };
	}

	it('the grid appears at the grace deadline while probes are still pending', async () => {
		// The user-visible core of the change: a cold query whose
		// availability probes hang must still show its results after
		// the grace, with the spinner gone.
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockReturnValue(new Promise(() => {}));
		const hits = [ref('a'), ref('b', { status: 'finished' }), ref('c')];
		const { runner, results, busyLog } = runnerWithRealFilter(hits);

		void runner.run('bleach');
		await vi.advanceTimersByTimeAsync(1999);
		expect(results).toEqual([]);
		await vi.advanceTimersByTimeAsync(1);
		expect(results).toEqual([hits]);
		expect(busyLog).toEqual([true, false]);
	});

	it('a late negative verdict prunes its card from the rendered grid', async () => {
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		const probes = new Map<string, (r: { available: boolean }) => void>();
		apiMock.checkAvailability.mockImplementation(
			(req: { kitsu_id: string }) =>
				new Promise<{ available: boolean }>((res) => probes.set(req.kitsu_id, res))
		);
		const keep = ref('keep');
		const gone = ref('gone', { status: 'finished' });
		const { runner, results } = runnerWithRealFilter([keep, gone]);

		void runner.run('bleach');
		await vi.advanceTimersByTimeAsync(2000);
		expect(results).toEqual([[keep, gone]]);

		probes.get('gone')!({ available: false });
		await vi.advanceTimersByTimeAsync(0);
		expect(results).toEqual([[keep, gone], [keep]]);
	});
});
