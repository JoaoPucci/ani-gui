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

	it('malformed ep_no (NaN-parse): treats the row as no-history, fetches episode 1', async () => {
		// Codex P2 #3349231667 — a user-edited or otherwise malformed
		// ep_no ('abc', '0', empty) must not surface a phantom "watched
		// episode 1, ready for episode 2" state. The detail page's
		// defaultEpisode reads raw `parseInt(resumeEntry.ep_no, 10)`
		// and lets pickNextEpisode collapse NaN/<1 to episode 1. The
		// home handler must do the same so both Continue surfaces
		// agree on a malformed row.
		const entry = makeEntry('hist-a', 'abc', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(1)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setMatch).toEqual([['hist-a', match]]);
		expect(spy.calls.setPlayableCount).toEqual([['hist-a', 12]]);
		// pickNextEpisode(NaN, 12) → 1; ceil(1/20) → page 1; setEpisode
		// receives the ep-1 row, not an ep-2 lookup that would miss.
		expect(spy.fetchKitsuEpisodes).toHaveBeenCalledWith('k-a', 1);
		expect(spy.calls.setEpisode).toEqual([['hist-a', expect.objectContaining({ number: 1 })]]);
	});

	it('ep_no="0" is treated as no-history (matches pickNextEpisode\'s <1 fence)', async () => {
		const entry = makeEntry('hist-a', '0', 'Show A');
		const match = makeMatch('k-a', 12);
		const spy = makeSpy([entry], () => Promise.resolve([makeKitsuEpisode(1)]));
		const handle = makeContinueRowReadyHandler(spy.deps);

		handle('hist-a', match, 12);
		await Promise.resolve();
		await Promise.resolve();

		expect(spy.calls.setEpisode).toEqual([['hist-a', expect.objectContaining({ number: 1 })]]);
	});
});

