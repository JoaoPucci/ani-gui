import { describe, expect, it, vi } from 'vitest';
import { loadContinueWatchingState } from './continue-watching-loader';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

function makeEntry(id: string, ep: string, title: string): HistoryEntry {
	return { id, ep_no: ep, title };
}

function makeMatch(id: string, episodeCount: number | null): KitsuAnimeRef {
	return {
		id,
		slug: `slug-${id}`,
		canonical_title: `Title ${id}`,
		titles: {},
		episode_count: episodeCount,
		subtype: 'TV',
		status: 'current',
		poster_image: null,
		start_date: null
	} as unknown as KitsuAnimeRef;
}

// Defer helper — returns a promise and its resolver so the test
// can sequence resolve order across stages.
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

const subMode = () => Promise.resolve<'sub' | 'dub'>('sub');

describe('loadContinueWatchingState', () => {
	it('emits onRowReady per row as each row resolves, independent of other rows', async () => {
		// Codex P2 #3348386932 — per-card releases: a slow row must
		// not gate a fast row. Before this fix the loader awaited
		// Promise.all on every match before firing the batch, so any
		// one slow Kitsu resolution held every card in its
		// search-link fallback state. The per-row callback releases
		// each card as ITS match + count both land — slow rows fall
		// behind without dragging fast rows with them.
		const fast = makeEntry('hist-fast', '5', 'Fast');
		const slow = makeEntry('hist-slow', '10', 'Slow');
		const mFast = makeMatch('k-fast', 12);
		const mSlow = makeMatch('k-slow', 24);

		const slowMatch = defer<KitsuAnimeRef | null>();
		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-fast') return Promise.resolve(mFast);
			return slowMatch.promise;
		});
		const fetchAvailability = vi.fn().mockImplementation((match: KitsuAnimeRef) => {
			if (match.id === 'k-fast') return Promise.resolve({ episode_count: 12 });
			return Promise.resolve({ episode_count: 25 });
		});

		const ready: { id: string; count: number | null }[] = [];
		const onRowReady = vi.fn().mockImplementation((id: string, _m, count: number | null) => {
			ready.push({ id, count });
		});

		const loaderPromise = loadContinueWatchingState([fast, slow], {
			resolveMatch,
			fetchAvailability,
			getMode: subMode,
			onRowReady
		});

		// Let microtasks settle. Fast row's match has resolved + its
		// per-row probe should also have run; slow row is still
		// pending. The fast row MUST have been released.
		for (let i = 0; i < 20; i++) await Promise.resolve();
		expect(ready.find((r) => r.id === 'hist-fast')?.count).toBe(12);
		expect(ready.find((r) => r.id === 'hist-slow')).toBeUndefined();

		// Now let the slow row through; it releases independently.
		slowMatch.resolve(mSlow);
		const result = await loaderPromise;
		expect(ready.find((r) => r.id === 'hist-slow')?.count).toBe(25);
		expect(result.matches).toEqual({ 'hist-fast': mFast, 'hist-slow': mSlow });
		expect(result.playableCounts).toEqual({ 'hist-fast': 12, 'hist-slow': 25 });
	});

	it('releases no-match rows immediately with onRowReady(null, null)', async () => {
		// An entry whose Kitsu resolution returns null can't be a
		// resumable card — it just renders as the /search fallback
		// link. No need to wait for anything else; release it ASAP.
		const orphan = makeEntry('hist-orphan', '1', 'Unmatchable');
		const other = makeEntry('hist-other', '5', 'Other');
		const mOther = makeMatch('k-other', 12);
		const otherMatch = defer<KitsuAnimeRef | null>();

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-orphan') return Promise.resolve(null);
			return otherMatch.promise;
		});
		const fetchAvailability = vi.fn().mockResolvedValue({ episode_count: 12 });

		const ready: { id: string; match: KitsuAnimeRef | null; count: number | null }[] = [];
		const onRowReady = (id: string, match: KitsuAnimeRef | null, count: number | null) => {
			ready.push({ id, match, count });
		};

		const loaderPromise = loadContinueWatchingState([orphan, other], {
			resolveMatch,
			fetchAvailability,
			getMode: subMode,
			onRowReady
		});

		for (let i = 0; i < 5; i++) await Promise.resolve();
		// Orphan released immediately; other still pending its match.
		expect(ready.find((r) => r.id === 'hist-orphan')).toEqual({
			id: 'hist-orphan',
			match: null,
			count: null
		});
		expect(ready.find((r) => r.id === 'hist-other')).toBeUndefined();

		otherMatch.resolve(mOther);
		await loaderPromise;
	});

	it('caps concurrent probes at the configured pool size', async () => {
		// Codex P2 #3348430790 — bounded probe concurrency: a sizable
		// CLI-imported history with many cache-miss rows shouldn't
		// fire N concurrent allmanga probes. The backend's `warm`
		// path spaces equivalent probes by 500ms; filterAvailableStrict
		// caps inline probes too. Mirror the same default (4) here.
		const entries = Array.from({ length: 8 }, (_, i) => makeEntry(`h${i}`, '1', `Show ${i}`));
		const matches = entries.map((e, i) => makeMatch(`k${i}`, 12));

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			const i = entries.findIndex((e) => e.id === entry.id);
			return Promise.resolve(matches[i]);
		});

		let inFlight = 0;
		let maxInFlight = 0;
		const probeDeferreds = matches.map(() => defer<{ episode_count: number }>());
		const fetchAvailability = vi.fn().mockImplementation((match: KitsuAnimeRef) => {
			inFlight++;
			maxInFlight = Math.max(maxInFlight, inFlight);
			const i = matches.findIndex((m) => m.id === match.id);
			return probeDeferreds[i].promise.finally(() => {
				inFlight--;
			});
		});

		const loaderPromise = loadContinueWatchingState(entries, {
			resolveMatch,
			fetchAvailability,
			getMode: subMode,
			probeConcurrency: 4
		});

		// Let matches + initial probe dispatch settle. At most 4 probes
		// should be in flight; the rest queue.
		for (let i = 0; i < 20; i++) await Promise.resolve();
		expect(inFlight).toBeLessThanOrEqual(4);
		expect(maxInFlight).toBe(4);

		// Drain the queue.
		for (const d of probeDeferreds) d.resolve({ episode_count: 13 });
		await loaderPromise;
		expect(maxInFlight).toBe(4);
		expect(fetchAvailability).toHaveBeenCalledTimes(8);
	});

	it('defaults to sub when getMode rejects', async () => {
		// getMode reads through settingsGet → pickAvailabilityMode at
		// the page level. Settings shouldn't reject under normal
		// operation, but a corrupt config file or filesystem error
		// could surface as a rejection. Match the page's pre-loader
		// fallback ('sub') so probes still fire with a useful mode
		// rather than throwing out of the entire load.
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const m1 = makeMatch('k-a', 12);

		const resolveMatch = vi.fn().mockResolvedValue(m1);
		const fetchAvailability = vi.fn().mockResolvedValue({ episode_count: 12 });

		const result = await loadContinueWatchingState([e1], {
			resolveMatch,
			fetchAvailability,
			getMode: () => Promise.reject(new Error('config read failed'))
		});

		expect(fetchAvailability).toHaveBeenCalledWith(m1, 'sub');
		expect(result.matches).toEqual({ 'hist-a': m1 });
		expect(result.playableCounts).toEqual({ 'hist-a': 12 });
	});

	it('treats a per-entry resolveMatch rejection as null match (no throw)', async () => {
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const e2 = makeEntry('hist-b', '2', 'Show B');
		const m2 = makeMatch('k-b', 24);

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-a') return Promise.reject(new Error('boom'));
			return Promise.resolve(m2);
		});
		const fetchAvailability = vi.fn().mockResolvedValue({ episode_count: 24 });

		const result = await loadContinueWatchingState([e1, e2], {
			resolveMatch,
			fetchAvailability,
			getMode: subMode
		});

		expect(result.matches).toEqual({ 'hist-a': null, 'hist-b': m2 });
		expect(result.playableCounts).toEqual({ 'hist-b': 24 });
	});

	it('omits the playable count for matches whose probe returns null or rejects', async () => {
		// Probe failure / unavailable response: per-card cap falls back
		// to match.episode_count (Kitsu's announced cap). Same shape
		// the original (no-probe) flow had — just no longer racy on
		// the batch-only side.
		const e1 = makeEntry('hist-rejects', '5', 'Probe Fails');
		const e2 = makeEntry('hist-unavailable', '5', 'Returns Null');
		const m1 = makeMatch('k-rej', 12);
		const m2 = makeMatch('k-null', 12);

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-rejects') return Promise.resolve(m1);
			return Promise.resolve(m2);
		});
		const fetchAvailability = vi.fn().mockImplementation((match: KitsuAnimeRef) => {
			if (match.id === 'k-rej') return Promise.reject(new Error('network'));
			return Promise.resolve(null);
		});

		const result = await loadContinueWatchingState([e1, e2], {
			resolveMatch,
			fetchAvailability,
			getMode: subMode
		});

		expect(result.matches).toEqual({ 'hist-rejects': m1, 'hist-unavailable': m2 });
		expect(result.playableCounts).toEqual({});
	});
});
