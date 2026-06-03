import { describe, expect, it, vi } from 'vitest';
import { makeContinueRowReadyHandler } from './row-ready';
import type { HistoryEntry, KitsuAnimeRef, KitsuEpisode } from '$lib/api';

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

function makeKitsuEpisode(number: number, relative = number): KitsuEpisode {
	return {
		id: `ep-${number}`,
		number,
		relative_number: relative,
		canonical_title: `Episode ${number}`,
		thumbnail: null
	} as unknown as KitsuEpisode;
}

interface Spy {
	deps: Parameters<typeof makeContinueRowReadyHandler>[0];
	calls: {
		setMatch: [string, KitsuAnimeRef | null][];
		setPlayableCount: [string, number][];
		setEpisode: [string, KitsuEpisode | null][];
	};
	fetchKitsuEpisodes: ReturnType<typeof vi.fn>;
}

function makeSpy(
	history: HistoryEntry[],
	fetchImpl?: (kitsuId: string, page: number) => Promise<KitsuEpisode[]>
): Spy {
	const historyById = new Map(history.map((h) => [h.id, h]));
	const fetchKitsuEpisodes = vi.fn(fetchImpl ?? (() => Promise.resolve([])));
	const calls = {
		setMatch: [] as [string, KitsuAnimeRef | null][],
		setPlayableCount: [] as [string, number][],
		setEpisode: [] as [string, KitsuEpisode | null][]
	};
	return {
		deps: {
			historyById,
			fetchKitsuEpisodes,
			setMatch: (id, m) => calls.setMatch.push([id, m]),
			setPlayableCount: (id, c) => calls.setPlayableCount.push([id, c]),
			setEpisode: (id, ep) => calls.setEpisode.push([id, ep])
		},
		calls,
		fetchKitsuEpisodes
	};
}

describe('makeContinueRowReadyHandler', () => {
	it('null match: only setMatch is called, no episode fetch', async () => {
		// Codex P1 #3349155760 motivation — the extracted handler must
		// short-circuit for orphan rows. The page renders these as
		// /search-fallback links; there's no kitsu_id to fetch episodes
		// for and no count to surface.
		const entry = makeEntry('hist-a', '5', 'Show A');
		const spy = makeSpy([entry]);
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', null, null);
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-a', null]]);
		expect(spy.calls.setPlayableCount).toEqual([]);
		expect(spy.calls.setEpisode).toEqual([]);
		expect(spy.fetchKitsuEpisodes).not.toHaveBeenCalled();
	});

	it('match + count: surfaces both, fetches kitsu episodes for the next episode page', async () => {
		const entry = makeEntry('hist-a', '5', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(6)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-a', match]]);
		expect(spy.calls.setPlayableCount).toEqual([['hist-a', 12]]);
		// last_watched=5, cap=12 → pickNextEpisode = 6. Page for ep 6
		// is Math.ceil(6 / EPISODES_KITSU_PAGE_SIZE) = 1.
		expect(spy.fetchKitsuEpisodes).toHaveBeenCalledWith('k-a', 1);
		expect(spy.calls.setEpisode).toEqual([['hist-a', expect.objectContaining({ number: 6 })]]);
	});

	it('match with null playableCount: omits setPlayableCount but still fetches episodes using match.episode_count', async () => {
		// Cache-miss row whose live probe didn't return a count. The
		// cap falls back to match.episode_count (Kitsu's announced
		// total) for the episode-fetch decision, mirroring the
		// template's `playableCount ?? match?.episode_count` cap.
		const entry = makeEntry('hist-a', '5', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(6)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, null);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-a', match]]);
		expect(spy.calls.setPlayableCount).toEqual([]);
		expect(spy.fetchKitsuEpisodes).toHaveBeenCalledWith('k-a', 1);
	});

	it('entry not in historyById: setMatch fires but no episode fetch (stale callback)', async () => {
		// onRowReady can fire after the page has navigated away or
		// the history map has rotated. setMatch is still safe to call
		// (it just writes a stale entry into the map which the page
		// no longer reads); skipping the episode fetch avoids a wasted
		// IPC.
		const spy = makeSpy([]);
		const handle = makeContinueRowReadyHandler(spy.deps);
		const match = makeMatch('k-orphan', 12);

		handle('hist-missing', match, 12);
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-missing', match]]);
		expect(spy.calls.setPlayableCount).toEqual([['hist-missing', 12]]);
		expect(spy.fetchKitsuEpisodes).not.toHaveBeenCalled();
	});

	it('falls back to relative_number when the episode list has no number match', async () => {
		// Kitsu's episodes endpoint sometimes returns a list whose
		// `number` field is absolute across the parent show while
		// `relative_number` is the per-cour index. The page's render
		// rule and the original inline handler both use the relative
		// fallback as a last resort.
		const entry = makeEntry('hist-a', '5', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () =>
			Promise.resolve([
				{
					id: 'wrong-1',
					number: 26,
					relative_number: 6,
					canonical_title: 'rel-6',
					thumbnail: null
				} as unknown as KitsuEpisode
			])
		);
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setEpisode).toEqual([
			['hist-a', expect.objectContaining({ relative_number: 6 })]
		]);
	});

	it('falls through to null when the episode list has no matching number', async () => {
		const entry = makeEntry('hist-a', '5', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(99, 99)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setEpisode).toEqual([['hist-a', null]]);
	});

	it('fetch rejection: sets episode null (the row degrades to no thumbnail rather than throwing)', async () => {
		const entry = makeEntry('hist-a', '5', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.reject(new Error('network')));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setEpisode).toEqual([['hist-a', null]]);
	});

	it('unparseable ep_no still fires the fetch using displayEpisode=1 + the cap', async () => {
		// resolveHistoryEntry falls back to displayEpisode=1 on a
		// parse failure (parseInt('NaN', 10) || 1 → 1), so the row is
		// still resumable. pickNextEpisode(1, 12) → 2; the handler
		// fetches the kitsu page for ep 2.
		const entry = makeEntry('hist-a', 'NaN', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(2)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-a', match]]);
		expect(spy.calls.setPlayableCount).toEqual([['hist-a', 12]]);
		expect(spy.fetchKitsuEpisodes).toHaveBeenCalledWith('k-a', 1);
		expect(spy.calls.setEpisode).toEqual([['hist-a', expect.objectContaining({ number: 2 })]]);
	});
});
