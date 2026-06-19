import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveKitsuMatch } from './match';
import { resolveHistoryEntry } from './resolve';
import {
	allmangaKitsuMapDelete,
	allmangaKitsuMapGet,
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuResolveAllmangaShowId,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type HistoryEntry,
	type KitsuAnimeRef
} from '$lib/api';

// Mock the api module wholesale — `match.ts` is decoupled from the
// transport (was Tauri invoke, now HTTP fetch), and the assertions
// here are about which api functions get called with what args.
// Mocking the module itself lets these tests survive any future
// transport switch without churn.
vi.mock('$lib/api', () => ({
	allmangaKitsuMapDelete: vi.fn(),
	allmangaKitsuMapGet: vi.fn(),
	kitsuAnimeBySlug: vi.fn(),
	kitsuAnimeDetail: vi.fn(),
	kitsuResolveAllmangaShowId: vi.fn(),
	kitsuSearch: vi.fn(),
	kitsuTitleMatchGet: vi.fn(),
	kitsuTitleMatchPut: vi.fn()
}));

const mockedAllmangaDelete = vi.mocked(allmangaKitsuMapDelete);
const mockedAllmangaMap = vi.mocked(allmangaKitsuMapGet);
const mockedSlug = vi.mocked(kitsuAnimeBySlug);
const mockedDetail = vi.mocked(kitsuAnimeDetail);
const mockedResolveAllmanga = vi.mocked(kitsuResolveAllmangaShowId);
const mockedSearch = vi.mocked(kitsuSearch);
const mockedGetMatch = vi.mocked(kitsuTitleMatchGet);
const mockedPutMatch = vi.mocked(kitsuTitleMatchPut);

const stubKitsu = (
	id: string,
	canonical_title = 'Stub',
	episode_count: number | null = null
): KitsuAnimeRef => ({
	id,
	canonical_title,
	slug: null,
	synopsis: null,
	start_date: null,
	end_date: null,
	episode_count,
	average_rating: null,
	subtype: null,
	status: null,
	age_rating: null,
	popularity_rank: null,
	poster_image: null,
	cover_image: null
});

const entry = (title: string, ep_no = '1'): HistoryEntry => ({
	id: 'allmanga-id',
	ep_no,
	title
});

beforeEach(() => {
	mockedAllmangaDelete.mockReset();
	mockedAllmangaDelete.mockResolvedValue(undefined);
	mockedAllmangaMap.mockReset();
	mockedAllmangaMap.mockResolvedValue(null);
	mockedSlug.mockReset();
	mockedDetail.mockReset();
	mockedResolveAllmanga.mockReset();
	mockedResolveAllmanga.mockResolvedValue(null);
	mockedSearch.mockReset();
	mockedGetMatch.mockReset();
	mockedPutMatch.mockReset();
	mockedPutMatch.mockResolvedValue(undefined);
});

