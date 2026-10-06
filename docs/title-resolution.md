# Title resolution and the cross-API bridge

`ani-gui` reads from five catalogues that don't share an id space:

- **Kitsu** (REST/JSON:API) — discovery surface (search, trending fallback, top rated, recently released, detail pages, episode metadata).
- **AniList** (GraphQL) — recency-weighted "Trending Now" row, banner backfill when Kitsu's banner is null, and episode stills Kitsu lacks.
- **anidb.app** — the streaming catalogue the backend resolves playback against first.
- **hianime** — the streaming catalogue the backend resolves against when the walk moves on from anidb.app (unreachable, refusing or rate-limiting, a page the app cannot read, a background request its gate turned away, or an answer that settles nothing — the show found with nothing said about the audio asked for, or a denial of a show a live record remembers it carrying), and first for a show it was found on while that memory lasts — the availability record's lifetime: at most a day from the last resolve that found the show there — a play, a download, a hand-off, or the background warm a page runs as it follows what is in view: the detail page for the episode its Play button targets, the play page for the next episode, or with resolution caching on for every aired episode in view, again as the grid or strip is paged — or, for a show a probe alone found and nothing resolved since, at most a day for an ongoing one and thirty days for a finished one; a play, hand-off or page warm served from the resolution cache leaves a live record as it is and writes a record with the ongoing window again once it has lapsed, and a download never reads that cache (see [Providers and failover](./architecture.md#providers-and-failover)).
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
                  │       (kitsu id ↔ mal / anilist id)
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

