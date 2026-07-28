import { describe, expect, it, vi } from 'vitest';
import { resolveResumeEpisode } from './resume-episode';
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

describe('resolveResumeEpisode', () => {
	it('uses a probed cap without any live fetch', async () => {
		const fetchCount = vi.fn();
		const r = await resolveResumeEpisode(12, 12, 24, fetchCount);
		expect(r).toEqual({ episode: 12, count: 12, approximate: false });
		expect(fetchCount).not.toHaveBeenCalled();
	});

	it('live-probes when the cap is unknown and honors the answer', async () => {
		// The lagging-dub case: Kitsu announces 24, only 12 are
		// actually playable. The pre-probe click must not forward
		// episode 13 into an episode-not-released error — the live
		// answer keeps the user on a replay of 12.
		const fetchCount = vi.fn().mockResolvedValue({ count: 12, approximate: false });
		const r = await resolveResumeEpisode(12, null, 24, fetchCount);
		expect(r).toEqual({ episode: 12, count: 12, approximate: false });
	});

	it('falls back to the Kitsu count when the live probe fails', async () => {
		const fetchCount = vi.fn().mockRejectedValue(new Error('breaker open'));
		const r = await resolveResumeEpisode(5, null, 24, fetchCount);
		expect(r).toEqual({ episode: 6, count: null, approximate: false });
	});

	it('falls back to the Kitsu count when the live probe has no count', async () => {
		const fetchCount = vi.fn().mockResolvedValue({ count: null, approximate: true });
		const r = await resolveResumeEpisode(5, null, 24, fetchCount);
		expect(r).toEqual({ episode: 6, count: null, approximate: false });
	});

	it('treats malformed lastWatched as no-history (episode 1)', async () => {
		const fetchCount = vi.fn().mockResolvedValue({ count: 12, approximate: false });
		const r = await resolveResumeEpisode(null, null, 24, fetchCount);
		expect(r.episode).toBe(1);
	});
});

describe('acceptance: early click on a just-released card (loader + resolver)', () => {
	it('a click before the background probe lands still replays the last dubbed episode', async () => {
		// End-to-end over the real loader and the real resolver: the
		// card releases at Kitsu-match time (background probe hanging),
		// the user clicks immediately, and the click-time resolution
		// answers with the true playable cap — replay of 12, never a
		// phantom episode 13.
		const entry = makeEntry('hist-a', '12', 'Show A');
		const match = makeMatch('k-a', 24);
		const resolveMatch = vi.fn().mockResolvedValue(match);
		const backgroundProbe = new Promise<{ episode_count: number }>(() => {});
		const fetchAvailability = vi.fn().mockReturnValue(backgroundProbe);

		let releasedMatch: KitsuAnimeRef | null = null;
		let releasedCount: number | null = null;
		void loadContinueWatchingState([entry], {
			resolveMatch,
			fetchAvailability,
			getMode: () => Promise.resolve('sub' as const),
			onRowReady: (_id, m, count) => {
				releasedMatch = m;
				releasedCount = count;
			}
		});
		for (let i = 0; i < 20; i++) await Promise.resolve();

		// Card is rendered and clickable; the cap is still unknown.
		expect(releasedMatch).toEqual(match);
		expect(releasedCount).toBeNull();

		// The click resolves the episode live (interactive lane).
		const clickProbe = vi.fn().mockResolvedValue({ count: 12, approximate: false });
		const r = await resolveResumeEpisode(
			12,
			releasedCount,
			(releasedMatch as KitsuAnimeRef | null)?.episode_count ?? null,
			clickProbe
		);
		expect(r).toEqual({ episode: 12, count: 12, approximate: false });
		expect(clickProbe).toHaveBeenCalledTimes(1);
	});
});

describe('acceptance: click-time lookup rides the interactive lane with awaited settings', () => {
	it('the real wrapper sends background:false and the awaited dub mode', async () => {
		// Composes the real pieces of the click path: settings resolve
		// mid-click with DUB, the cap lookup goes out through the real
		// makeFetchAvailability with the interactive flag, and the
		// episode is picked against the answered cap.
		const { makeFetchAvailability } = await import('./availability-from-match');
		const { resolveResumeSettings } = await import('./resume-settings');
		const checkAvailability = vi.fn().mockResolvedValue({ available: true, episode_count: 12 });

		const settings = await resolveResumeSettings(
			null,
			Promise.resolve({ mode: 'dub', quality: 'best' } as never)
		);
		const fetch = makeFetchAvailability(checkAvailability, { background: false });
		const r = await resolveResumeEpisode(12, null, 24, () =>
			fetch(makeMatch('k-a', 24), settings.mode).then((res) => ({
				count: res?.episode_count ?? null,
				approximate: res?.episode_count_approximate === true
			}))
		);

		expect(checkAvailability).toHaveBeenCalledWith(
			expect.objectContaining({ background: false, mode: 'dub', kitsu_id: 'k-a' })
		);
		expect(r).toEqual({ episode: 12, count: 12, approximate: false });
	});
});

