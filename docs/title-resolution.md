# Title resolution and the cross-API bridge

`ani-gui` reads from four catalogues that don't share an id space:

- **Kitsu** (REST/JSON:API) — discovery surface (search, trending fallback, top rated, recently released, detail pages, episode metadata).
- **AniList** (GraphQL) — recency-weighted "Trending Now" row and banner backfill when Kitsu's banner is null.
- **anidb.app** — the streaming catalogue the backend resolves playback against.
- **aniskip** (REST) — community OP / ED skip-time intervals, keyed by MyAnimeList id.

Every interaction other than discovery has to find the same show in two or more of these. None of them carry the others' ids, and any given anime may appear in some but not others (aniskip in particular is sparse). This document describes how the backend bridges them and what the cache stores in the process.

## The four bridges

```
                       ┌──────────────────────────────────────────┐
                       │                                          │
                       │             Discovery surface            │
                       │                                          │
                       │   ┌──────────────┐    ┌──────────────┐   │
                       │   │   AniList    │    │    Kitsu     │   │
                       │   │  (trending)  │    │ (everything  │   │
                       │   │              │    │   else)      │   │
                       │   └──────┬───────┘    └──────┬───────┘   │
                       │          │                   │           │
                       │     mal id only         kitsu id (canonical
                       │          │              for the renderer)
                       └──────────┼───────────────────┼───────────┘
                                  │                   │
                  ┌───────────────┴───────────────────┘
                  │       Kitsu mappings endpoint
                  │       (kitsu id ↔ mal id)
                  ▼
       ┌──────────────────┐                ┌──────────────────────┐
       │     aniskip      │                │       anidb.app      │
       │   (mal_id, ep)   │                │  (canonical title +  │
       │  → skip times    │                │   alt titles → hits  │
       │                  │                │   → probe episode    │
       │                  │                │   lists → pick)      │
       └──────────────────┘                └──────────────────────┘
```

Four distinct lookups, each with its own gotchas:

1. **Kitsu → provider title match** — the native walk searches anidb.app's browse page. Kitsu canonical titles (often the licensed English form) and the provider's index don't always agree, so the bridge tries the canonical first and falls back to romanized Japanese, native script, and known synonyms before giving up.
2. **Candidate disambiguation** — multiple browse hits can match the same query string ("Gintama" returns the series and its movies in the provider's own ranking). The browse page carries titles only, so the picker probes each considered hit's episode list — bounded to the first few hits, since real queries put the right show near the top and every probe is an upstream request — and Kitsu's authoritative `episode_count` picks the candidate whose count is closest.
3. **Kitsu ↔ MAL** — neither Kitsu's id nor the provider's slug matches MAL's. Kitsu publishes a mappings endpoint that exposes the third-party ids it knows about; the backend fetches `kitsu/anime/:id?include=mappings` and walks the included documents for the MyAnimeList row.
4. **MAL → aniskip / AniList** — once the MAL id is in hand, aniskip and AniList's `Media(idMal:)` query are direct lookups.

## Title resolution: Kitsu → anidb.app

When the user clicks an episode, the backend builds a list of search terms from the Kitsu metadata in priority order and feeds them to the provider's browse search in turn:

1. Canonical title.
2. `titles.en_jp` (romanized Japanese).
3. `titles.ja_jp` (native script).
4. `titles.en` / `titles.en_us` (English alternates).

Empty / whitespace-only titles are skipped and exact-string duplicates are deduped, so the backend never makes a redundant provider query. A pool whose pick is rejected keeps the walk going — the next alias may carry the real show — while an upstream refusal (the provider's protection page) stops it: a block on one query blocks them all, and each further request deepens the hole.

## Disambiguation by episode count

The picker probes at most the first few browse hits via the episodes endpoint and scores each by the distance between its episode count and Kitsu's authoritative `episode_count`. The best distance wins, rejected outright when it exceeds the tolerance (`max(3, expected / 10)` — long-running shows get proportional slack, short shows a hard floor); ties prefer an exact case-insensitive title match on the search term. The same picker is used by:

- **The play path** — so clicking "play" lands on the right show even when the provider ranks a movie or side story first.
- **The availability probe** — so the home / detail page's "is this on anidb.app?" gate matches what the play path would do.
- **The download path** — so a download started from the player picks the same show the player was streaming.

When Kitsu's episode count is unknown (rare, but happens for upcoming shows), an exact title match wins, else the provider's own first hit stands. The frontend treats this as a soft signal and still renders the card; the lazy click path will surface a real error if the bridge picked wrong.

Only a walk in which every search completed and nothing matched counts as evidence of absence — that is the one verdict the availability cache may persist as a negative row. Transport failures, upstream refusals, and failed probes are transient and write nothing, so a real show can't hide behind the negative TTL.

## Episode caps

The picked show's episode list arrives with the probe, so the availability cap is the exact highest listed episode number — no second fetch and no approximation. The provider lists integer episodes only; recap half-episodes don't exist in its numbering.

## Kitsu → MAL via the mappings endpoint

The Kitsu API exposes its known third-party ids on the `mappings` relationship of an anime resource. The backend queries `GET /anime/:id?include=mappings` and walks the `included` documents for one whose `attributes.externalSite` is `"myanimelist/anime"`; its `attributes.externalId` is the MAL id.

The mappings response is cached in `meta_cache` indefinitely — Kitsu's mapping table doesn't move once a show has shipped. A miss on `MAL` is itself cached (as `None`), so shows that aren't on MAL don't get re-probed on every page visit.

## What this enables

- **Trending Now** uses AniList's `TRENDING_DESC` sort, then bridges each MAL id back to a Kitsu id so the rest of the renderer can treat the row uniformly with Kitsu-sourced rows.
- **Banner backfill** — when the detail page sees a null `coverImage` from Kitsu, it bridges to MAL and asks AniList for `bannerImage`. Roughly half of any week's currently-airing top 20 shows hit this fallback path.
- **aniskip** lookups need the MAL id to query, and the same Kitsu→MAL mapping is reused.
- **Availability** can answer "is this on anidb.app in the requested mode?" by running the same walk the play path would and caching the verdict per `(kitsu_id, mode)`.
- **Continue Watching** maps history rows back to Kitsu: rows key on the provider slug, whose hyphenated words are the show's own title — the reverse-resolver searches Kitsu with them directly and persists the `(slug → kitsu_id)` mapping.

## Failure modes the bridge tolerates

- **Kitsu has no MAL mapping** — aniskip lookup returns an empty list (the player just doesn't show the skip button); banner backfill falls through to the blurred-poster placeholder.
- **The provider has no candidate matching any title** — the play path returns `NoResults`; the frontend renders an "isn't on the streaming source" overlay instead of a cryptic backend error.
- **The picker can't disambiguate** — exact-title-or-first-hit fallback. This is the worst case for correctness, but it's still a real entry on the provider; the user sees a sub-show rather than no show. They can pick the right one manually from search.
- **A cached play-resolution URL stops working** — the silent retry path evicts the cached row and re-resolves once before surfacing an error to the user.
