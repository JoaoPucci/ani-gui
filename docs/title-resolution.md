# Title resolution and the cross-API bridge

`ani-gui` reads from five catalogues that don't share an id space:

- **Kitsu** (REST/JSON:API) — discovery surface (search, trending fallback, top rated, recently released, detail pages, episode metadata).
- **AniList** (GraphQL) — recency-weighted "Trending Now" row and banner backfill when Kitsu's banner is null.
- **anidb.app** — the streaming catalogue the backend resolves playback against first.
- **hianime** — the streaming catalogue the backend resolves against when the walk moves on from anidb.app (unreachable, refusing or rate-limiting, a page the app cannot read, or a background request its gate turned away), and first for a show it was found on while that memory lasts — the availability record's lifetime: about a day from the last resolve that found the show there — a play, a download, a hand-off, one served from the resolution cache, or the background warm a detail page runs on opening for the episode its Play button targets, or with resolution caching on for every aired episode in view — or thirty days for a finished show a probe alone found and nothing resolved since (see [Providers and failover](./architecture.md#providers-and-failover)).
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
       │     aniskip      │                │    stream provider   │
       │   (mal_id, ep)   │                │  (canonical title +  │
       │  → skip times    │                │   alt titles → hits  │
       │                  │                │   → probe episode    │
       │                  │                │   lists → pick)      │
       └──────────────────┘                └──────────────────────┘