describe('an approximate cap at the boundary is revalidated', () => {
	// The background cap is not always exact. When the scraper gate
	// refuses the detail fetch, the backend deliberately falls back to
	// the search hit's count and caches it — its own comment says "off
	// by one for shows with halves, but good enough for the cap until
	// next probe". Treating every numeric cap as authoritative then
	// forwards a phantom episode: true final 12 under an approximate
	// cap of 13 resolves to 13, and playback fails on an episode that
	// does not exist.
	//
	// The click cannot see whether a cap is approximate, but it does
	// know when it is standing on the boundary — the only place the
	// off-by-one can bite. Revalidating just that case costs one
	// interactive lookup, on the lane that is neither paced nor
	// refused, while the resume spinner is already up.
	it('revalidates when the next episode would be the cap itself', async () => {
		const fetchCount = vi.fn(async () => ({ count: 12, approximate: false }));
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, true);
		expect(fetchCount).toHaveBeenCalledTimes(1);
		expect(r.episode).toBe(12); // replay the true finale, not phantom 13
		expect(r.count).toBe(12);
	});

	it('keeps the boundary cap when revalidation confirms it', async () => {
		const fetchCount = vi.fn(async () => ({ count: 13, approximate: false }));
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, true);
		expect(fetchCount).toHaveBeenCalledTimes(1);
		expect(r.episode).toBe(13);
		expect(r.count).toBe(13);
	});

	it('trusts the cap when the click is not standing on the boundary', async () => {
		// An EXACT cap needs no lookup at any position.
		const fetchCount = vi.fn(async () => ({ count: 12, approximate: false }));
		const r = await resolveResumeEpisode(5, 13, 24, fetchCount);
		expect(fetchCount).not.toHaveBeenCalled();
		expect(r.episode).toBe(6);
		expect(r.count).toBe(13);
	});

	it('falls back to the background cap when revalidation fails', async () => {
		const fetchCount = vi.fn(async () => {
			throw new Error('offline');
		});
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, true);
		expect(r.episode).toBe(13);
		expect(r.count).toBe(13);
	});
});

describe('a cap that lands while the interactive lookup runs', () => {
	// The click can start before the background probe settles: the
	// snapshot cap is null, so the interactive lookup runs. If that
	// lookup fails while the background probe publishes an exact cap
	// in the meantime, falling back to Kitsu's optimistic count throws
	// away a better answer that is already in hand — and Kitsu's count
	// is exactly the number the probe exists to correct.
	it('prefers a cap published during the lookup over Kitsu on failure', async () => {
		let published: number | null = null;
		const fetchCount = vi.fn(async () => {
			published = 12; // the background probe lands mid-flight
			throw new Error('interactive lookup failed');
		});
		const r = await resolveResumeEpisode(12, null, 24, fetchCount, () => published);
		expect(r.episode).toBe(12); // replay the real finale, not Kitsu's 13
		expect(r.count).toBe(12);
	});

	it('still falls back to Kitsu when no cap arrived', async () => {
		const fetchCount = vi.fn(async () => {
			throw new Error('interactive lookup failed');
		});
		const r = await resolveResumeEpisode(12, null, 24, fetchCount, () => null);
		expect(r.episode).toBe(13);
	});

	it('treats the reader as optional for callers that have no live cap', async () => {
		const fetchCount = vi.fn(async () => ({ count: 12, approximate: false }));
		const r = await resolveResumeEpisode(5, null, 24, fetchCount);
		expect(r.episode).toBe(6);
		expect(r.count).toBe(12);
	});
});

describe('an approximate cap is never trusted', () => {
	// The backend reports when a count came from the search hit rather
	// than the detail fetch. That number counts half episodes as whole
	// ones, so it can run high by MORE than one when a show carries
	// several — which is why position ("is the next episode exactly
	// the cap?") is not a safe proxy for the risk.
	it('revalidates an approximate cap even below the boundary', async () => {
		const fetchCount = vi.fn(async () => ({ count: 12, approximate: false }));
		const r = await resolveResumeEpisode(5, 14, 24, fetchCount, undefined, true);
		expect(fetchCount).toHaveBeenCalledTimes(1);
		expect(r.episode).toBe(6);
		expect(r.count).toBe(12);
	});

	it('still trusts an exact cap without any lookup', async () => {
		const fetchCount = vi.fn();
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, false);
		expect(fetchCount).not.toHaveBeenCalled();
		expect(r.episode).toBe(13);
	});

	it('keeps the approximate cap when revalidation fails', async () => {
		const fetchCount = vi.fn(async () => {
			throw new Error('offline');
		});
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, true);
		expect(r.episode).toBe(13);
		expect(r.count).toBe(13);
	});
});

describe('an interactive lookup that is itself approximate', () => {
	// The interactive lane bypasses the gate, but its detail fetch can
	// still fail — the backend then returns the search-hit count
	// marked approximate. Collapsing that to a bare number lets the
	// page record it as exact, so this click can advance to a phantom
	// AND every later click trusts the same cap.
	it('reports an approximate lookup result as approximate', async () => {
		const fetchCount = vi.fn(async () => ({ count: 13, approximate: true }));
		const r = await resolveResumeEpisode(12, null, 24, fetchCount);
		expect(r.count).toBe(13);
		expect(r.approximate).toBe(true);
	});

	it('reports a confirmed lookup result as exact', async () => {
		const fetchCount = vi.fn(async () => ({ count: 12, approximate: false }));
		const r = await resolveResumeEpisode(12, null, 24, fetchCount);
		expect(r.episode).toBe(12);
		expect(r.approximate).toBe(false);
	});

	it('a revalidation that comes back approximate stays approximate', async () => {
		const fetchCount = vi.fn(async () => ({ count: 13, approximate: true }));
		const r = await resolveResumeEpisode(12, 13, 24, fetchCount, undefined, true);
		expect(r.approximate).toBe(true);
	});
});