describe('refinement re-fire (render-then-refine)', () => {
	function defer<T>(): { promise: Promise<T>; resolve: (v: T) => void } {
		let resolveFn!: (v: T) => void;
		const promise = new Promise<T>((res) => {
			resolveFn = res;
		});
		return { promise, resolve: resolveFn };
	}

	it('a stale first episode fetch loses to the refinement fetch', async () => {
		// The loader now calls the handler twice per probed row: once
		// at match-release (count null), once when the probe refines
		// the cap. Each call starts an episode fetch; if the FIRST
		// fetch resolves LAST, its stale episode must not overwrite
		// the refinement's. Guarded by a per-entry token.
		const entry = makeEntry('h1', '12', 'Show');
		const match = makeMatch('k1', 24);
		const first = defer<KitsuEpisode[]>();
		const second = defer<KitsuEpisode[]>();
		const fetchKitsuEpisodes = vi
			.fn()
			.mockReturnValueOnce(first.promise)
			.mockReturnValueOnce(second.promise);
		const episodes: [string, KitsuEpisode | null][] = [];
		const handler = makeContinueRowReadyHandler({
			historyById: new Map([[entry.id, entry]]),
			fetchKitsuEpisodes,
			setMatch: () => {},
			setPlayableCount: () => {},
			setEpisode: (id, ep) => episodes.push([id, ep])
		});

		// Release: cap falls back to match.episode_count (24) → next 13.
		handler(entry.id, match, null);
		// Refinement: cap 12 → watched 12 of 12 → replay 12.
		handler(entry.id, match, 12);

		// Refinement's fetch lands first…
		second.resolve([makeKitsuEpisode(12)]);
		await Promise.resolve();
		await Promise.resolve();
		// …then the stale release-time fetch straggles in.
		first.resolve([makeKitsuEpisode(13)]);
		await Promise.resolve();
		await Promise.resolve();

		expect(episodes.at(-1)).toEqual(['h1', makeKitsuEpisode(12)]);
	});
	it('an unchanged target reuses the in-flight fetch instead of restarting it', async () => {
		// The common refinement is a NO-OP for the target: the probe
		// reports the same count Kitsu already gave, or the user is
		// below both caps, so nextEpisode and its page are unchanged.
		// Replacing the token there starts a second identical request
		// — double the cold-cache episode traffic for most Continue
		// rows — and hands the row's outcome to the newer request. If
		// that one fails while the original succeeds, the catch writes
		// null and the successful response is discarded as stale.
		const entry = makeEntry('h1', '5', 'Show');
		const match = makeMatch('k1', 24);
		const first = defer<KitsuEpisode[]>();
		// mockImplementationOnce, not mockReturnValueOnce: the rejected
		// promise must only exist if the handler actually calls it.
		const fetchKitsuEpisodes = vi
			.fn()
			.mockReturnValueOnce(first.promise)
			.mockImplementationOnce(() => Promise.reject(new Error('refinement request failed')));
		const episodes: [string, KitsuEpisode | null][] = [];
		const handler = makeContinueRowReadyHandler({
			historyById: new Map([[entry.id, entry]]),
			fetchKitsuEpisodes,
			setMatch: () => {},
			setPlayableCount: () => {},
			setEpisode: (id, ep) => episodes.push([id, ep])
		});

		// Release: cap 24 → next 6. Refinement: cap 24 again → next 6.
		handler(entry.id, match, null);
		handler(entry.id, match, 24);

		expect(fetchKitsuEpisodes).toHaveBeenCalledTimes(1);

		first.resolve([makeKitsuEpisode(6)]);
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();

		expect(episodes.at(-1)).toEqual(['h1', makeKitsuEpisode(6)]);
	});

	it('a changed target still supersedes the in-flight fetch', async () => {
		// Reuse is keyed on the target, not on "a request exists" —
		// a refinement that genuinely moves the episode must still
		// start its own fetch and win.
		const entry = makeEntry('h1', '12', 'Show');
		const match = makeMatch('k1', 24);
		const first = defer<KitsuEpisode[]>();
		const second = defer<KitsuEpisode[]>();
		const fetchKitsuEpisodes = vi
			.fn()
			.mockReturnValueOnce(first.promise)
			.mockReturnValueOnce(second.promise);
		const episodes: [string, KitsuEpisode | null][] = [];
		const handler = makeContinueRowReadyHandler({
			historyById: new Map([[entry.id, entry]]),
			fetchKitsuEpisodes,
			setMatch: () => {},
			setPlayableCount: () => {},
			setEpisode: (id, ep) => episodes.push([id, ep])
		});

		handler(entry.id, match, null); // next 13
		handler(entry.id, match, 12); // next 12 — a real change

		expect(fetchKitsuEpisodes).toHaveBeenCalledTimes(2);

		second.resolve([makeKitsuEpisode(12)]);
		await Promise.resolve();
		await Promise.resolve();
		first.resolve([makeKitsuEpisode(13)]);
		await Promise.resolve();
		await Promise.resolve();

		expect(episodes.at(-1)).toEqual(['h1', makeKitsuEpisode(12)]);
	});

	it('a settled target is re-fetched when the row fires again', async () => {
		// Reuse must not become a permanent cache: once the request
		// has settled there is nothing in flight to share, so a later
		// re-fire for the same target has to issue a fresh request.
		const entry = makeEntry('h1', '5', 'Show');
		const match = makeMatch('k1', 24);
		const fetchKitsuEpisodes = vi.fn(() => Promise.resolve([makeKitsuEpisode(6)]));
		const handler = makeContinueRowReadyHandler({
			historyById: new Map([[entry.id, entry]]),
			fetchKitsuEpisodes,
			setMatch: () => {},
			setPlayableCount: () => {},
			setEpisode: () => {}
		});

		handler(entry.id, match, 24);
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
		handler(entry.id, match, 24);

		expect(fetchKitsuEpisodes).toHaveBeenCalledTimes(2);
	});
});