```

Four distinct lookups, each with its own gotchas:

1. **Kitsu → provider title match** — the native walk searches the provider's catalogue: anidb.app's browse page, or hianime's search page when the walk moved on from anidb.app — unreachable, refusing or rate-limiting the request, answering a page the app cannot read, or its own gate turning a background request away — or when the show's still-live availability record names hianime. Kitsu canonical titles (often the licensed English form) and the provider's index don't always agree, so the bridge tries the canonical first and falls back to romanized Japanese, native script, and known synonyms before giving up.
2. **Candidate disambiguation** — multiple browse hits can match the same query string ("Gintama" returns the series and its movies in the provider's own ranking). The browse page carries titles only, so the picker probes each considered hit's episode list — bounded to the first few hits, since real queries put the right show near the top and every probe is an upstream request — and Kitsu's authoritative `episode_count` picks the candidate whose count is closest.
3. **Kitsu ↔ MAL** — neither Kitsu's id nor the provider's slug matches MAL's. Kitsu publishes a mappings endpoint that exposes the third-party ids it knows about; the backend fetches `kitsu/anime/:id?include=mappings` and walks the included documents for the MyAnimeList row.
4. **MAL → aniskip / AniList** — once the MAL id is in hand, aniskip and AniList's `Media(idMal:)` query are direct lookups.

## Title resolution: Kitsu → the provider

When the user clicks an episode, the backend builds a list of search terms from the Kitsu metadata in priority order and feeds them to the provider's search in turn:

1. Canonical title.
2. `titles.en_jp` (romanized Japanese).
3. `titles.ja_jp` (native script).
4. `titles.en` / `titles.en_us` (English alternates).

Empty / whitespace-only titles are skipped and exact-string duplicates are deduped, so the backend never makes a redundant provider query. A pool whose pick is rejected keeps the walk going — the next alias may carry the real show — while an upstream refusal (the provider's protection page) stops it: a block on one query blocks them all, and each further request deepens the hole.

## Disambiguation by episode count

The picker probes at most the first few browse hits via the episodes endpoint and scores each by the distance between its episode count and Kitsu's authoritative `episode_count`. The best distance wins, rejected outright when it exceeds the tolerance (`max(3, expected / 10)` — long-running shows get proportional slack, short shows a hard floor); ties prefer an exact case-insensitive title match on the search term. The same picker is used by:

- **The play path** — so clicking "play" lands on the right show even when the provider ranks a movie or side story first.
- **The availability probe** — so the home / detail page's "is this in the catalogue?" gate matches what the play path would do.
- **The download path** — so a download started from the player picks the same show the player was streaming.

When Kitsu's episode count is unknown (rare, but happens for upcoming shows), an exact title match wins, else the provider's own first hit stands. The frontend treats this as a soft signal and still renders the card; the lazy click path will surface a real error if the bridge picked wrong.

Only a walk in which every search completed and nothing matched counts as evidence of absence — that is the one verdict the availability cache may persist as a negative row. Transport failures, upstream refusals, and failed probes are transient and write nothing, so a real show can't hide behind the negative TTL.

## Episode caps

The picked show's episode list arrives with the probe, so the availability cap is the highest listed episode number, normalised to the entry's own numbering — a continuation cour on anidb.app lists its episodes with the franchise offset, which the cap subtracts — with no second fetch and no approximation. The cap is an integer on both providers. anidb.app also keeps a fractional display tag on a recap row (`number2`, `1061.5` for a One Piece recap), and the resolver surfaces those as playable extras beside the cap rather than inside it; hianime lists no such tags. The two number differently: anidb.app counts continuously across a franchise's seasons, hianime within each season-split entry — which is why the cap is stamped per provider and never compared across them.

## Kitsu → MAL via the mappings endpoint

The Kitsu API exposes its known third-party ids on the `mappings` relationship of an anime resource. The backend queries `GET /anime/:id?include=mappings` and walks the `included` documents for one whose `attributes.externalSite` is `"myanimelist/anime"`; its `attributes.externalId` is the MAL id.

The mappings response is cached in `meta_cache` indefinitely — Kitsu's mapping table doesn't move once a show has shipped. A miss on `MAL` is itself cached (as `None`), so shows that aren't on MAL don't get re-probed on every page visit.

## What this enables

- **Trending Now** uses AniList's `TRENDING_DESC` sort, then bridges each MAL id back to a Kitsu id so the rest of the renderer can treat the row uniformly with Kitsu-sourced rows.
- **Banner backfill** — when the detail page sees a null `coverImage` from Kitsu, it bridges to MAL and asks AniList for `bannerImage`. Roughly half of any week's currently-airing top 20 shows hit this fallback path.
- **aniskip** lookups need the MAL id to query, and the same Kitsu→MAL mapping is reused.
- **Availability** can answer "is this in the catalogue in the requested mode?" by running the same walk the play path would and caching the verdict per `(kitsu_id, mode)`, naming the provider whose catalogue answered.
- **Continue Watching** maps history rows back to Kitsu: rows key on the provider's show key (below), whose slug's hyphenated words are the show's own title — the reverse-resolver reads the key, searches Kitsu with the words directly, and persists the `(show key → kitsu_id)` mapping. When two providers have each left a row for the same show, the detail page resumes from the one with the latest watched-at stamp.

## Failure modes the bridge tolerates

- **Kitsu has no MAL mapping** — aniskip lookup returns an empty list (the player just doesn't show the skip button); banner backfill falls through to the blurred-poster placeholder.
- **The primary provider is unreachable** — the walk runs against the next provider. A miss reached there is persisted as that provider's negative verdict, naming it, and is served only while that provider's gate is answering and the primary's gate is refusing — a breaker open for its cooldown, or a rate-limit pause running for its advertised window; both end on the clock, with nothing having to prove the primary is back, and at the window's end the row stops standing and the probe runs again.
- **The provider has no candidate matching any title** — the play path returns `NoResults`; the frontend renders an "isn't in the streaming catalogue" overlay instead of a cryptic backend error. A provider that answered ends the walk — the miss is not retried on the next provider — unless a positive availability record put that provider first, in which case its miss is set aside and the rest of the order is asked, the record proving the show and not every episode.
- **The picker can't disambiguate** — exact-title-or-first-hit fallback. This is the worst case for correctness, but it's still a real entry on the provider; the user sees a sub-show rather than no show. They can pick the right one manually from search.
- **A cached play-resolution URL stops working** — the silent retry path evicts the cached row and re-resolves once before surfacing an error to the user.

## Show keys

Every store a resolve stamps holds the show key the resolve produced, as a string: history rows, the numbering sidecar beside the history file, the watched-at stamps and the reverse mapping key on it, and the resolution-cache row — keyed on the request itself — carries it in its value. anidb's key is the bare slug (`one-piece-69`), which is what every row written before there were two providers already holds, so nothing migrates. Any other provider's key carries its label as a prefix (`hianime:cowboy-bebop-1281`). The read side parses the string back: a known label names its provider; anything else — including the allanime-era ids that predate slugs — is anidb's. The renderer treats the id as opaque except for that label, which it reads to say which provider's title a title-match cache key is for: the title-match cache is keyed per provider, so two providers naming different shows identically cannot read or overwrite each other's mapping.

The cross-cour guard on the reverse mapping write compares the provider's title against Kitsu's slug convention, and it guards every provider's ids: the resolve carries no identity the guard could defer to — no Kitsu or MyAnimeList id, only the title, year and count the picker scored — so a wrongly picked sibling cour on any provider would otherwise persist the caller's Kitsu id under its slug, the row Continue Watching and resume then read. Lifting the guard for a provider wants identity provenance the resolve does not carry yet.
