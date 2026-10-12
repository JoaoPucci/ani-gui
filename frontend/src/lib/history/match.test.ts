import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveKitsuMatch, resolveKitsuMatchWithTrust } from './match';
import { resolveHistoryEntry } from './resolve';
import {
	allmangaKitsuMapDelete,
	allmangaKitsuMapGet,
	allmangaKitsuMapPlayed,
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
// transport (HTTP fetch to the backend), and the assertions
// here are about which api functions get called with what args.
// Mocking the module itself lets these tests survive any future
// transport switch without churn.
vi.mock('$lib/api', () => ({
	allmangaKitsuMapDelete: vi.fn(),
	allmangaKitsuMapGet: vi.fn(),
	allmangaKitsuMapPlayed: vi.fn(),
	kitsuAnimeBySlug: vi.fn(),
	kitsuAnimeDetail: vi.fn(),
	kitsuResolveAllmangaShowId: vi.fn(),
	kitsuSearch: vi.fn(),
	kitsuTitleMatchGet: vi.fn(),
	kitsuTitleMatchPut: vi.fn()
}));

const mockedAllmangaDelete = vi.mocked(allmangaKitsuMapDelete);
const mockedAllmangaMap = vi.mocked(allmangaKitsuMapGet);
const mockedPlayed = vi.mocked(allmangaKitsuMapPlayed);
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
	mockedPlayed.mockReset();
	mockedPlayed.mockResolvedValue(null);
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
	it('reads the show the row records, with no matching', async () => {
		// The row records the Kitsu id of the page the user played from.
		// Continue Watching takes it as given: no stored mapping, title
		// match, search or enrichment is asked, so nothing can land on
		// another show (the provider's "There Is Also a Hole in the
		// Student Organization!" once matched Here is Greenwood).
		const preliminary = resolveHistoryEntry(
			{
				id: 'hianime:there-is-also-a-hole-in-the-student-organization-10497',
				ep_no: '1',
				title: 'There Is Also a Hole in the Student Organization!',
				kitsu_id: '49877'
			},
			null
		);
		mockedDetail.mockResolvedValue(stubKitsu('49877', 'Seitokai ni mo Ana wa Aru!', 12));
		mockedAllmangaMap.mockResolvedValue('1623');
		mockedSearch.mockResolvedValue([stubKitsu('1623', 'Here is Greenwood', 6)]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('49877');
		expect(mockedDetail).toHaveBeenCalledWith('49877');
		expect(mockedAllmangaMap).not.toHaveBeenCalled();
		expect(mockedGetMatch).not.toHaveBeenCalled();
		expect(mockedSearch).not.toHaveBeenCalled();
		expect(mockedResolveAllmanga).not.toHaveBeenCalled();
	});

	it('answers no match for a row without a recorded show rather than a hit its title refutes', async () => {
		// The provider calls Seitokai ni mo Ana wa Aru! "There Is Also a
		// Hole in the Student Organization!", and a Kitsu search for those
		// words does not return the show: its first hit is Here is
		// Greenwood, which a row without a count once took on the count
		// alone.
		const row = resolveHistoryEntry(
			{
				id: 'hianime:there-is-also-a-hole-in-the-student-organization-10497',
				ep_no: '1',
				title: 'There Is Also a Hole in the Student Organization!'
			},
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([
			{ ...stubKitsu('1623', 'Here is Greenwood', 6), slug: 'here-is-greenwood', subtype: 'OVA' },
			{
				...stubKitsu('1677', 'Tokyo Majin Gakuen Kenpucho: Tou', 14),
				slug: 'tokyo-majin-gakuen-kenpucho-tou',
				subtype: 'TV'
			}
		]);

		const got = await resolveKitsuMatch(row);

		expect(got).toBeNull();
		expect(mockedPutMatch).not.toHaveBeenCalled();
	});

	// A row from before history recorded the show keeps the mapping a
	// play stored when only the provider's title doubts it: the provider
	// calls Seitokai ni mo Ana wa Aru! "There Is Also a Hole in the
	// Student Organization!", which shares only "student" with Kitsu's
	// English title for it, and a search for those words never returns
	// it.
	const seitokai = (): KitsuAnimeRef => ({
		...stubKitsu('49877', 'Seitokai ni mo Ana wa Aru!', 12),
		slug: 'seitokai-ni-mo-ana-wa-aru',
		status: 'current',
		subtype: 'TV',
		titles: {
			en_jp: 'Seitokai ni mo Ana wa Aru!',
			en: 'Even the Student Council Has Its Holes!'
		}
	});
	const greenwood = (): KitsuAnimeRef => ({
		...stubKitsu('1623', 'Here is Greenwood', 6),
		slug: 'here-is-greenwood',
		subtype: 'OVA'
	});
	const seitokaiRow = (title = 'There Is Also a Hole in the Student Organization!') =>
		resolveHistoryEntry(
			{
				id: 'hianime:there-is-also-a-hole-in-the-student-organization-10497',
				ep_no: '1',
				title
			},
			null
		);

	it('keeps a title-doubted mapping a play stored', async () => {
		mockedAllmangaMap.mockResolvedValue('49877');
		mockedDetail.mockResolvedValue(seitokai());
		mockedPlayed.mockResolvedValue('49877');
		mockedSearch.mockResolvedValue([greenwood()]);

		const got = await resolveKitsuMatch(seitokaiRow());

		expect(got?.id).toBe('49877');
		expect(mockedPlayed).toHaveBeenCalledWith(
			'hianime:there-is-also-a-hole-in-the-student-organization-10497'
		);
		expect(mockedSearch).not.toHaveBeenCalled();
		expect(mockedResolveAllmanga).not.toHaveBeenCalled();
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
	});

	it('does not keep a title-doubted mapping when the played read fails', async () => {
		mockedAllmangaMap.mockResolvedValue('49877');
		mockedDetail.mockResolvedValue(seitokai());
		mockedPlayed.mockRejectedValue(new Error('backend down'));
		mockedSearch.mockResolvedValue([greenwood()]);

		const got = await resolveKitsuMatch(seitokaiRow());

		expect(got).toBeNull();
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
	});

	it('does not keep a title-doubted mapping no play stored', async () => {
		// A guess an earlier resolve stored: Greenwood for the same row.
		mockedAllmangaMap.mockResolvedValue('1623');
		mockedDetail.mockResolvedValue(greenwood());
		mockedPlayed.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([greenwood()]);

		const got = await resolveKitsuMatch(seitokaiRow());

		expect(got).toBeNull();
	});

	it('does not keep a title-doubted mapping a play replaced after it was read', async () => {
		// The guess Greenwood was read and fetched; a play stored
		// Seitokai before the played read ran. The mark vouches for
		// Seitokai, not for the Greenwood this load holds.
		mockedAllmangaMap.mockResolvedValue('1623');
		mockedDetail.mockResolvedValue(greenwood());
		mockedPlayed.mockResolvedValue('49877');
		mockedSearch.mockResolvedValue([greenwood()]);

		const got = await resolveKitsuMatch(seitokaiRow());

		expect(got).toBeNull();
	});

	it('does not keep a played mapping its count doubts too', async () => {
		mockedAllmangaMap.mockResolvedValue('1623');
		mockedDetail.mockResolvedValue(greenwood());
		mockedPlayed.mockResolvedValue('1623');
		mockedSearch.mockResolvedValue([]);

		const got = await resolveKitsuMatch(
			seitokaiRow('There Is Also a Hole in the Student Organization! (24 episodes)')
		);

		expect(got?.id).not.toBe('1623');
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
	});

	it('answers no show, not a guess, when the recorded show cannot be read', async () => {
		const preliminary = resolveHistoryEntry(
			{ id: 'hianime:x-1', ep_no: '1', title: 'X', kitsu_id: '49877' },
			null
		);
		mockedDetail.mockRejectedValue(new Error('offline'));
		mockedSearch.mockResolvedValue([stubKitsu('1623', 'X', 6)]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got).toBeNull();
		expect(mockedSearch).not.toHaveBeenCalled();
	});

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
			'fresh-id',
			'anidb'
		);
	});

	it('asks and writes the title-match cache for the provider the history id names', async () => {
		const preliminary = resolveHistoryEntry(
			{ id: 'hianime:demon-slayer-9', ep_no: '5', title: 'Demon Slayer (26 episodes)' },
			null
		);
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('fresh-id', 'Demon Slayer')]);

		const got = await resolveKitsuMatch(preliminary);
		expect(got?.id).toBe('fresh-id');
		expect(mockedGetMatch).toHaveBeenCalledWith(
			preliminary.searchTitle,
			preliminary.cour,
			'hianime'
		);
		expect(mockedPutMatch).toHaveBeenCalledWith(
			preliminary.searchTitle,
			preliminary.cour,
			'fresh-id',
			'hianime'
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
		expect(mockedGetMatch).toHaveBeenCalledWith(preliminary.searchTitle, 2, 'anidb');
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

	// — the provider show_id → kitsu_id reverse mapping ————————————————
	//
	// Once the user has played a show through the GUI, the backend
	// has a deterministic id-keyed mapping that beats fuzzy text
	// search. Resolver checks this first; on hit, no kitsuSearch /
	// title-match round-trip is necessary.

	it('uses the provider→kitsu reverse mapping when present', async () => {
		// Naruto's provider title is typo'd ("Nato: Shippuuden") so
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

	it('cour > 1 reverse-map hit with slug mismatch re-resolves (no delete) + falls through', async () => {
		// The production poisoning case: Stone Ocean Part 2's provider
		// show_id (D5ksnsKtYAzzFXeSp) was mapped to Stone Ocean Part 1's
		// Kitsu id (44294) by a play through Part 1's detail page where
		// the backend's picker landed on the Part 2 sibling. Step 0's
		// existing ep-count check accepts (both parts are 12 eps).
		// Guard: when cour > 1 and the cached anime's slug doesn't carry
		// the matching -part-N suffix, the binding is INCONCLUSIVE — fall
		// through to the live slug-fetch path WITHOUT deleting the row
		// (only a music subtype is deleted). The correct mapping is
		// re-PUT by the resolution, healing the row without risking a
		// valid binding the slug heuristic merely guessed wrong.
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
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
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

	it('music-binding eviction tolerates a failing delete call', async () => {
		// Music is the one binding we delete, and that delete must not break
		// the resolve if the cache-delete IPC rejects (transient backend
		// hiccup, offline, etc.) — the resolver still falls through and heals.
		const preliminary = resolveHistoryEntry(entry('Some Show (12 episodes)', '4'), null);
		mockedAllmangaMap.mockResolvedValue('music-id');
		mockedDetail.mockResolvedValueOnce({
			...stubKitsu('music-id', 'Some Show', 1),
			subtype: 'music'
		});
		mockedAllmangaDelete.mockRejectedValue(new Error('cache backend down'));
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('real', 'Some Show', 12)]);

		const got = await resolveKitsuMatch(preliminary);

		expect(got?.id).toBe('real');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('allmanga-id', 'music-id');
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

	it('falls through to provider show enrichment when text search returns 0 hits', async () => {
		// Repro: cleared metadata cache + cryptic provider `name`. The
		// reverse cache miss + title-match cache miss + slug skip + 0-hit
		// text search should NOT be terminal — the resolver calls the
		// enrichment IPC, which resolves from the show id rather than
		// from the stub name.
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
		// bypassCache=true: step 0 already read + rejected the reverse row, so the
		// backend enrichment must skip its reverse-cache fast path (else a
		// count-rejected id round-trips back).
		expect(mockedResolveAllmanga).toHaveBeenCalledWith('ReooPAxPMsHM4KPMY', true);
	});

	it('returns null when text search and provider enrichment both miss', async () => {
		// Worst case: title-search empty AND backend enrichment also
		// finds no Kitsu match (shows the provider indexes that Kitsu
		// doesn't carry at all). Resolver returns null; the home page
		// renders the bare provider title and routes the resume card
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
		// The Love Live movie's provider show_id was poisoned to point at the
		// YOASOBI "Idol" music video (Kitsu subtype `music`, 1 ep). Music never
		// exists on the provider, so step 0 must drop the row and re-resolve — the
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
		// The eviction names the id it judged: a mapping a play stored
		// while the detail was in flight is not the one judged.
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('9mJyPki2Hm4NmSrhG', '47328');
		expect(mockedSearch).toHaveBeenCalled();
	});

	it('re-resolves (does not delete) a reverse-map binding whose title grossly mismatches', async () => {
		// Non-music poison: an informative hsts title bound to an unrelated Kitsu
		// entry. The title tripwire re-resolves WITHOUT deleting — the corrected
		// mapping is re-PUT by the resolution, so the row heals without risking a
		// valid binding the heuristic merely guessed wrong.
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
		expect(mockedAllmangaDelete).not.toHaveBeenCalled();
	});

	it('keeps a reverse-map binding whose provider title is a plausible typo', async () => {
		// Guard must not over-reject: "Nato: Shippuuden" (the provider typo) shares
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

	it('awaits the reverse-map eviction before reaching provider enrichment', async () => {
		// Race guard: the backend enrichment endpoint (resolve_allmanga_show_id)
		// reads the reverse cache first, so the eviction DELETE must complete
		// before enrichment runs — otherwise the poisoned id comes straight back.
		// Music is the only binding we delete, so it drives this race: a music
		// reverse-map hit → evict; search misses → enrichment.
		const preliminary = resolveHistoryEntry(
			{ id: 'vDTSJHSpYnrkZnAvG', ep_no: '150', title: 'Nato: Shippuuden (500 episodes)' },
			null
		);
		mockedAllmangaMap.mockResolvedValue('wrong-kitsu');
		mockedDetail.mockResolvedValue({
			...stubKitsu('wrong-kitsu', 'Some Music Video', 1),
			subtype: 'music'
		});
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([]); // title-search misses → reach enrichment

		let deleteResolved = false;
		let resolveDelete!: () => void;
		mockedAllmangaDelete.mockImplementation(
			() =>
				new Promise<void>((res) => {
					resolveDelete = () => {
						deleteResolved = true;
						res();
					};
				})
		);
		let enrichmentSawDeleteDone: boolean | null = null;
		mockedResolveAllmanga.mockImplementation(async () => {
			enrichmentSawDeleteDone = deleteResolved;
			return stubKitsu('11061', 'Naruto: Shippuuden', 500);
		});

		const p = resolveKitsuMatch(preliminary);
		await new Promise((r) => setTimeout(r, 0)); // let the resolver park on the awaited delete
		resolveDelete();
		const got = await p;

		expect(got?.id).toBe('11061');
		expect(mockedAllmangaDelete).toHaveBeenCalledWith('vDTSJHSpYnrkZnAvG', 'wrong-kitsu');
		// Enrichment must have observed the eviction as already committed.
		expect(enrichmentSawDeleteDone).toBe(true);
	});
});

// A row records the Kitsu id of the show played, and Kitsu can delete
// that entry. Its absence is an answer about the id — a 404 or 410,
// which the backend passes on as an upstream error carrying the status
// — and the row is then matched the way a row with no recorded id is.
// Any other failure says nothing about the id, and the row answers no
// show for this load rather than a guess.
describe('a recorded id Kitsu cannot read', () => {
	const recordedRow = () =>
		resolveHistoryEntry(
			{ id: 'hianime:cowboy-bebop-1', ep_no: '3', title: 'Cowboy Bebop', kitsu_id: '999' },
			null
		);

	it.each([404, 410])('matches the row anew when Kitsu answers %i for it', async (status) => {
		mockedDetail.mockImplementation(async (id) => {
			if (id === '999') throw { kind: 'upstream', status, key: 'error.network.upstream' };
			return stubKitsu(id, 'Cowboy Bebop', 26);
		});
		mockedSearch.mockResolvedValue([stubKitsu('1', 'Cowboy Bebop', 26)]);

		const got = await resolveKitsuMatch(recordedRow());

		expect(got?.id).toBe('1');
		expect(mockedSearch).toHaveBeenCalled();
	});

	it.each([
		['a provider 503', { kind: 'upstream', status: 503, key: 'error.network.upstream' }],
		['a rate limit', { kind: 'upstream', status: 429, key: 'error.network.upstream' }],
		['an unreachable network', { kind: 'network', key: 'error.network.unreachable' }],
		['a timeout', { kind: 'timeout', key: 'error.scraper.timeout' }],
		['a bodiless gateway error', { kind: 'http', status: 502 }]
	])('answers no show on %s, and matches nothing', async (_label, failure) => {
		mockedDetail.mockRejectedValue(failure);
		mockedSearch.mockResolvedValue([stubKitsu('1', 'Cowboy Bebop', 26)]);

		const got = await resolveKitsuMatch(recordedRow());

		expect(got).toBeNull();
		expect(mockedSearch).not.toHaveBeenCalled();
		expect(mockedAllmangaMap).not.toHaveBeenCalled();
	});
});

// A play started from a Continue card records the card's Kitsu id on
// the row, and the row is matched by that id from then on. Only a match
// the user stands behind may be recorded: the id the row recorded, or a
// mapping a real play stored. A guess — a remembered title match, a
// search pick, an unplayed mapping — stays off the row, so the row can
// still be matched again.
describe('which Continue matches a play may record', () => {
	const legacyRow = () =>
		resolveHistoryEntry({ id: 'hianime:cowboy-bebop-1', ep_no: '3', title: 'Cowboy Bebop' }, null);

	it('records the id the row recorded', async () => {
		mockedDetail.mockResolvedValue(stubKitsu('49877', 'Seitokai ni mo Ana wa Aru!', 12));
		const got = await resolveKitsuMatchWithTrust(
			resolveHistoryEntry({ id: 'hianime:x-1', ep_no: '1', title: 'X', kitsu_id: '49877' }, null)
		);
		expect(got.match?.id).toBe('49877');
		expect(got.trusted).toBe(true);
	});

	it('records a mapping a play stored', async () => {
		mockedAllmangaMap.mockResolvedValue('1');
		mockedDetail.mockResolvedValue(stubKitsu('1', 'Cowboy Bebop', 26));
		mockedPlayed.mockResolvedValue('1');

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(true);
	});

	it('does not record a mapping no play stored', async () => {
		mockedAllmangaMap.mockResolvedValue('1');
		mockedDetail.mockResolvedValue(stubKitsu('1', 'Cowboy Bebop', 26));
		mockedPlayed.mockResolvedValue(null);

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});

	it('does not record a mapping a play replaced after it was read', async () => {
		// Guess 1 was read and fetched; a play stored 2 before the
		// played read ran. The mark vouches for 2, not for the 1 this
		// load holds, which stays a guess.
		mockedAllmangaMap.mockResolvedValue('1');
		mockedDetail.mockResolvedValue(stubKitsu('1', 'Cowboy Bebop', 26));
		mockedPlayed.mockResolvedValue('2');

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});

	it('does not record a mapping whose played read fails', async () => {
		mockedAllmangaMap.mockResolvedValue('1');
		mockedDetail.mockResolvedValue(stubKitsu('1', 'Cowboy Bebop', 26));
		mockedPlayed.mockRejectedValue(new Error('backend down'));

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});

	it('does not record a remembered title match', async () => {
		mockedGetMatch.mockResolvedValue('1');
		mockedDetail.mockResolvedValue(stubKitsu('1', 'Cowboy Bebop', 26));

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});

	it('does not record a search pick', async () => {
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('1', 'Cowboy Bebop', 26)]);

		const got = await resolveKitsuMatchWithTrust(legacyRow());

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});

	it('does not record the match a row whose recorded id is gone falls back to', async () => {
		mockedDetail.mockImplementation(async (id) => {
			if (id === '999') throw { kind: 'upstream', status: 404, key: 'error.network.upstream' };
			return stubKitsu(id, 'Cowboy Bebop', 26);
		});
		mockedGetMatch.mockResolvedValue(null);
		mockedSearch.mockResolvedValue([stubKitsu('1', 'Cowboy Bebop', 26)]);

		const got = await resolveKitsuMatchWithTrust(
			resolveHistoryEntry(
				{ id: 'hianime:cowboy-bebop-1', ep_no: '3', title: 'Cowboy Bebop', kitsu_id: '999' },
				null
			)
		);

		expect(got.match?.id).toBe('1');
		expect(got.trusted).toBe(false);
	});
});
