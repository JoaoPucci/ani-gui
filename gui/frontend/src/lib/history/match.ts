/**
 * Resolves a `ResumeTarget`'s kitsu match against the title-match
 * cache before falling back to a live `kitsuSearch` + `pickKitsuMatch`
 * round-trip. Cache hit → one IPC call (`kitsuAnimeDetail`, also
 * cached). Miss → search + pick + persist, so the next session
 * short-circuits.
 *
 * Errors at any layer fall through to the next strategy and ultimately
 * to `null` — the caller (Continue Watching cards) treats null as "no
 * Kitsu data, render the bare allmanga title as a card".
 */

import {
	allmangaKitsuMapDelete,
	allmangaKitsuMapGet,
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuResolveAllmangaShowId,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type KitsuAnimeRef
} from '$lib/api';
import { cachedBindingVerdict, deriveSlug, pickKitsuMatch, type ResumeTarget } from './resolve';

export async function resolveKitsuMatch(preliminary: ResumeTarget): Promise<KitsuAnimeRef | null> {
	// 0) Reverse-mapping lookup: allmanga show_id → kitsu_id. Recorded
	//    by the backend on every successful play, so once the user
	//    has played a show through the GUI the home-page strip can
	//    skip every other path. Wins over title-match because the
	//    show_id is deterministic — the title is sometimes a typo
	//    (allmanga's "Nato: Shippuuden" for Naruto Shippuuden).
	//
	//    Validate the cached detail's episode_count against the
	//    user's history courSize before accepting — older sessions
	//    may have persisted a wrong mapping (e.g. Burichi/Buriki
	//    fuzzy-matched to Doraemon Movie 14). When the count is
	//    incompatible, fall through to a fresh resolution path.
	if (preliminary.allmangaShowId) {
		try {
			const kitsuId = await allmangaKitsuMapGet(preliminary.allmangaShowId);
			if (kitsuId) {
				try {
					const cached = await kitsuAnimeDetail(kitsuId);
					const verdict = cachedBindingVerdict(cached, preliminary, true);
					if (verdict === 'trust') return cached;
					if (verdict === 'evict') {
						// Provably wrong reverse-map row — a music entry, a gross title
						// mismatch (the Love Live movie's show_id poisoned to the YOASOBI
						// "Idol" MV), or a cross-cour slug mismatch. Self-heal: drop it so
						// it re-resolves on every install without a manual cache wipe.
						// Awaited (not fire-and-forget): the step-4 enrichment endpoint
						// reads this same reverse cache first, so the DELETE must commit
						// before we fall through or a typo title whose search misses gets
						// the just-rejected id straight back. Tolerate a failing delete.
						await allmangaKitsuMapDelete(preliminary.allmangaShowId).catch(() => {});
					}
					// 'evict' / 'reresolve' both fall through to the title-search path.
				} catch {
					// Stale id — fall through to the title-search path.
				}
			}
		} catch {
			// Endpoint unavailable — fall through.
		}
	}

	// 1) Cache lookup. If we've resolved this title→id before, fetch
	//    the (cached, 7d-TTL) detail and short-circuit.
	//
	//    Defense-in-depth: for cour > 1 entries, validate that the
	//    cached anime's slug ends with `-part-N` / `-cour-N` /
	//    `-season-N`. A stale mapping (e.g. from a prior version
	//    where the picker collapsed sequels onto Part 1) returns
	//    Part 1's anime which fails the slug check; we fall through
	//    to the slug-fetch path and let the resolution rebuild.
	//
	//    Same episode-count compatibility check as step 0: if a
	//    poisoned title-match cache row points at an anime whose
	//    count is incompatible with courSize, drop the cached hit
	//    and force a fresh resolution.
	try {
		const cachedId = await kitsuTitleMatchGet(preliminary.searchTitle, preliminary.cour);
		if (cachedId) {
			try {
				const cached = await kitsuAnimeDetail(cachedId);
				if (cachedBindingVerdict(cached, preliminary, false) === 'trust') {
					return cached;
				}
				// Incompatible / implausible / cross-cour → fall through and re-resolve
				// (step 5 re-Puts a corrected title-match row; no title-match delete).
			} catch {
				// Stale id (Kitsu removed the entry) — fall through to a
				// live search and re-cache.
			}
		}
	} catch {
		// Cache backend unavailable — degrade to live search.
	}

	let match: KitsuAnimeRef | null = null;

	// 2) Slug-first for multi-cour entries. Kitsu's `filter[text]`
	//    ranks the most-popular sibling and drops sequels with
	//    Japanese-romanized canonical titles entirely (Stone Ocean
	//    Part 2 is the canonical example: same franchise, different
	//    Kitsu entry, NOT in the text-search response). Our hsts
	//    title slugifies cleanly to Kitsu's URL pattern, so a direct
	//    slug lookup pinpoints the right entry. Single-cour entries
	//    skip this and go straight to the search path — slug-fetching
	//    every Continue Watching row would double the IPC volume on
	//    cold load.
	if (preliminary.cour > 1) {
		const slug = deriveSlug(preliminary.searchTitle);
		if (slug.length >= 4) {
			try {
				match = await kitsuAnimeBySlug(slug);
			} catch {
				// Slug-fetch failure is non-fatal; fall through to search.
			}
		}
	}

	// 3) Live search + pick. Either the slug fallback didn't apply
	//    (cour 1) or it didn't find an entry; let the picker work the
	//    text-search hits.
	if (!match) {
		try {
			const hits = await kitsuSearch(preliminary.searchTitle);
			match = pickKitsuMatch(hits, preliminary);
		} catch {
			return null;
		}
	}

	// 4) Allmanga-aliases enrichment fallback. Reach here when steps
	//    0-3 all whiff — typically because allmanga's primary `name`
	//    is a stub the Kitsu text search can't resolve ("1P" for One
	//    Piece, "Nato: Shippuuden" for Naruto Shippuuden). The backend
	//    fetches allmanga's Show GraphQL, harvests englishName /
	//    nativeName / altNames, retries Kitsu search with each, and
	//    persists the resolved kitsu_id into the reverse cache so
	//    subsequent calls short-circuit through step 0.
	//
	//    Only fires when there's an allmanga show_id to enrich AND
	//    earlier paths already failed — title-search hits skip this
	//    branch entirely (verified by the "skips enrichment" test).
	if (!match && preliminary.allmangaShowId) {
		try {
			// bypassCache: step 0 already read + rejected this show's reverse-cache
			// row (count/music/title guard), so the backend must NOT short-circuit
			// on it again — go straight to the alias walk.
			match = await kitsuResolveAllmangaShowId(preliminary.allmangaShowId, true);
		} catch {
			// Enrichment endpoint failure is non-fatal — fall through
			// to the null return below.
		}
	}

	// 5) Persist on success so the next session bypasses the lookup.
	if (match) {
		try {
			await kitsuTitleMatchPut(preliminary.searchTitle, preliminary.cour, match.id);
		} catch {
			// Cache write failed — non-fatal, callers still get the match.
		}
	}

	return match;
}
