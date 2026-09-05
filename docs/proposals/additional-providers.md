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
   through TLS-fingerprint impersonation (curl-impersonate) with
   full control of headers and cookies. That defeats fingerprint
   checks; it cannot execute JavaScript, so a Cloudflare interactive
   challenge or equivalent is a hard wall for the Rust client alone
   (see "The transport gap" below).
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
(Chrome 116 and 136 fingerprints). One connection, one day: a
Cloudflare challenge can be per-IP and per-moment, and domain
listings churn constantly.

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
properties: entries are season-split, and the embed URLs are keyed
by MyAnimeList id — the provider itself is MAL-mapped, so a pick
could be verified against the Kitsu→MAL mapping directly instead of
by episode-count distance. Risks: domain churn and ISP blocks make
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
  instance per provider, and the commands taking the trait.
- **Provider-stamped state.** Every cache stamped by provider output
  needs an explicit migrate-or-keep decision in the same change:
  resolution-cache rows; availability verdicts per `(kitsu_id,
  mode)`, which today mean "on anidb.app" specifically; history rows
  keyed on the provider slug and the slug→kitsu reverse mapping; the
  numbering-offsets sidecar beside the history file
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
  and two providers' identical slugs share one timestamp; and
  episode caps, which inherit the numbering-model difference above.
- **Failover policy is three features, not one.** (1) Fail over when
  the primary is unreachable or refusing — the states the gate and
  outcome layers already classify; this is the outage fix. (2) Fall
  through to another provider when the primary lacks the show —
  catalogue expansion, with murkier availability semantics (absence
  verdicts become per-provider). (3) Per-title manual choice — UI
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
