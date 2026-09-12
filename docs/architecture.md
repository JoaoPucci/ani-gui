# Architecture

`ani-gui` is a desktop app that lets you browse and watch anime through a graphical interface. It began as a front end over the [`ani-cli`](https://github.com/pystardust/ani-cli) Bash scraper and now resolves streams itself in Rust. The repository no longer carries the script; the terminal flow is upstream's to provide.

## What gets shipped

A single-window desktop application. Linux: AppImage and `.deb`. Windows: NSIS installer. The user double-clicks an icon and gets a native window. There is no URL to visit, no port to remember, no internet-reachable service.

## Why there is a "backend"

The app talks to three things a browser tab cannot reach on its own:

1. The streaming providers — each sits behind TLS-fingerprinting protection that rejects browser and plain-HTTP clients, so requests go out through a `curl-impersonate` subprocess.
2. The watch-history file — needs filesystem access.
3. Anime stream CDNs — require a `Referer:` header that browser fetch APIs cannot set, and serve segments without permissive CORS.

So the app embeds a Rust backend, bound to `127.0.0.1` on a kernel-assigned port, that orchestrates these pieces. It runs as a sidecar process the desktop shell launches at startup — a localhost daemon, not a server anyone else can reach.

## Components

```
 ┌──────────────────────────────────────────────────────────────┐
 │  ani-gui desktop app                                         │
 │                                                              │
 │  ┌────────────────────┐   fetch()   ┌────────────────────┐   │
 │  │ Renderer (SPA)     │ ──────────► │ Backend (sidecar)  │   │
 │  │ SvelteKit + hls.js │ ◄────────── │ Rust HTTP server   │   │
 │  └─────────┬──────────┘             └─────┬──────────────┘   │
 │            │                              │                  │
 │            │ <video src="http://127.0.0.1:├──► providers    │
 │            │  PORT/s/<token>/...">        │   via curl       │
 │            │                              │                  │
 │            │  bytes streamed via proxy    ├──► Kitsu (REST)  │
 │            └─────────────────────────────►│   AniList (GQL)  │
 │                  Referer + CORS           │                  │
 │                                           ├──► history TSV   │
 │                                           │                  │
 │                                           └──► SQLite +      │
 │                                              image cache     │
 └──────────────────────────────────────────────────────────────┘

```

Three layers, in lockstep:

- **Renderer** — SvelteKit static SPA running inside the desktop shell's web view. Renders the discovery surface, search results, detail pages, and the embedded player (`<video>` + hls.js). Stateless beyond UI state; talks only to the backend.
- **Backend** — Rust crate inside `backend/`. Spawned as a sidecar by the desktop shell at startup. Resolves streams from the providers, fetches metadata from Kitsu/AniList, reads/writes the watch-history file, runs a streaming proxy on a localhost port, and exposes an HTTP API the renderer talks to via `fetch()`.
- **External processes** — `curl-impersonate` for provider requests, `yt-dlp` / `ffmpeg` for downloads, and optionally `mpv` for the "Open in external player" escape hatch.

## Data flow: searching and playing an episode

1. The user types a query into the search bar.
2. The renderer calls `POST /api/kitsu/search`. The backend hits Kitsu and returns matches.
3. The user picks a result; the renderer fetches detail and episode list via `GET /api/kitsu/anime/:id` and `GET /api/kitsu/episodes/:id`.
4. The user clicks an episode. The renderer calls `POST /api/sessions` with the chosen anime + episode. The backend resolves the stream natively against the providers in order. [anidb.app](https://anidb.app) first: it searches the browse page for the title (falling back through every alias), probes candidates' episode lists to pick the right show, fetches the episode's language embeds, and reads the master-playlist URL off the chosen embed page. When the walk moves on from anidb.app — unreachable, refusing or rate-limiting the request, answering a page the parser does not recognise, or its gate turning a background request away — or when the show's availability record remembers hianime, [hianime](https://hianime.at) through the same walk over its own pages: its search page, its per-entry episode list and per-episode server list, and the embed page whose payload carries the master-playlist URL and any sidecar subtitle tracks. Requests go through a `curl-impersonate` subprocess — both providers sit behind TLS-fingerprinting protection that rejects plain HTTP clients. See [Providers and failover](#providers-and-failover).
5. The backend creates a `StreamSession` (UUID, upstream URL, referer, expiry), stores it in memory, and returns a token to the renderer.
6. The renderer mounts `<video>` and points hls.js at `http://127.0.0.1:<port>/s/<token>/master.m3u8`.
7. The streaming proxy fetches the upstream master playlist with the correct `Referer:` header, parses it with `m3u8-rs`, and rewrites every variant + segment URI to flow back through itself with HMAC-signed sub-tokens. CORS headers are added so hls.js inside the webview can consume the rewritten manifest without preflight blocks.
8. Subsequent segment requests follow the same path: hls.js asks the proxy, the proxy asks the upstream with the `Referer:`, bytes stream back.

## Providers and failover

The resolver's walks — search the aliases, pick the candidate by episode count and year, chase the episode to a playable URL — are policy, and they read the same whichever site answers. What differs per site sits behind a `Provider` trait in `backend/src/scraper/`: how a query becomes hits, how a slug becomes an episode list, how an episode becomes a playlist, and what the CDN wants on the request (hianime's playlists are served only with the embed host's origin as `Referer`; anidb.app's need none). A resolved stream carries that context — the master URL, its referer, its sidecar subtitle tracks — and every consumer reads it from there: the proxy session, the resolution cache, downloads, and the external-player and Syncplay handoffs.

Two providers ship: anidb.app, then hianime. Each has an admission gate of its own — a pacer for background probes and a circuit breaker that opens after consecutive failures — so an outage on one never paces or refuses traffic to the other. The failover orchestrator (`backend/src/commands/providers.rs`) runs every walk against the providers in order and moves to the next only when the current one was **unreachable, refusing or broken**: a transport failure, a timeout, a provider-shaped block or rate limit, a background request the gate refused, or a page the parser no longer recognises. A provider that answered — including one that searched and found nothing — ends the walk; a miss on anidb.app is not retried on hianime. A provider that is refusing — its breaker open, or an advertised rate-limit window still running — is skipped while another remains, on background and interactive walks alike; the last one is always tried, since an interactive request may be the trial that closes a breaker, and a click is admitted through a pause regardless. Every attempt but the last is bounded by a 20-second budget under the 60-second resolve deadline, so a stalled outage cannot spend the whole deadline before the fallback is asked. On an interactive walk that skipped a provider for its open breaker, one attempt budget is held back for each skipped provider, so a stalling fallback is cut off at its attempt budget and the skipped provider's half-open trial has its time; a background walk, which never retries, reserves nothing. Each attempt's outcome lands on its own provider's gate.

The order is per request when a show is already known. A positive availability row names the provider whose catalogue carries the show, and a play, a download or a handoff for that show starts from it, so a show hianime listed, in the requested audio, during an anidb.app outage — a positive row says the show and the mode are listed there, not that a stream was played — keeps starting from hianime after anidb.app recovers — its clean miss would otherwise end the walk on a show the row says is there. So does the availability probe's own reprobe of a positive row it does not serve — a resolve's count-less stamp, an approximate count — which would otherwise start from the primary and let its clean miss overwrite the fallback's proof with a negative. The row proves the show and its mode at the show's level, not that every episode has an embed, so a remembered provider's answered miss does not end the walk either: the rest of the order is asked, and two misses then stand against each other by rank, not by order — an answered miss that found the show, an episode dead end, outranks a clean catalogue miss from another provider, so the remembered provider's own dead end is the verdict and nothing persists, while misses of equal rank keep the last answer given. A remembered provider heard from with an episode dead end has not denied the show, so, exactly as while it is unreachable, a clean miss or an absence from the rest of the order is the verdict the caller sees but not one that persists over the row. And on an interactive walk the skip of an open breaker is only the fast path — the gate admits a click through an open breaker as its half-open trial — so when the walk's verdict is negative — every provider tried answered a miss or was unreachable, or the one that answered found the show without the requested mode — the skipped ones are asked before it surfaces: an answer that is not negative is the walk's; a retried provider's own negative answer or miss, being the last answer given, replaces the verdict, author and all — by rank, as between any two misses, so a retried provider's clean catalogue miss does not replace an episode dead end already held; and one still unreachable leaves it standing. Background traffic keeps the skip.

The asymmetry is absence. Two verdicts persist as a negative row — a clean miss, where every search completed and nothing matched, and a show found whose first readable episode lacked the requested audio mode (the probe samples one episode's server list, not the whole show's) — and each is a verdict from **the provider that answered**, which the row names: since a miss does not fail over, a negative row from anidb.app means anidb.app searched and found nothing (or found the show without the mode), not that no provider carries it. A negative row is served — by the page's probe, and by the lists, which hide a finished show on it — only while its provider has been seen answering — its breaker closed by a success, or never opened, and no advertised pause running — and every provider ahead of it in the order is refusing, where refusing means the gate's breaker is open or an advertised rate-limit pause is still running; otherwise the probe runs again and the card renders. The two are different questions: past its cooldown a breaker refuses nobody but is half-open, one trial let through and nothing yet heard, so a row served on that alone would short-circuit the very probe whose trial could fail over; a half-open primary therefore backs nothing of its own, while the fallback's row yields to the trial. So anidb.app's negatives stop standing through anidb.app's own outage once its gate has learned of it — a breaker opens after repeated failures and a pause starts on an advertised rate limit, and until some request has taught the gate that, a negative row is still served without a request being made — which would otherwise hide a show hianime carries. And hianime's negatives, written during that outage so an absent show is not re-walked on every look, stand exactly as long as both halves of the read rule hold — hianime's own gate seen answering and anidb.app's refusing — so a hianime negative stops standing the moment hianime itself opens a breaker or enters a pause, whatever anidb.app's gate says, and does not stand again until a success has closed it; and otherwise it stands for as long as anidb.app's gate refuses: a breaker is open for its cooldown, a minute, and a pause for the window the rate limit advertised, and both close on the clock — nothing has to prove recovery — so at the window's end the next look reprobes rather than serving the row, and the reprobe's own outcome teaches the gate again, since absence on hianime proves nothing about anidb.app. A positive row's proof is guarded the same way from the other side: while the provider the row remembers is unreachable — skipped for refusing, or failed over, and not heard from since — a negative from the rest of the order, whether a clean miss or a show found without the requested mode, is the verdict the caller sees but not one that persists, so the row stands until the remembered provider is reachable again and the next resolve starts from it, rather than a negative from the other catalogue overwriting proof it never contradicted. A remembered provider that answers for itself is heard from, and its own negative persists as before.

The history rows, the numbering sidecar beside the history file, the watched-at stamps and the reverse mapping to Kitsu key on a show key that says whose id it holds — anidb.app's as the bare slug every existing row already holds, hianime's under its label — and the play-resolution row, keyed on the request as the cache section below describes, carries the same key in its value. [`title-resolution.md`](./title-resolution.md#show-keys) describes the format and what reads it.

The survey that chose hianime, and the reasoning behind the shape this section describes, are in [`proposals/additional-providers.md`](./proposals/additional-providers.md) — the proposal as it was written before the work, kept for its survey; this section, not the proposal, describes what shipped.

## Discovery (landing page)

The landing page shows four rows: **Trending Now**, **Popular This Season**, **Top Rated**, **Recently Released**.

- **Trending Now** is fetched from AniList's GraphQL endpoint (`Page.media(sort: TRENDING_DESC)`). AniList's trending sort tracks current weekly popularity, which Kitsu's `userCount` cannot match.
- **Popular This Season**, **Top Rated**, and **Recently Released** are fetched from Kitsu (REST/JSON:API). Kitsu's posters and banners (sizes verified at build time) are sufficient for these views.
- Both APIs are hit only when cache misses; cache is SQLite (`$XDG_CACHE_HOME/ani-gui/meta.db`) with TTLs from 1 hour (trending) up to 30 days (title-match cache).

When a user clicks a discovery card, the backend resolves its title against the provider (searching every available alias from the metadata API: English, Romaji, Native, synonyms) and falls into the same playback flow. The cross-API bridge — including how Kitsu's episode count disambiguates colliding titles on the provider, and how the MAL id is fetched for the aniskip and trending lookups — is documented in [`title-resolution.md`](./title-resolution.md).

When Kitsu's `coverImage` is null (common for shows currently airing — roughly half of the trending row in any given week), the detail-page resolver falls back to AniList: it bridges the Kitsu id through the mappings endpoint to a MAL id, then queries AniList for that MAL id's `bannerImage`. Without the fallback the detail page would render a flat colour where the hero banner belongs.

## Caching

| Asset | Storage | TTL |
|---|---|---|
| AniList trending row | SQLite `meta_cache` | 1 hour |
| Kitsu seasonal / top / recent | SQLite `meta_cache` | 6 hours |
| Per-anime metadata (`/anime/:id`) | SQLite `meta_cache` | 7 days |
| Availability probe (positive, ongoing show) | SQLite `meta_cache` | 24 hours |
| Availability probe (positive, finished show) | SQLite `meta_cache` | 30 days |
| Availability probe (negative — a clean miss, or the requested mode absent, from the provider that answered; served while that provider's gate is answering and every gate ahead of it is refusing) | SQLite `meta_cache` | 7 days |
| aniskip OP/ED skip-time intervals (per MAL id + episode) | SQLite `meta_cache` | 7 days |
| Title matches (search text → Kitsu/AniList ids) | SQLite `title_match` | 30 days |
| Long-term play resolution (resolved stream URLs) | SQLite play-resolution table | until upstream rotates |
| In-flight play-resolution coalescer (`play-cache.getOrFire`) | Renderer-side `Map` | 4 hours (also dedupes concurrent calls) |
| Poster + banner image bytes | Filesystem `images/<shard>/<hash>.<ext>` | LRU, capped at 500 MB |

Image bytes never live in SQLite; they're filesystem-keyed by `sha256(url)[..16]`, sharded two-deep to avoid huge flat directories. The play-resolution cache is separate from `meta_cache` — it stores fully-resolved stream URLs keyed by the request tuple `(title, mode, quality, episode, year, episode count)` so a repeat visit to an episode skips the whole provider walk. Entries are invalidated only when a cached URL fails on use; upstream URLs rotate, so the layer self-heals via the silent retry path rather than a wall-clock TTL.

The **availability TTL branches on Kitsu's `status` field**: shows airing weekly need a 24-hour window so a new episode surfaces within a day, but finished shows can hold for 30 days. Unknown / missing status falls back to the short (24h) window — a stale "no episode 1161 yet" is much worse than re-probing too eagerly.

## The copy earlier versions maintained

Versions before 0.12 shipped the script inside the bundle and kept it
current: a writable copy was seeded into `$XDG_CACHE_HOME/ani-gui/`,
the carried test-loader guard was stripped from it, and a background
task ran `-U` against it at every launch. A setting governed that and
the **/diagnostics** page showed a log of the runs.

Native resolution made the whole flow unreadable by anything. The copy
was not on `PATH`, no packaging exposed it, and a separately installed
`ani-cli` was never touched — so it updated a file nobody could reach,
over the network, on every start.

All of it is gone. What remains is a boot-time sweep: if that cache
copy is present, the backend deletes it and reports the path on
`/api/app-info`, so the **/diagnostics** page can say what was removed
rather than the file vanishing silently. The sweep takes a regular
file at exactly that path — a directory wearing the name is left
alone, and the neighbouring image cache and database are never
candidates.

## Embedded playback

Playback happens inside the desktop window — not in a detached `mpv` process. Implementation:

- `<video>` element receives the master.m3u8 URL from the local proxy.
- `hls.js` handles HLS streams. mp4 streams (some providers) play natively via `<video src=...>`.
- Subtitles inside the HLS manifest are surfaced by hls.js as `textTracks`. Sidecar tracks — the `.vtt` files a provider lists beside the stream, outside the playlist — ride the session: the proxy lists them at `/s/<id>/subtitles` and serves each at `/s/<id>/sub/<n>.vtt` with the session's referer, and the play page appends one `<track>` per listing. The player's CC picker selects among both kinds. Downloads write sidecar tracks beside the media as `<name>.<lang>.vtt` — never over a file already there, which is the user's; external players receive them as subtitle-file arguments, and Syncplay forwards them to the player it wraps the same way.
- Quality switching maps to hls.js's `currentLevel` for HLS, or re-resolution for mp4.

An "Open in external player" button on the player chrome launches the user's `mpv` (or platform default) with the natively resolved master-playlist URL. This is a user choice, never an automatic fallback — silent fallback would be confusing.

### Skip OP / ED via aniskip

The player surfaces "Skip Opening" / "Skip Outro" buttons during their respective intervals. The skip times come from [aniskip.com](https://aniskip.com)'s community-submitted database, keyed by MyAnimeList id rather than Kitsu. The backend bridges Kitsu → MAL using Kitsu's mappings endpoint, then asks aniskip for `(mal_id, episode)` skip intervals and caches the response for 7 days (skip times stabilize quickly once submitted). When auto-skip is enabled in settings, the player jumps the playhead past the interval automatically; otherwise it just shows the button.

### Persistent Picture-in-Picture across navigation

The Fullscreen and Picture-in-Picture APIs both bind to a specific `HTMLVideoElement` instance: removing the element from the DOM closes the PiP window. SvelteKit destroys page components on route change, which would otherwise kill PiP every time the user clicked away from the player.

The app sidesteps that by parking the `<video>` element in a hidden 1×1 host attached to `document.body` — body lives outside Svelte's reactive tree, so the element survives any number of route changes. The play page is a "controller" for that singleton: on mount it moves the element into its player frame; on destroy it moves it back to the hidden host. PiP keeps drawing throughout.

Navigation away from the player branches three ways:

1. **Episode swap on the same show** — the singleton stays attached to the play frame; the new page's load effect swaps its `src` in place. No PiP, no teardown.
2. **Different route or different show**, auto-PiP enabled (default) — the page calls `requestPictureInPicture()` from the navigation hook, the floating window appears, the user keeps watching while they browse. A paused video also pops out into PiP so the user keeps the floating thumbnail and can resume from there.
3. **Different route or different show**, auto-PiP disabled — the page pauses the singleton instead of requesting PiP. Without an explicit pause the off-screen element would keep streaming audio in the background.

The PiP window itself has two close paths, and the app distinguishes them:

- **X button (close in place)** — the platform's PiP UI pauses the video as part of the close path. The app reads this signal (a `pause` event lands within milliseconds of `leavepictureinpicture`) and does nothing else; the user dismissed the floating thumbnail and stays where they are.
- **Return-to-tab** — the platform keeps playback state intact. The app interprets that as an explicit request to come back to the player and navigates to `/play/[id]` so the stream surfaces inline again.

The discriminator is "did a `pause` event fire within ~100 ms of `leavepictureinpicture`?". The edge case (user manually pauses then immediately clicks return-to-tab inside the 100 ms window) misclassifies as X-close; this is accepted to keep the common cases right.

Clicking back into the same episode reuses the live session: the play page's load effect detects that the singleton already has the right `src` loaded and skips re-attaching, so playback resumes at its current timestamp instead of restarting from zero.

### Episode prefetching

Two prefetch surfaces warm play data ahead of demand so episode boundaries don't stutter:

- **Adjacent-episode warm.** When the play page mounts, it warms `episode + 1` (and on the detail page, `episode 1`) through the same play-resolution path the click would take. Hits land in the long-term resolution cache. If the current playback ends and auto-play-next is on, the next episode usually plays from cache instead of waiting on a fresh provider walk.
- **Visible-page warm.** The episode strip's currently-rendered Kitsu page is warmed in parallel so episode tiles get titles and thumbnails before the user scrolls.

Both flow through `play-cache.getOrFire` — keyed by show id + episode + mode + quality — which dedupes concurrent calls and keeps a 4-hour TTL. Cancellation goes through `clearForShow(showId)`, which aborts every in-flight prefetch for that show.

The cancellation policy is PiP-aware. On play-page destroy:

| Situation                                        | Action                                                                                  |
|--------------------------------------------------|-----------------------------------------------------------------------------------------|
| No PiP active                                    | `clearForShow` immediately — the user truly left the show.                              |
| PiP active                                       | **Defer.** Register a one-shot `leavepictureinpicture` listener and a deferred-cancel registry entry keyed on the show id. The user is still engaged with the show via the floating thumbnail. |

The deferred entry can be discharged in three ways:

1. **PiP closes elsewhere** — the listener fires `clearForShow(showId)` and self-removes. User truly disengaged.
2. **PiP closes while the user is back on `/play/[id]` for the same show** — listener noops; the new mount has already taken ownership of the prefetches.
3. **A different show's `/play/[id]` mounts during PiP** — the new mount calls `fireDeferredCancelsExcept(currentShowId)`, which flushes every deferred cancel whose id differs from the current one. Without this, two shows' prefetches would run concurrently against the provider's rate limit until PiP eventually closed.

Closing PiP via X **while still on `/play/[id]`** doesn't kill prefetch — the page never unmounted, no listener was registered, and `onDestroy` hasn't run.

The pure decision helpers and the registry live in [`frontend/src/lib/play/prefetch-lifecycle.ts`](../frontend/src/lib/play/prefetch-lifecycle.ts) and are unit-tested next to the file.

## User settings

User-editable settings live in `$XDG_CONFIG_HOME/ani-gui/config.toml`. The Settings page reads/writes via the backend (`GET / PUT /api/settings`); changes apply immediately to the surfaces that observe them. Available fields:

| Field | Default | Effect |
|---|---|---|
| `mode` | `"sub"` | Audio mode for new play / download calls — `"sub"` or `"dub"`. |
| `quality` | `"best"` | Quality bucket — `"best"`, `"1080"`, `"720"`, `"480"`, `"worst"`. |
| `locale` | `"en"` | UI locale (the four MVP locales — see [`i18n.md`](./i18n.md)). |
| `external_player` | `"mpv"` | Command launched by "Open in external player". |
| `image_cache_cap_mb` | `500` | Cap for the on-disk image cache; LRU evicts above this. |
| `auto_play_next` | `false` | When the current episode ends, automatically resolve and play the next one. |
| `auto_skip_op` | `false` | When aniskip has an OP interval, jump past it automatically. |
| `auto_skip_ed` | `false` | Same as above, for the ED. |
| `use_custom_player_controls` | `false` | Replace the browser's native controls with the in-app two-row bar. The native bar gives free PiP/captions menus; the custom bar keeps the Skip OP/ED button visible during fullscreen. |
| `disable_auto_pip_on_leave` | `false` | When set, navigating away from the player pauses playback instead of entering PiP. |
| `download_bottom_bar_enabled` | `true` | Show the per-download progress dock at the bottom of the window when downloads are active. |

## Localization

Four MVP locales: English (`en`), Brazilian Portuguese (`pt-BR`), Latin American Spanish (`es-419`), Russian (`ru`). The set was chosen for free-content market fit, not language-coverage prestige. Phase-2 candidates listed in `docs/i18n.md`.

The backend never returns localized text. Errors are stable keys (`error.scraper.timeout`, `error.search.no_results`, etc.); the frontend resolves them via Paraglide. Anime titles themselves are not translated by the app — they come from Kitsu/AniList per a user-chosen title-language preference.

## The retired CLI

The project spent its first releases as a desktop shell over the vendored `ani-cli` script, then replaced the subprocess with native resolution and, release by release, retired everything that carried the script: the spawn, the packaged copy, the boot-time updater, and finally the vendored file itself. The repository holds only the GUI now. Users who want a terminal flow install upstream's script; the two share an origin and nothing at runtime. What remains in the tree is the boot sweep (`backend/src/legacy_script.rs`) that deletes the copy earlier versions maintained in the user's cache, and reports having done so.

## Design direction (UI as a first-class surface)

The pivot from CLI to GUI is positioned as a premium-experience product: the UI is the differentiator. The design direction explicitly rejects generic AI aesthetics and embraces:

- Dynamic per-anime theming, with accent colors extracted from `coverImage.color`.
- Editorial typography pairing — a display face for hero titles, a clean body face for paragraphs, oversized tabular numerals for episode numbers.
- Motion as structure: elastic-eased carousels, parallax-on-hover cards, shared-element page transitions (poster card morphs into the detail-page poster), theater-dim into playback.
- Subtle anime motifs: manga-page-inspired dividers, oversized episode numerals, occasional Japanese typography accents — restrained, not cosplay.
- A player chrome closer to Apple TV+ than to VLC: minimal, autohides cleanly.

`tests/arch/i18n.sh` enforces that no `.svelte` file ships with hardcoded English. The wider design guard rails are documented in `AGENTS.md` §7.
