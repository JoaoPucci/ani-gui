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
		settingsLoaded: () => true,
		getPlayableCount: () => null,
		setPlayableCount: (id, c) => counts.push([id, c]),
		fetchInteractiveCount: vi.fn().mockImplementation(async () => {
			log.push('interactive-count');
			return { count: 12, approximate: false };
		}),
		reuseSession: vi.fn().mockReturnValue(null),
		resolvePlay: vi.fn().mockImplementation(async () => {
			log.push('resolve-play');
			return { session_id: 's1' };
		}),
		markWatched: vi.fn().mockResolvedValue(undefined),
		syncTrackers: vi.fn().mockResolvedValue(undefined),
		navigateToCached: vi.fn().mockImplementation(() => log.push('nav-cached')),
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

	it('takes the cached-session shortcut without resolving a new play', async () => {
		const cached = { session_id: 'pip', episode: 6, media_kind: 'Hls' };
		const h = makeHarness({ reuseSession: vi.fn().mockReturnValue(cached) });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.navigateToCached).toHaveBeenCalledWith('k1', cached);
		expect(h.deps.resolvePlay).not.toHaveBeenCalled();
		expect(h.deps.markWatched).not.toHaveBeenCalled();
	});

	it('passes undefined quality/mode to session reuse when settings never loaded', async () => {
		const reuseSession = vi.fn().mockReturnValue(null);
		const h = makeHarness({ settingsLoaded: () => false, reuseSession });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(reuseSession).toHaveBeenCalledWith('k1', expect.any(Number), undefined, undefined);
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
				onProgress('allanime ✓');
				return { session_id: 's1' };
			})
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		// null on entry (clearing the previous run), then each label.
		expect(labels).toEqual([null, 'searching…', 'allanime ✓']);
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