1. **Kitsu → provider title match** — the native walk searches the provider's catalogue: anidb.app's browse page, or hianime's search page when the walk moved on from anidb.app — unreachable, refusing or rate-limiting the request, answering a page the app cannot read, its own gate turning a background request away, or answering without settling the question: the show found with every sampled row silent about the requested audio, or a denial of a show a live positive row remembers it carrying, which the row outranks until the rest of the order has been asked — or when the show's still-live availability record names hianime. Kitsu canonical titles (often the licensed English form) and the provider's index don't always agree, so the bridge tries the canonical first and falls back to romanized Japanese, native script, and known synonyms before giving up.
2. **Candidate disambiguation** — multiple browse hits can match the same query string ("Gintama" returns the series and its movies in the provider's own ranking). The browse page carries titles only, so the picker probes each considered hit's episode list — bounded to the first few hits, since real queries put the right show near the top and every probe is an upstream request — and Kitsu's authoritative `episode_count` picks the candidate whose count is closest.
3. **Kitsu ↔ MAL / AniList** — neither Kitsu's id nor the provider's slug matches MAL's or AniList's. Kitsu publishes a mappings endpoint that exposes the third-party ids it knows about; the backend fetches `kitsu/anime/:id?include=mappings` and walks the included documents for the MyAnimeList and AniList rows.
4. **MAL / AniList → aniskip / AniList** — once the ids are in hand, aniskip (MAL id only) and AniList's `Media(idMal:)` query are direct lookups. The banner and episode-thumbnail backfills fall back to AniList's `Media(id:)` when Kitsu carries the AniList mapping but no MAL one, which is common for fresh seasonal shows.

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

One Kitsu entry can be several shows on the provider. Steel Ball Run is twelve episodes on Kitsu, while hianime lists its March premiere as a one-episode show of its own and the weekly run as "… 2nd Stage", numbered from 1. The picker recognises the parts among the candidates it already probed — a bare title and the same title followed by a short part marker ("2nd Stage", "Part 3", "Season 2", "II"), every part's own premiere year equal to Kitsu's — and stitches them when together they explain the expected count better than any single candidate, either within the tolerance or short of it as an airing entry is. Where a single candidate already sits within the tolerance, the count cannot tell a finished split (1 + 11 against 12, the later part one off) from a cour and its same-year sequel (12 + 1 against 13, the bare title one off), so the near miss decides: the parts are stitched over it when it is one of the later parts, never when it is the bare title — a bare title that fits alone is the entry. The entry may itself be a later part: Kitsu keeps "X Season 2" as its own entry, and asked for it, the provider's same-year "X" is the season before. Any candidate that a title the entry goes by — its canonical title or any alias — names a later part of is dropped from the pool before probing, so it neither heads a chain (a finished 12-episode "X" beside a "X Season 2" five episodes into an announced 24 would otherwise stitch, 12 + 5 short of 24 like an airing split, and play Season 2's first twelve episodes from the first season), nor wins alone when its count happens to fit, nor stands in when the provider has not listed the later part yet. Steel Ball Run's Kitsu titles name no part, so its pool keeps both parts. The reverse has no title to go on: asked for a bare "X" that has finished but that the provider lists short, beside a same-year "X Season 2" that Kitsu keeps as its own entry, the two can still stitch when together they fit the count better than "X" alone, since nothing in the titles or counts tells that from a real split. Beyond that check the parts' titles are not compared with the one searched, so a provider that spells the parts differently from the title searched (an English title against Kitsu's romaji, say) stitches the same way, and an airing split stays stitched the week its later part comes within the tolerance. A bare title whose own listing is numbered as a continuation is never stitched: played alone, its key already carries that offset and its history rows speak those numbers. Stitching makes one listing in the entry's numbering: each part normalised to its own numbers and numbered on from where the part before it ends, every row keeping the episode id it streams from, and every row taking its position across the parts as its slot, with its episode identity in the tag where the two differ — as a single listing's rows do. The first part's id is the show's identity, the key history and every cache file the plays under, so Kitsu's episode 2 of Steel Ball Run streams the 2nd Stage's episode 1 and records as episode 2 under the first part's key. A sibling that lists nothing yet, one from another year, a gap in the part numbers or a candidate whose probe failed leaves the pick as it was; so does a single candidate the rules above leave standing, which keeps a provider entry spanning several Kitsu entries on its continuation offset (below).

When Kitsu's episode count is unknown (rare, but happens for upcoming shows), an exact title match wins, else the provider's own first hit stands. The frontend treats this as a soft signal and still renders the card; the lazy click path will surface a real error if the bridge picked wrong.

Two verdicts of absence are the ones the availability cache may persist as a negative row: a walk in which every search completed and nothing matched, and the probe's answer that a show it found is listed without the requested mode — a listing with no rows, or a sampled row that lacks the mode. Transport failures, upstream refusals, failed probes, and a mode nobody answered for — every sampled row saying nothing either way — are transient and write nothing, so a real show can't hide behind the negative TTL.

## Episode caps

The picked show's episode list arrives with the probe, so the availability cap is the highest listed episode number, normalised to the entry's own numbering — a continuation cour on anidb.app lists its episodes with the franchise offset, which the cap subtracts — with no second fetch and no approximation. The cap is an integer on both providers. Both keep a fractional display tag on a recap row (`number2`: `1061.5` for a One Piece recap on anidb.app, a `7.5` the listing numbers a recap with on hianime, where the row's position is its slot and the site's number its tag), and the resolver surfaces those as playable extras beside the cap rather than inside it. The two number differently: anidb.app counts continuously across a franchise's seasons, hianime within each season-split entry — which is why the cap is stamped per provider and never compared across them. A stitched pick's cap is counted across its parts, in Kitsu's numbering.

## Kitsu → MAL and AniList via the mappings endpoint

The Kitsu API exposes its known third-party ids on the `mappings` relationship of an anime resource. The backend queries `GET /anime/:id?include=mappings` and walks the `included` documents for the ones whose `attributes.externalSite` is `"myanimelist/anime"` or `"anilist/anime"`; their `attributes.externalId` values are the MAL and AniList ids.

The mappings response is not cached on its own. The detail row with its backfilled banner and the episode-thumbnail map are keyed by the Kitsu id, so a hit skips the mappings round-trip along with the AniList lookup it fed, and the row stays unambiguous whichever id space that lookup used. aniskip caches its intervals by MAL id and episode, so it reads the mappings on every request.

## What this enables

- **Trending Now** uses AniList's `TRENDING_DESC` sort, then bridges each MAL id back to a Kitsu id so the rest of the renderer can treat the row uniformly with Kitsu-sourced rows.
- **Banner backfill** — when the detail page sees a null `coverImage` from Kitsu, it asks AniList for `bannerImage`, by MAL id when Kitsu has one and by AniList id otherwise. Roughly half of any week's currently-airing top 20 shows hit this fallback path.
- **Episode-thumbnail backfill** — episodes Kitsu has no still for take one from AniList's `streamingEpisodes`, reached the same way: MAL id first, AniList id when there is no MAL mapping.
- **aniskip** lookups need the MAL id to query, and the same Kitsu→MAL mapping is reused.
- **Availability** can answer "is this in the catalogue in the requested mode?" by running the same walk the play path would and caching the verdict per `(kitsu_id, mode)`, naming the provider whose catalogue answered.
- **Continue Watching** maps history rows back to Kitsu: rows key on the provider's show key (below), whose slug's hyphenated words are the show's own title — the reverse-resolver reads the key, searches Kitsu with the words directly, and persists the `(show key → kitsu_id)` mapping. When two providers have each left a row for the same show, the detail page and the strip pick the same row. A watch writes its moment beside its history row and in the cache, and a row ranks by the later of the two, so a cache that refused the stamp still leaves the row ranked as the watch it was; the latest moment wins, a row with one beats a row without, and when the moments do not separate two rows the further progress does, counted in Kitsu's numbering, since two providers' rows can count from different offsets.

## Failure modes the bridge tolerates

- **Kitsu has no MAL mapping** — aniskip lookup returns an empty list (the player just doesn't show the skip button). The banner and episode-thumbnail backfills use the AniList mapping instead when there is one; with neither mapping, the banner falls through to the blurred-poster placeholder and episodes keep their placeholder stills.
- **The primary provider is unreachable** — the walk runs against the next provider. A clean catalogue miss reached there — every search completed and nothing matched; an episode dead end on a show that was found persists nothing — is persisted as that provider's negative verdict, naming it, as is the availability probe's answer that the show is listed without the requested mode — unless a live positive row remembers the unreachable provider, in which case the miss is not the verdict at all — the caller sees that provider's unavailability, and nothing persists, since the row's own provider has not denied the show and an unrelated absence must not overwrite its evidence — and a persisted one is served only while that provider's gate has been seen answering since it last failed and the primary's gate is refusing — a breaker open for its cooldown, or a rate-limit pause running for its advertised window; both end on the clock, with nothing having to prove the primary is back, and at the window's end the row stops standing and the probe runs again.
- **The provider has no candidate matching any title** — the play path returns `NoResults`; the frontend renders an "isn't in the streaming catalogue" overlay instead of a cryptic backend error. A provider that answered conclusively ends the walk — the miss is not retried on the next provider — unless a positive availability record put that provider first, in which case its miss is set aside and the rest of the order is asked, the record proving the show and not every episode. A provider whose answer settles nothing — the show found, every sampled episode row silent about the requested mode — does not end it; the walk moves on.
- **The picker can't disambiguate** — exact-title-or-first-hit fallback. This is the worst case for correctness, but it's still a real entry on the provider; the user sees a sub-show rather than no show. They can pick the right one manually from search.
- **A cached play-resolution URL stops working** — the silent retry path evicts the cached row and re-resolves once before surfacing an error to the user.

## Show keys

Every store a resolve stamps holds the show key the resolve produced, as a string: history rows, the numbering sidecar beside the history file, the watched-at stamps and the reverse mapping key on it, and the resolution-cache row — keyed on the request itself — carries it in its value. anidb's key is the bare slug (`one-piece-69`), which is what every row written before there were two providers already holds, so nothing migrates. Any other provider's key carries its label as a prefix (`hianime:cowboy-bebop-1281`). The read side parses the string back: a known label names its provider; anything else — including the allanime-era ids that predate slugs — is anidb's. The renderer treats the id as opaque except for that label, which it reads to say which provider's title a title-match cache key is for: the title-match cache is keyed per provider, so two providers naming different shows identically cannot read or overwrite each other's mapping.

The cross-cour guard on the reverse mapping write compares the provider's title against Kitsu's slug convention, and it guards every provider's ids: the resolve carries no identity the guard could defer to — no Kitsu or MyAnimeList id, only the title, year and count the picker scored — so a wrongly picked sibling cour on any provider would otherwise persist the caller's Kitsu id under its slug, the row Continue Watching and resume then read. Lifting the guard for a provider wants identity provenance the resolve does not carry yet.
