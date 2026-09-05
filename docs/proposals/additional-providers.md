# Proposal: additional stream providers

**Status**: proposed. The candidate survey below was taken on
2026-09-05 and describes a landscape that rots quickly — re-verify
every claim in it before building on one.

## Why this matters

All stream resolution rides a single provider, anidb.app, and inside
ten days that bet failed twice in two different ways:

- **2026-08-27** — the provider's server-rendered routes stalled
  globally for hours (TLS completed, then zero bytes until timeout)
  while its JSON routes kept answering. Nothing new could be
  resolved; plays survived only where a cached resolution existed —
  resolution caching was still unconditional then — inside the
  cache's seven-day row lifetime, with a stream URL that still
  answered validation.
- **Since early September 2026** (observed 2026-09-05) — a total,
  deliberate outage: every route, the JSON API included, answers a
  self-branded "Under Maintenance" 503 through Cloudflare in about a
  third of a second. Resolution caching became an opt-in setting,
  default off, on 2026-09-03 — so this time most installs have no
  cached rescue at all, and an opted-in install keeps a play alive
  only while a row under seven days old still validates. Playback
  is down.

The same outage took down the whole `ani-cli` ecosystem, which moved
onto anidb.app for its v5 in July 2026. Its maintainers are not
certain the site comes back, and warn that even if it does, its
endpoints may return changed (pystardust/ani-cli#1890). Waiting out
each outage is therefore not a strategy; a second provider is.

## What a second provider must offer

1. **Reachable by our transport class.** The backend speaks HTTP
   through TLS-fingerprint impersonation (curl-impersonate). The
   binary supports arbitrary headers and cookies, but the current
   fetch seam does not expose them — it passes a bare URL with a
   fixed user agent, one fresh process per request, no cookie jar —
   so scrape-time header and cookie control (`Referer`, `Origin`,
   `X-Requested-With`, cookie continuity across a walk) is part of
   any new client's work, not something the transport already
   hands it. Impersonation defeats fingerprint checks; it cannot
   execute JavaScript, so a Cloudflare interactive challenge or
   equivalent is a hard wall for the Rust client alone (see "The
   transport gap" below). And reachability has two halves, because
   the scrape and the playback ride different transports: the
   impersonating binary only performs the walk, while the embedded
   player's playlists and segments are fetched by the proxy's own
   plain `reqwest` client (`backend/src/proxy/upstream.rs`, a rustls
   fingerprint), downloads by yt-dlp and ffmpeg, and handoffs by the
   external player. A provider can resolve cleanly and still reject
   every play if its CDN fingerprints the second transport. A
   candidate counts as reachable only once a master playlist and its
   segments have been pulled through the local proxy — and through
   each non-embedded consumer the app supports — not when the site
   answers a scrape probe.
2. **Searchable by title.** The title-resolution bridge feeds
   Kitsu-derived titles in priority order (`docs/title-resolution.md`);
   the provider needs a text search that accepts them.
3. **An episode listing with numbers.** Availability caps and the
   count-based disambiguation both come from it. Numbering models
   differ per provider and are part of the bridge contract: anidb.app
   numbers continuously across seasons (episode 1 of season 2 is
   episode 14), while season-split providers number within each
   entry. A cap from one model is not comparable to a cap from the
   other.
4. **A per-mode (sub/dub) signal**, ideally per episode. The
   per-episode audio-caps entry in `docs/deferred-work.md` exists
   because anidb.app carries audio only on each episode's languages
   row; a provider that answers per-mode cheaply closes it.
5. **A stream-extraction step with sustainable maintenance cost.**
   Embed obfuscation rotates; the question is how often and how much
   machinery each rotation costs.
6. **Catalogue breadth and rate-limit temperament.** The gate,
   breaker and reservation layers apply per provider either way.

## Survey — 2026-09-05, one residential connection

Probed with plain curl and with the staged impersonating transports
(Chrome 116 and 136 fingerprints). "Reachable" below is the scrape
half of requirement 1 only: no playlist or segment was pulled
through the proxy's own client, so playback-transport acceptance is
unverified for every row. One connection, one day: a Cloudflare
challenge can be per-IP and per-moment, and domain listings churn
constantly.

| Provider | Reachable? | Verdict |
|---|---|---|
| anidb.app (current) | Maintenance 503 on every route | Primary; fate unknown |
| hianime (via hianime.at) | Yes — 200 in ~0.3s, real content | **Strongest candidate** |
| anizone.to | Yes — plain curl, no challenge | Possible third; small |
| allanime | Cloudflare interactive challenge | Blocked for the Rust client |
| animepahe (now at .pw) | Cloudflare interactive challenge | Blocked for the Rust client |
| animekai | .to unresolvable, .cc/.bz parked | Current home not found |

**hianime.** The canonical hianime.to is SNI-filtered on the
surveyed connection: the name resolves to Cloudflare, and a TLS
handshake carrying it hangs until timeout even when the IP is pinned
— the first surveyed case where reachability depends on the user's
ISP rather than the provider, and an argument for failover in
general. The hianime.at domain serves the same site and answered an
impersonated GET in ~0.3s. `ani-cli` has a working single-file
scraper against hianime.at as of 2026-09-05
(pystardust/ani-cli#1894): search-page HTML → per-entry AJAX episode
list → per-episode AJAX server list, whose entries are typed sub or
dub (a **per-episode** audio signal, requirement 4 at its
strongest) → base64-decode a server hash into an embed URL → the
embed page carries an XOR-obfuscated blob (key `otaku-embed-v1`)
whose plaintext holds the master-playlist URL. Two bridge-relevant
properties: entries are season-split, and the site surfaces
MyAnimeList ids (its entry pages link them; the decoded embed URLs
carry them), so a pick can be cross-checked against the Kitsu→MAL
mapping. That is a verification signal on top of the episode-count
disambiguation, not a replacement for it: the primary identifiers —
entry slug, episode id, server hash — are provider ids, the MAL id
costs a per-candidate fetch, and it stays a claim to verify until
the client has proven the mapping holds. Risks: domain churn and ISP blocks make
"which domain" a live question (a configurable or probing base URL,
not a constant); the obfuscation key literally carries a version
suffix, so rotation is a when, not an if.

**anizone.to.** Fully open to plain curl — no challenge, ~0.6s
responses — and named by an `ani-cli` maintainer the same day as
successfully scraped without HLS blocking. The UI is
Livewire-rendered: initial HTML carries navigation but not result
cards, so search and listings mean speaking to its Livewire
endpoints rather than scraping query-param pages. The same
maintainer's own caveat: a medium-sized self-hosted library that
"could go down as easily as anidb". Worth keeping on the list as a
later third, not as the second.

**allanime.** The ecosystem's home before 2026, and what this
project's pre-native pipeline drove through the vendored script. The
API host answers instantly — with a Cloudflare interactive challenge,
served to the impersonating transports as well (correct referer
included). That is presumably why the ecosystem left. Its GraphQL API
returned per-mode episode arrays (`availableEpisodesDetail`), the
exact shape requirement 4 wants, which is the reason to keep it on
the list should the transport gap below ever be closed.

**animepahe.** Found at animepahe.pw (the .ru domain now redirects
to a for-sale page at .su). Behind the same Cloudflare interactive
challenge as allanime. Same conclusion.

**Not surveyed**: the gogoanime/anitaku lineage (a history of domain
deaths), 9anime/aniwave (shut down in 2024), and smaller sites. The
open-source scraper ecosystem's reference codebases (consumet.ts,
aniwatch-api) were removed from GitHub by DMCA takedowns in March
2026, so "read what maintained projects do" now has fewer places to
look; `ani-cli`'s repository is the liveliest public reference left.

## The transport gap, and the option only this app has

curl-impersonate passes TLS fingerprinting; it cannot pass a
JavaScript challenge, and two of the three largest candidates sit
behind one. This app ships a real Chromium. A hidden window could
visit a challenged origin, let a non-interactive check complete, and
hand the resulting clearance cookie to the Rust client — which must
then present the same user agent from the same IP for the cookie to
hold. That would open allanime and animepahe without an
obfuscation-solving arms race, at real cost: challenges can be
interactive (then the choices are surfacing a visible window or
giving up), clearance cookies expire within hours and are bound to
origin, IP and user agent, and the cookie hand-off is a new bridge
between shell and backend with its own failure modes. If allanime is
ever the pick, this is its own investigation first.

## How a second provider slots in

- **The code seam.** The scraper module is already split into a
  client (`backend/src/scraper/anidb/`) and provider-agnostic policy
  (`gate`, `outcome`, `reservation` — "nothing here decides
  policy"). What is missing is the trait boundary: the play,
  availability and download commands import the concrete client and
  its types directly, and app state holds a single gate. The work is
  a provider trait with provider-neutral hit/episode types, one gate
  instance per provider, and the commands taking the trait. History
  reverse resolution is a fourth consumer of provider knowledge:
  when a Continue Watching row has no stamped mapping, the recovery
  derives a search term from the slug through the current provider's
  parser alone and writes anything shaped differently off as an
  unresolvable legacy row — provider-aware id parsing belongs to the
  seam, and the id-qualification decision below must leave this path
  able to tell whose id it is holding. The seam
  owns attribution too: the resolve path stamps a hard-coded
  provider label into its progress events, and the play and download
  walks record their combined outcome into the one gate after the
  fact — under failover both must be answered per attempt by the
  abstraction or its orchestrator, or an attempt against the second
  provider displays the first one's name and trains the first one's
  circuit breaker. And a resolved stream must carry its provider's
  playback request context: hianime's CDN wants the embed origin as
  the `Referer`, while the current provider needs none, so the play
  command hard-codes an empty one into the proxy session and the
  resolution-cache row. The proxy is already referer-capable per
  session — the seam work is the provider owning that value (and any
  future header like it) in the resolved result, propagated through
  the resolution cache, the proxy session, the download path, and
  the external-player and Syncplay handoffs, whose fresh-resolve
  path builds its launch arguments with no referer today on the
  same "the current provider needs none" reasoning.
  Subtitles ride the same decision: today they travel inside the
  HLS master, whose rewrite the proxy already fetches and serves
  (`.vtt` included), while hianime delivers soft-sub tracks as
  sidecar files outside the playlist — so the resolved result also
  carries subtitle descriptors, served through the proxy like every
  other upstream fetch rather than by exposing upstream URLs to the
  player. The proxy answer covers only the embedded player; the
  three non-embedded consumers need their own: the download tool
  receives just the master URL today, so sidecar tracks have to be
  fetched alongside the media or handed to the tool to mux, and the
  external-player and Syncplay handoffs expose a stream URL and
  referer only — forward a subtitle argument where the target
  player accepts one, and say so where it does not.
- **Provider-stamped state.** Every cache stamped by provider output
  needs an explicit migrate-or-keep decision in the same change:
  resolution-cache rows; availability verdicts per `(kitsu_id,
  mode)`, which today mean "on anidb.app" specifically; history rows
  keyed on the provider slug and the slug→kitsu reverse mapping —
  one row per id, appended rather than merged, and the detail page's
  resume lookup takes the first file-order row whose mapping matches
  the Kitsu id, so a show watched through two providers gets two
  rows for one entry and resumes from the older one unless the
  migration adds a Kitsu-level selection rule (most recent
  last-watched stamp wins, or one row per Kitsu entry carrying the
  provider as an attribute); the numbering-offsets sidecar beside
  the history file
  (`backend/src/commands/anidb_offset_store.rs`), one row per bare
  slug carrying a show's numbering offset and last-watch display
  stamp — it outlives SQLite cache clears and is consulted on every
  history read and write, so a second provider's slugs landing in it
  unqualified would apply the wrong episode translation to Continue
  Watching; the last-watched stamps (`watched-at:v1:<show id>` rows
  in the metadata cache, 30-day TTL) that Continue Watching's
  recency order joins against history rows by the same bare id —
  qualify history ids without migrating these and every stamp
  detaches, demoting the whole strip to file order; keep both bare
  and two providers' identical slugs share one timestamp; the
  title-match rows (`title-match:` keys mapping a normalized
  provider title and cour to a Kitsu id, 30-day TTL), which the
  reverse-resolver falls back to when the slug mapping is absent or
  rejected — they carry no provider dimension, so colliding titles
  from two providers can read or overwrite each other's mapping;
  the read side re-validates a hit before trusting it, which
  softens the collision without closing it; and episode caps, which
  inherit the numbering-model difference above.
- **Failover policy is three features, not one.** (1) Fail over when
  the primary is unreachable or refusing — the states the gate and
  outcome layers already classify; this is the outage fix. To
  restore anything it also needs a bound on the primary attempt: an
  interactive resolve is allowed the full 60-second deadline today,
  and interactive requests deliberately bypass an open breaker, so a
  sequential try-then-fall-over would spend a minute on a known-down
  primary before every play. The orchestrator has to consult the
  breaker to skip (or race) a primary whose outage is already
  established, or give the first attempt a far shorter budget than
  the resolve deadline. Failover also changes what a clean miss
  proves. Today a clean miss is the one verdict that persists as a
  negative availability row, because a single provider that
  searched every alias and found nothing has proven absence. A
  fallback's clean miss while the primary is unreachable proves
  nothing about the primary, so the aggregation rule has to be
  explicit: a global negative may be written only when every
  provider answered cleanly; otherwise key negatives per provider,
  or persist nothing — or a primary-only show stays hidden for the
  negative row's lifetime after the primary recovers. (2) Fall
  through to another provider when the primary lacks the show —
  catalogue expansion, which turns the same per-provider absence
  question into a user-visible one (a show absent on the primary
  and present on the fallback is "available", and every surface
  that says so has to agree). (3) Per-title manual choice — UI
  plus a cache dimension. Build (1) first; (2) and (3) only on
  demonstrated need.

## Recommendation

Build the provider seam and a hianime client together, as one arc,
with failover-on-unreachable only. The case: it is reachable today
where the primary is not (restoring playback now, not just
resilience later); it carries per-episode sub/dub typing and MAL ids
(strengthening two existing weak spots); a working reference scraper
exists in public as of 2026-09-05; and its extraction step is
tractable. Treat the base domain as data, not a constant. Hold
anizone as a candidate third, and allanime/animepahe behind the
webview-clearance investigation. The seam alone, with one provider,
would be speculation — land it with the client that needs it.

If anidb.app returns with changed endpoints before this work starts,
repairing the primary comes first and this proposal's ordering is
unchanged: the outage that motivated it will have happened twice
either way.