describe('resolveKitsuMatch', () => {
	it('returns the cached anime detail when the title-match cache hits', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue('cached-id');
		mockedDetail.mockResolvedValue(stubKitsu('cached-id', 'Demon Slayer'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('cached-id');
		expect(mockedGetMatch).toHaveBeenCalled();
		expect(mockedDetail).toHaveBeenCalledWith('cached-id');
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through to a live search + pick + put on cache miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('fresh-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('fresh-id');
		expect(mockedGetMatch).toHaveBeenCalled();
		expect(mockedSearch).toHaveBeenCalled();
		expect(mockedPutMatch).toHaveBeenCalledWith(
			preliminary.searchTitle,
			preliminary.cour,
			'fresh-id'
		);
	});

	it('cour > 1 with stale cache hit (slug mismatch) falls through to slug-fetch', async () => {
		// Pre-86e02d2 versions of the picker collapsed sequels onto
		// Part 1 and persisted "Part 2 → Part 1's id" into the cache.
		// On a cache hit, validate the anime's slug — if it doesn't
		// carry the cour suffix, the mapping is stale and we re-resolve.
		const preliminary = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		const stalePart1 = {
			...stubKitsu('part1-stale', 'Stone Ocean'),
			slug: 'jojo-s-bizarre-adventure-part-6-stone-ocean'
		};
		mockedGetMatch.mockResolvedValue('part1-stale');
		mockedDetail.mockResolvedValue(stalePart1);
		mockedSlug.mockResolvedValue(stubKitsu('part2-correct'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-correct');
	});

	it('cour > 1 with cache hit whose slug DOES match returns cached without re-fetch', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		const correctlyCached = {
			...stubKitsu('part2-cached', 'Some Anime Part 2'),
			slug: 'some-anime-part-2'
		};
		mockedGetMatch.mockResolvedValue('part2-cached');
		mockedDetail.mockResolvedValue(correctlyCached);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-cached');
		expect(mockedSlug).not.toHaveBeenCalled();
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through to live search when kitsuAnimeDetail rejects (stale cached id)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue('stale-id');
		mockedDetail.mockRejectedValue(new Error('404'));
		mockedSearch.mockResolvedValue([stubKitsu('rebuilt-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('rebuilt-id');
	});

	it('returns null when the live search itself fails', async () => {
		const preliminary = resolveHistoryEntry(entry('Obscure (12 episodes)', '1'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockRejectedValue(new Error('network down'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got).toBeNull();
	});

	it('still returns the live match when the cache write fails (non-fatal)', async () => {
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('id-1', 'Demon Slayer')]);
		mockedPutMatch.mockRejectedValue(new Error('disk full'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
	});

	it('passes searchTitle (cour-stripped if applicable) + cour to the cache key', async () => {
		const preliminary = resolveHistoryEntry(
			entry('JoJo Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([]);

		await resolveKitsuMatch(preliminary);
		expect(mockedGetMatch).toHaveBeenCalledWith(preliminary.searchTitle, 2);
	});

	it('multi-cour entry: tries slug-fetch first and skips search when slug hits', async () => {
		// Stone Ocean Part 2: Kitsu's text-search drops it; the slug
		// lookup pinpoints it. resolveKitsuMatch should NOT fall through
		// to a search call once the slug returns a hit.
		const preliminary = resolveHistoryEntry(
			entry('JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)', '4'),
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(stubKitsu('part2-id', 'JoJo Stone Ocean Part 2'));

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('part2-id');
		expect(mockedSlug).toHaveBeenCalledWith('jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2');
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('multi-cour entry: falls through to search + pick when slug miss', async () => {
		const preliminary = resolveHistoryEntry(entry('Some Anime Part 2 (12 episodes)', '3'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSlug.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('searched-id', 'Some Anime Part 2')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('searched-id');
		expect(mockedSlug).toHaveBeenCalled();
		expect(mockedSearch).toHaveBeenCalled();
	});

	it('single-cour entry: skips slug-fetch and goes straight to search', async () => {
		// We don't want to double the IPC volume on cold load; slug
		// fetch is opt-in for cour > 1.
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('id-1', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('id-1');
		expect(mockedSlug).not.toHaveBeenCalled();
	});

	// — allmanga show_id → kitsu_id reverse mapping ————————————————
	//
	// Once the user has played a show through the GUI, the backend
	// has a deterministic id-keyed mapping that beats fuzzy text
	// search. Resolver checks this first; on hit, no kitsuSearch /
	// title-match round-trip is necessary.

	it('uses allmanga→kitsu reverse mapping when present', async () => {
		// Naruto's allmanga title is typo'd ("Nato: Shippuuden") so
		// the title-match path mismatches it to Mysterious Girlfriend
		// X. The reverse mapping recorded on play side-steps that
		// failure mode entirely.
		const preliminary = resolveHistoryEntry(
			{ id: 'vDTSJHSpYnrkZnAvG', ep_no: '150', title: 'Nato: Shippuuden (500 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue('11061');
		// Real Kitsu data: Naruto: Shippuuden has episode_count = 500.
		// Pass it explicitly so the count compatibility check in
		// resolveKitsuMatch's step-0 reverse-cache path validates the
		// cached detail against history's courSize=500 and accepts.
		mockedDetail.mockResolvedValue(stubKitsu('11061', 'Naruto: Shippuuden', 500));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('11061');
		expect(mockedAllmangaMap).toHaveBeenCalledWith('vDTSJHSpYnrkZnAvG');
		expect(mockedDetail).toHaveBeenCalledWith('11061');
		expect(mockedGetMatch).not.toHaveBeenCalled();
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through to title-match when reverse mapping misses', async () => {
		// First-time load (no play through GUI yet). Returning null
		// from the new endpoint must not break the legacy resolver.
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedAllmangaMap.mockResolvedValue(null);
		mockedGetMatch.mockResolvedValue('cached-id');
		mockedDetail.mockResolvedValue(stubKitsu('cached-id', 'Demon Slayer'));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('cached-id');
		expect(mockedAllmangaMap).toHaveBeenCalled();
		expect(mockedGetMatch).toHaveBeenCalled();
	});

	it('falls through when the reverse-mapping endpoint itself rejects', async () => {
		// Backend transient error (network blip, 5xx). Resolver must
		// degrade gracefully — same behaviour as the title-match
		// outer-catch.
		const preliminary = resolveHistoryEntry(entry('Demon Slayer (26 episodes)', '5'), null);
		mockedAllmangaMap.mockRejectedValueOnce(new Error('boom'));
		mockedGetMatch.mockResolvedValue('cached-id');
		mockedDetail.mockResolvedValue(stubKitsu('cached-id', 'Demon Slayer'));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('cached-id');
		expect(mockedGetMatch).toHaveBeenCalled();
	});

	it('skips the reverse-mapping path when allmangaShowId is empty', async () => {
		// Defensive: ResumeTarget's allmangaShowId is always set from
		// entry.id, but if a future caller hands us a blank id we
		// shouldn't make a useless round-trip.
		const preliminary = resolveHistoryEntry(
			{ id: '', ep_no: '1', title: 'Demon Slayer (26 episodes)' },
			null
		);
		mockedGetMatch.mockResolvedValue('cached-id');
		mockedDetail.mockResolvedValue(stubKitsu('cached-id', 'Demon Slayer'));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('cached-id');
		expect(mockedAllmangaMap).not.toHaveBeenCalled();
	});

	it('cour > 1 reverse-map hit with slug mismatch evicts the bad row + falls through', async () => {
		// The production poisoning case: Stone Ocean Part 2's allmanga
		// show_id (D5ksnsKtYAzzFXeSp) was mapped to Stone Ocean Part 1's
		// Kitsu id (44294) by a play through Part 1's detail page where
		// the backend's picker landed on the Part 2 sibling. Step 0's
		// existing ep-count check accepts (both parts are 12 eps);
		// without a slug guard step 0 returns Part 1 and the Continue
		// Watching card displays / navigates to the wrong show.
		// Guard: when cour > 1 and the cached anime's slug doesn't
		// carry the matching -part-N suffix, evict the bad mapping
		// and fall through to the live slug-fetch path.
		const preliminary = resolveHistoryEntry(
			{
				id: 'D5ksnsKtYAzzFXeSp',
				ep_no: '4',
				title: 'JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)'
			},
			null
		);
		mockedAllmangaMap.mockResolvedValue('44294');
		mockedDetail.mockResolvedValueOnce({
			...stubKitsu('44294', 'Stone Ocean', 12),
			// Part 1's slug carries no -part-N. cour > 1 expects -part-2.
			slug: 'jojo-no-kimyou-na-bouken-stone-ocean'
		});
		mockedSlug.mockResolvedValue({
			...stubKitsu('46010', 'JoJo no Kimyou na Bouken: Stone Ocean Part 2', 12),
			slug: 'jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2'
		});

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('46010');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('D5ksnsKtYAzzFXeSp');
		expect(mockedSlug).toHaveBeenCalled();
	});

	it('cour > 1 reverse-map hit with absent slug preserves the cache (no eviction)', async () => {
		// Codex P2: an absent slug is missing evidence, not proof of
		// cross-cour poisoning. Evicting on slug=null churns valid rows
		// whose Kitsu detail payload simply doesn't include the slug.
		const preliminary = resolveHistoryEntry(
			{
				id: 'D5ksnsKtYAzzFXeSp',
				ep_no: '4',
				title: 'JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)'
			},
			null
		);
		mockedAllmangaMap.mockResolvedValue('46010');
		mockedDetail.mockResolvedValueOnce({
			...stubKitsu('46010', 'JoJo no Kimyou na Bouken: Stone Ocean Part 2', 12),
			slug: null
		});

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('46010');
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
		expect(mockedSlug).not.toHaveBeenCalled();
	});

	it('cour > 1 slug mismatch tolerates a failing eviction call (fire-and-forget)', async () => {
		// The eviction call is fire-and-forget — the rest of step 1+ must
		// still complete cleanly even if the cache-delete IPC rejects
		// (transient backend hiccup, offline, etc.).
		const preliminary = resolveHistoryEntry(
			{
				id: 'D5ksnsKtYAzzFXeSp',
				ep_no: '4',
				title: 'JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)'
			},
			null
		);
		mockedAllmangaMap.mockResolvedValue('44294');
		mockedDetail.mockResolvedValueOnce({
			...stubKitsu('44294', 'Stone Ocean', 12),
			slug: 'jojo-no-kimyou-na-bouken-stone-ocean'
		});
		mockedAllmangaDelete.mockRejectedValue(new Error('cache backend down'));
		mockedSlug.mockResolvedValue({
			...stubKitsu('46010', 'JoJo no Kimyou na Bouken: Stone Ocean Part 2', 12),
			slug: 'jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2'
		});

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('46010');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('D5ksnsKtYAzzFXeSp');
	});

	it('cour > 1 reverse-map hit with matching slug keeps the cache + skips re-resolve', async () => {
		// Negative case: the reverse mapping IS correct (Part 2 →
		// Part 2's real Kitsu id with the -part-2 slug). Step 0 should
		// return the cached detail and never evict or hit slug-fetch.
		const preliminary = resolveHistoryEntry(
			{
				id: 'D5ksnsKtYAzzFXeSp',
				ep_no: '4',
				title: 'JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)'
			},
			null
		);
		mockedAllmangaMap.mockResolvedValue('46010');
		mockedDetail.mockResolvedValueOnce({
			...stubKitsu('46010', 'JoJo no Kimyou na Bouken: Stone Ocean Part 2', 12),
			slug: 'jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2'
		});

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('46010');
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
		expect(mockedSlug).not.toHaveBeenCalled();
	});

	it('falls through to title-match when reverse-mapped detail fetch fails', async () => {
		// Stale id (Kitsu removed the entry that was mapped). The
		// resolver should not fail — it falls through to the live
		// title-search path so the row eventually heals.
		const preliminary = resolveHistoryEntry(
			{ id: 'show-stale', ep_no: '5', title: 'Demon Slayer (26 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue('stale-kitsu');
		mockedDetail.mockRejectedValueOnce(new Error('not found'));
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('fresh-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('fresh-id');
		expect(mockedSearch).toHaveBeenCalled();
	});

	it('falls through to allmanga show enrichment when text search returns 0 hits', async () => {
		// Repro: cleared metadata cache + cryptic allmanga `name`. The
		// reverse cache miss + title-match cache miss + slug skip + 0-hit
		// text search should NOT be terminal — the resolver calls the
		// new enrichment IPC, which fetches allmanga's Show GraphQL and
		// retries Kitsu search with englishName / altNames.
		const preliminary = resolveHistoryEntry(
			{ id: 'ReooPAxPMsHM4KPMY', ep_no: '1', title: '1P (1161 episodes)' },
			null
		);
		// All earlier paths whiff:
		mockedAllmangaMap.mockResolvedValue(null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([]);
		// Enrichment recovers — backend returns the proper Kitsu entry.
		mockedResolveAllmanga.mockResolvedValue(stubKitsu('12', 'One Piece'));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('12');
		expect(got?.canonical_title).toBe('One Piece');
		expect(mockedResolveAllmanga).toHaveBeenCalledWith('ReooPAxPMsHM4KPMY');
	});

	it('returns null when text search and allmanga enrichment both miss', async () => {
		// Worst case: title-search empty AND backend enrichment also
		// finds no Kitsu match (shows allmanga indexes that Kitsu
		// doesn't carry at all). Resolver returns null; the home page
		// renders the bare allmanga title and routes the resume card
		// to /search.
		const preliminary = resolveHistoryEntry(
			{ id: 'unknown-show', ep_no: '1', title: 'mystery (1 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue(null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([]);
		mockedResolveAllmanga.mockResolvedValue(null);

		const got = await resolveKitsuMatch(preliminary);
		expect(got).toBeNull();
	});

	it('skips enrichment when text search hits — no extra IPC', async () => {
		// Common case: title-search wins on the first try. Don't
		// double-spend by also calling the enrichment endpoint.
		const preliminary = resolveHistoryEntry(
			{ id: 'show-x', ep_no: '5', title: 'Demon Slayer (26 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue(null);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('id-1', 'Demon Slayer')]);

		await resolveKitsuMatch(preliminary);
		expect(mockedResolveAllmanga).not.toHaveBeenCalled();
	});

	// — identity guard: music subtype + gross title mismatch ————————————

	it('evicts a music-subtype reverse-map binding and re-resolves (the Idol bug)', async () => {
		// The Love Live movie's allmanga show_id was poisoned to point at the
		// YOASOBI "Idol" music video (Kitsu subtype `music`, 1 ep). Music never
		// exists on allanime, so step 0 must drop the row and re-resolve — the
		// card stops showing "Idol" for a Love Live entry.
		const preliminary = resolveHistoryEntry(
			{
				id: '9mJyPki2Hm4NmSrhG',
				ep_no: '1',
				title: 'Love Live! Nijigasaki Gakuen School Idol Doukoukai: Kanketsu-hen (1 episodes)'
			},
			null
		);
		mockedAllmangaMap.mockResolvedValue('47328');
		mockedDetail.mockResolvedValue({ ...stubKitsu('47328', 'Idol', 1), subtype: 'music' });
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([
			stubKitsu('love-live', 'Love Live! Nijigasaki Gakuen School Idol Doukoukai: Kanketsu-hen', 1)
		]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('love-live');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('9mJyPki2Hm4NmSrhG');
		expect(mockedSearch).toHaveBeenCalled();
	});

	it('evicts a reverse-map binding whose title grossly mismatches the entry', async () => {
		// Non-music poison: an informative hsts title bound to an unrelated Kitsu
		// entry. The title tripwire alone evicts + re-resolves.
		const preliminary = resolveHistoryEntry(
			{ id: 'show-x', ep_no: '5', title: 'Some Very Specific Long Title (12 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue('wrong');
		mockedDetail.mockResolvedValue(stubKitsu('wrong', 'Totally Unrelated Other Show', 12));
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('right', 'Some Very Specific Long Title', 12)]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('right');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('show-x');
	});

	it('keeps a reverse-map binding whose allmanga title is a plausible typo', async () => {
		// Guard must not over-reject: "Nato: Shippuuden" (allmanga typo) shares
		// the distinctive "Shippuuden" with "Naruto: Shippuuden", so the binding
		// stays trusted — no eviction, no re-search.
		const preliminary = resolveHistoryEntry(
			{ id: 'naruto-id', ep_no: '150', title: 'Nato: Shippuuden (500 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue('11061');
		mockedDetail.mockResolvedValue(stubKitsu('11061', 'Naruto: Shippuuden', 500));

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('11061');
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
		expect(mockedSearch).not.toHaveBeenCalled();
	});

	it('falls through a music-subtype title-match cache hit', async () => {
		// Step 1 (title-match cache) applies the same guard. "Idol" is a stub the
		// title tripwire can't judge, so this isolates the music gate.
		const preliminary = resolveHistoryEntry(entry('Idol (1 episodes)', '1'), null);
		mockedAllmangaMap.mockResolvedValue(null);
		mockedGetMatch.mockResolvedValue('47328');
		mockedDetail.mockResolvedValue({ ...stubKitsu('47328', 'Idol', 1), subtype: 'music' });
		mockedSearch.mockResolvedValue([stubKitsu('real', 'Idol Anime', 1)]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('real');
		expect(mockedSearch).toHaveBeenCalled();
	});
});
