import { describe, expect, it, vi } from 'vitest';
import { makeStartResume, type StartResumeDeps } from './start-resume';
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

interface Harness {
	deps: StartResumeDeps;
	log: string[];
	busyLog: (string | null)[];
	counts: [string, number][];
	failures: [string, unknown][];
}

function makeHarness(overrides: Partial<StartResumeDeps> = {}): Harness {
	const log: string[] = [];
	const busyLog: (string | null)[] = [];
	const counts: [string, number][] = [];
	const failures: [string, unknown][] = [];
	const deps: StartResumeDeps = {
		isBusy: () => busyLog.at(-1) != null,
		onBusy: (id) => {
			busyLog.push(id);
			log.push(`busy:${id}`);
		},
		onProgress: () => {},
		onFailure: (title, e) => failures.push([title, e]),
		getSettings: vi.fn().mockImplementation(async () => {
			log.push('settings');
			return { mode: 'sub' as const, quality: 'best' };
		}),
		getPlayableCount: () => null,
		setPlayableCount: (id, c) => counts.push([id, c]),
		fetchInteractiveCount: vi.fn().mockImplementation(async () => {
			log.push('interactive-count');
			return { count: 12, approximate: false };
		}),
		resolvePlay: vi.fn().mockImplementation(async () => {
			log.push('resolve-play');
			return { session_id: 's1' };
		}),
		markWatched: vi.fn().mockResolvedValue(undefined),
		syncTrackers: vi.fn().mockResolvedValue(undefined),
		navigateToSession: vi.fn().mockImplementation(() => log.push('nav-session')),
		...overrides
	};
	return { deps, log, busyLog, counts, failures };
}

describe('makeStartResume (click orchestration)', () => {
	it('sequences busy -> settings -> interactive count -> play -> navigate', async () => {
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '12', 'Show'), makeMatch('k1', 24), 24, false);

		expect(h.log).toEqual([
			'busy:k1',
			'settings',
			'interactive-count',
			'resolve-play',
			'nav-session'
		]);
		// Replay at the live cap: watched 12, live count 12 → episode 12.
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 12, mode: 'sub', quality: 'best' }),
			expect.any(Function)
		);
		expect(h.counts).toEqual([['h1', 12]]);
		// Busy stays set on success — navigation unmounts the page.
		expect(h.busyLog).toEqual(['k1']);
	});

	it('ignores a click while another resume is busy', async () => {
		const h = makeHarness({ isBusy: () => true });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.log).toEqual([]);
		expect(h.deps.resolvePlay).not.toHaveBeenCalled();
	});

	it('skips the interactive lookup when the probed cap is already in', async () => {
		const h = makeHarness({ getPlayableCount: () => 12 });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 24), 24, false);
		expect(h.deps.fetchInteractiveCount).not.toHaveBeenCalled();
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it('always resolves the episode: there is no player left behind to take back', async () => {
		// Leaving the play page unloads its video, so a click on the
		// episode just left resolves it like any other and the page
		// resumes from where it was left.
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledTimes(1);
		expect(h.deps.navigateToSession).toHaveBeenCalledTimes(1);
	});

	it('fires markWatched and tracker sync on success', async () => {
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.markWatched).toHaveBeenCalledWith(expect.objectContaining({ episode: 6 }));
		expect(h.deps.syncTrackers).toHaveBeenCalledWith('k1', 6, 12, true);
	});

	it('a failed play resolution clears busy and reports the failure', async () => {
		const h = makeHarness({
			resolvePlay: vi.fn().mockRejectedValue(new Error('no sources'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.busyLog).toEqual(['k1', null]);
		expect(h.failures).toEqual([['Title k1', expect.any(Error)]]);
		expect(h.deps.navigateToSession).not.toHaveBeenCalled();
	});

	it('a countless interactive lookup writes no cap and falls back to the Kitsu count', async () => {
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockRejectedValue(new Error('down'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 24), 24, false);
		expect(h.counts).toEqual([]);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it('does nothing for a match without a canonical title', async () => {
		const match = makeMatch('k1', 12);
		(match as { canonical_title: string | null }).canonical_title = null;
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), match, 12, true);
		expect(h.log).toEqual([]);
	});
});

describe('makeStartResume — progress and best-effort fan-out', () => {
	it('forwards resolution progress labels to the page', async () => {
		const labels: (string | null)[] = [];
		const h = makeHarness({
			onProgress: (l) => labels.push(l),
			resolvePlay: vi.fn().mockImplementation(async (_args, onProgress) => {
				onProgress('searching…');
				onProgress('provider ✓');
				return { session_id: 's1' };
			})
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		// null on entry (clearing the previous run), then each label.
		expect(labels).toEqual([null, 'searching…', 'provider ✓']);
	});

	it('still navigates when the watched-history write fails', async () => {
		// markWatched is best-effort: the episode is already resolved
		// and the user is owed the player regardless.
		const h = makeHarness({
			markWatched: vi.fn().mockRejectedValue(new Error('hsts locked'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		await Promise.resolve();
		expect(h.deps.navigateToSession).toHaveBeenCalled();
		expect(h.failures).toEqual([]);
	});

	it('still navigates when the tracker sync fails', async () => {
		const h = makeHarness({
			syncTrackers: vi.fn().mockRejectedValue(new Error('anilist 503'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		await Promise.resolve();
		expect(h.deps.navigateToSession).toHaveBeenCalled();
		expect(h.failures).toEqual([]);
	});
});

describe('makeStartResume — an episode left part-way', () => {
	it('goes back to the last watched episode when it was left part-way', async () => {
		const leftPartWay = vi.fn((kitsuId: string, ep: number) => kitsuId === 'k1' && ep === 5);
		const h = makeHarness({ getPlayableCount: () => 12, leftPartWay });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 5 }),
			expect.any(Function)
		);
		expect(h.deps.navigateToSession).toHaveBeenCalledWith('k1', { session_id: 's1' }, 5);
	});

	it('goes on to the next episode when only an older one was left part-way', async () => {
		const h = makeHarness({
			getPlayableCount: () => 12,
			leftPartWay: (_k, ep) => ep === 4
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it("goes back to a series' last episode left part-way", async () => {
		const h = makeHarness({ getPlayableCount: () => 12, leftPartWay: () => true });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '12', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 12 }),
			expect.any(Function)
		);
	});
});
