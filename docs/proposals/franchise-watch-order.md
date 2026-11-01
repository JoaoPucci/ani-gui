# Proposal: watch order — what comes before, what comes next, and the franchise around a show

**Status**: proposed as the main feature of the 0.19.0 release. Nothing
below is built. The research was done on 2026-11-01; the live samples
it quotes describe the catalogues on that day and will drift.

## The problem in plain terms

Anime is published in pieces. A story that a viewer thinks of as one
show — *JoJo's Bizarre Adventure*, *Gintama*, *Bleach: Thousand-Year
Blood War* — reaches the catalogues as many separate entries: seasons,
"parts" and "cours", movies, OVAs, specials and recaps, each with its
own title, poster and episode list. `ani-gui` takes its catalogue from
Kitsu, which models every one of those pieces as its own anime, so
the app does too.

Today the app knows nothing about how those entries connect. Three
things follow from that:

1. **A finale is a dead end.** When you finish the last episode of a
   season, the player stops (auto-play halts at the last known
   episode), and the Continue Watching card and the show page's button
   go back to that same last episode — the show page labels it
   "Replay". Nothing says that a second season exists, let alone
   offers it.
2. **A show page has no sense of place.** Opening *Bleach:
   Thousand-Year Blood War – The Calamity* does not tell you it is the
   fourth part of a run, which part comes before it, or that you have
   not watched that part.
3. **A long franchise is a hunt.** Someone who left *JoJo* half-way
   through, months ago, has to search, scan eight or nine similarly
   named results, and remember which part they were on. That is the
   request behind this proposal: a viewer asked for JoJo to be a
   single entry, so as not to have to hunt for the part they were on.

The aim is for a show to know what comes first, what comes next, and
in what order — and for the app to answer "where am I in this
franchise?" without merging entries into one.

Labels used below: **[data]** is something observed in an API
response, a cached file or the code; **[inference]** is a conclusion
drawn from data; **[proposal]** is a design choice.

## 1. How others do it

### Netflix: seasons live inside one title

Netflix's model is one title with numbered seasons, and its autoplay
setting is phrased in those terms: "You can set Netflix to
automatically play the next episode in a series" ([Netflix Help: How to autoplay the next
episode](https://help.netflix.com/en/node/121518)).

Anime that the catalogues split into separately named entries is
folded into that model. *Demon Slayer*'s *Entertainment District
Arc* reached Netflix as "season 3" of the one *Demon Slayer* title,
where Crunchyroll, following the Japanese seasons, has it as the
second part of season 2 ([What's on Netflix](https://www.whats-on-netflix.com/news/demon-slayer-season-3-sets-netflix-us-release-date/);
[Den of Geek on the two "season 3"s](https://www.denofgeek.com/tv/why-does-demon-slayer-have-two-season-3s-the-confusing-world-of-anime-arcs/)).
*JoJo's Bizarre Adventure: Stone Ocean* is described as "the defacto
fifth season" of the one JoJo title, with country tables counting five
seasons ([What's on Netflix](https://www.whats-on-netflix.com/news/jojos-bizarre-adventure-stone-ocean-part-2-is-coming-to-netflix-in-2022/)).
Netflix also labels seasons "Part 1"/"Part 2" where a producer split
one, which trackers then have to translate back into seasons
([SIMKL: parts vs seasons](https://docs.simkl.org/how-to-use-simkl/getting-started-with-simkl/basic-navigation/tv-shows-tracking/parts-vs-seasons-tv-show-formats)).

[inference] Netflix can merge because it is the publisher: it decides
what a title contains, by licensing, and its numbering is its own. It
does not merge *everything* — films and spin-offs stay separate
titles. And its numbering disagrees with everyone else's, which is
the cost of curating by hand.

### Crunchyroll: one series page, a season dropdown

Crunchyroll groups seasons under one show page with a season dropdown
— its help centre describes marking a whole season watched "from the
season dropdown on the show page"
([Crunchyroll Help](https://help.crunchyroll.com/hc/en-us/articles/31320247305748-How-do-I-mark-episodes-seasons-and-shows-as-watched)).
For years every dub was listed as its own "season" (*Spy × Family*
showed nine seasons), until a language picker moved into the player
in January 2023 ([Epicstream](https://epicstream.com/article/crunchyroll-fixes-its-seasons-interface)).

[inference] The lesson is that "next season" is only as good as the
grouping beneath it: for years Crunchyroll's grouping mixed language
versions with seasons, so "the next season" was ambiguous on its own
show pages.

### HIDIVE

HIDIVE's pages are rendered client-side and did not load for this
research; a fetched series page showed only "Oshi No Ko – Season 1"
with no selector. Nothing is claimed about HIDIVE here.

### Plex and Jellyfin: collections plus "Next Up"

Self-hosted servers separate two ideas:

- **Collections** — a group of whole shows and films, made by hand
  or by a metadata agent
  ([Plex Support: Collections](https://support.plex.tv/articles/201273953-collections/)).
  A collection is a shelf: it has no progress of its own and does not
  resume.
- **Next Up / On Deck** — per-show, number-driven. Jellyfin's Next Up
  "queues things by using the season and episode number" and "never
  uses aired dates", and specials appear only when a setting places
  them inside a season
  ([Jellyfin forum](https://forum.jellyfin.org/t-special-episode-not-showing-in-next-up?pid=49647));
  in Plex, a user reports On Deck going by air date while the
  post-play "up next" screen does not interleave specials
  ([Plex forum](https://forums.plex.tv/t/on-deck-suggesting-next-seasons-episode/604802)).

Both rely on the library already being organised as one show with
seasons (by TVDB/TMDB conventions). Anime libraries split per
AniDB/MAL entry suffer exactly the problem described here, which is
why anime users of those servers lean on custom orderings and
plugins.

### AniList, MyAnimeList and Kitsu: relation graphs, not lists

The anime databases do not store "a franchise". They store **edges**
between entries, each with a type.

- **AniList** shows a "Relations" row on every entry. Its GraphQL
  schema's own descriptions (read by introspection on 2026-11-01)
  say `PREQUEL` means "Released before the relation" and `SEQUEL`
  "Released after the relation" — i.e. *release* order. The other
  types: `PARENT`, `SIDE_STORY`, `SUMMARY`, `ALTERNATIVE`, `SPIN_OFF`,
  `CHARACTER`, `ADAPTATION`, `OTHER`, plus `SOURCE`, `COMPILATION`
  and `CONTAINS` in the schema's version 2 and `SAME_UNIVERSE` in its
  version 3.
  But a submission manual written for AniList contributors says
  "Relations should be in ***chronological*** order" and defines a
  prequel as "a story preceding the current work (chronologically)"
  ([AniList submission manual, gitbook](https://anilist2.gitbook.io/submission-manual/submission-form/relations);
  the page does not say it is official). The two definitions
  disagree, and the data (section 2) shows both in use.
- **MyAnimeList** lists "Related Entries" on each page. Watch order
  across a franchise is community work: MAL's *Anime Entry Guide
  Club* asks contributors to link the franchise view on
  [chiaki.site](https://chiaki.site/), a third-party site that walks
  MAL's relations into a watch-order list, when a series should be
  watched in broadcast order, and to write the order out by hand
  otherwise ([MAL club](https://myanimelist.net/clubs.php?cid=79626);
  the [MAL stack "Largest Anime Franchises (51-100)"](https://myanimelist.net/stacks/39205)
  links chiaki's groups). Guide sites such as AnimeBookmark
  offer separate release-order and story-order lists per franchise.
- **Kitsu** stores `media-relationships` with a role from a fixed
  set — `sequel`, `prequel`, `alternative_setting`,
  `alternative_version`, `side_story`, `parent_story`, `summary`,
  `full_story`, `spinoff`, `adaptation`, `character`, `other`
  ([kitsu-server `media_relationship.rb`](https://github.com/hummingbird-me/kitsu-server/blob/the-future/app/models/media_relationship.rb)).
  Kitsu's source also has a **franchise** model whose installments
  carry both a `release_order` and an `alternative_order` and a tag
  (`main_story`, `side_story`, `spinoff`, `crossover`,
  `alternate_setting`, `alternate_version`)
  ([kitsu-server `installment.rb`](https://github.com/hummingbird-me/kitsu-server/blob/the-future/app/models/installment.rb))
  — exactly the shape this feature wants. Section 2 shows why it
  cannot be used.

### Release order versus story order

"Prequel" and "sequel" are ambiguous in the wild. For most shows
the two orders coincide — season 2 aired after season 1 and continues
it. They diverge where a story was told out of order:
*Fate/Zero* (2011) is set before *Fate/stay night* (2006); the
*Monogatari* series aired *Bakemonogatari* (2009) first but set
*Kizumonogatari* (films, 2016–17) and *Nekomonogatari (Kuro)* (2012)
before it; the 2000 *JoJo* OVA covers the start of Part 3 and the
1993 OVA its end. Section 2 shows that the databases encode these
cases inconsistently, which decides how much the app can promise.

### What to take from this

[inference]

- Viewers expect "next" to just work at a season's end; the
  platforms that do it well own the grouping.
- No platform merges a franchise's *films and side stories* into the
  season run. Merging stops at the main story; the rest is shelved
  around it (Netflix: separate titles; Plex: collections).
- Where grouping is derived from data rather than curated,
  "what's next" has to tolerate ambiguity — several candidates, an
  unknown order — rather than guess.

## 2. What data the app has

### Kitsu `media-relationships` (already read)

[data] The backend already reads this endpoint:
`KitsuClient::media_relationships` in
`backend/src/meta/kitsu_relations.rs` asks for one page of up to 20
relations with each destination's subtype, status, episode count and
start date, and `commands::kitsu::kitsu_relationships` caches the
result under `kitsu:relations:v3:<id>` for a week. Today it feeds the
tracker episode map (`single_relation` in
`backend/src/commands/tracker_map_kitsu.rs`, which accepts a sequel or
prequel only when the page was complete and named exactly one) and
the appended-extras check. No screen shows relations.

[data] The parsed record keeps a `complete` flag: whether the one page
read held every relation Kitsu counts. *Ranma ½* (Kitsu 186, a repository fixture) has 15,
*Gintama* (818) 15. [inference] Long franchises with many specials can
pass 20, and then a `sequel` edge may be on a page not read. [data]
Kitsu honours a role filter on this endpoint:
`filter[role]=sequel,prequel` on *Gintama* answered `count: 1`
instead of 15 — a cheap way to ask only for the edges a watch order
needs.

Samples (live reads on 2026-11-01 unless marked as a repository
fixture):

| Entry (Kitsu id) | What Kitsu relates it to | Reading |
|---|---|---|
| *Bleach: TYBW* part 1 (43078) | prequel *Bleach* (2004, 366 eps); sequel *The Separation* (2023); three music videos as `other` | Linear, correct. |
| *TYBW – The Separation* (46903) | prequel part 1; sequel *The Conflict* (2024, 14 eps); a recap special as `summary` | Linear; the recap is correctly marked and easy to skip. |
| *Attack on Titan* S3 Part 2 (41982) | prequel S3 (2018); sequel *The Final Season* (2020); two recaps (*Chronicle*, a 25-minute digest) as `summary` | Linear, correct. |
| *JoJo: Stardust Crusaders* (8063) | prequel *JoJo* (TV, 2012); sequel *Stardust Crusaders 2nd Season* (2015); the 2000 OVA as `alternative_version` | Linear; Kitsu splits Part 3 into two entries, as MAL and AniList do. |
| *JoJo: Stone Ocean Part 3* (46598) | prequel *Stone Ocean Part 2*; sequel *Steel Ball Run* (ONA, 2026) | The chain runs across "parts" into the newest series. |
| *JoJo* 2000 OVA (7730) | sequel the 1993 OVA; prequel *JoJo* (TV, 2012); `alternative_version` *Stardust Crusaders* | **Story order**: the 2000 OVA's "sequel" aired seven years earlier, and its "prequel" twelve years later. |
| *Gintama* (818) | sequel *Gintama'* (2011); four specials as `side_story`; film 1 and the 2026 film as `alternative_version`; film 2 as `other`; the 2025 *Ginpachi-sensei* spin-off as `side_story` | Linear main chain; films classified loosely. |
| *Bakemonogatari* (3919) | sequels *Nisemonogatari* (2012) **and** *Kizumonogatari I* (2016); a recap as `summary` | **Release order**, and two sequels. |
| *Kizumonogatari I* (5649) | prequels *Bakemonogatari* and *Koyomimonogatari*; sequel *Kizumonogatari II* | **Release order**: in story order *Kizumonogatari* comes first. |
| *Fate/Zero* 2nd season (7658, repository fixture) | prequel *Fate/Zero*; **four sequels**: *Fate/stay night* (2006), the *Unlimited Blade Works* film (2010), the *Heaven's Feel* film (2017), the UBW prologue (2014) | **Story order** (2006 is the "sequel" of 2012), and one sequel per route. |
| *Fate/stay night: UBW* (7882, repository fixture) | prequel the UBW prologue; sequel UBW 2nd season; the 2006 series and both films as `alternative_version` | The routes are alternatives of each other. |

[inference] Kitsu's prequel/sequel edges are reliable for linear runs —
every TV continuation sampled (Bleach, TYBW, Attack on Titan, JoJo's
TV parts, Gintama) is linked correctly and recaps are marked
`summary`. They are **not** consistently release order or story
order: *Fate/Zero* and the *JoJo* OVAs follow story order,
*Monogatari* follows release order. And an entry can have several
sequels (Fate's routes; *Bakemonogatari*), so "the next one" is not
always a single answer.

### Kitsu franchises and installments (not usable)

[data] `GET /franchises` answers, with 1,332 franchises; the first two
were created and last updated on 2013-07-07. Every installments route
tried answered HTTP 500: `/franchises?include=installments.media`,
`/franchises/2/installments` with and without `include=media`, and
`/anime/7730/installments`. Kitsu's GraphQL schema has no franchise
lookup (`findFranchiseById` does not exist; the media type exposes
`relationships` and `mappings` but no franchise field, per
kitsu-server's `app/graphql/types/interface/media.rb`).

[inference] The model that would carry release and alternative order
exists in Kitsu's code, but the two franchises sampled were last
touched in 2013, and the route to their entries is broken. Do not build on it.

### AniList `relations` (not read yet)

[data] Listed in [`deferred-work.md`](../deferred-work.md#metadata-the-app-does-not-use-yet)
under "Relations". A single GraphQL request can fetch the relations of
several entries at once (the samples below were two requests of eight
and nine entries). Samples, 2026-11-01:

| Entry | AniList relations (anime only) | Reading |
|---|---|---|
| *TYBW* part 1 / *The Separation* | prequel *Bleach*; sequel *The Separation*; then sequel *The Conflict* | Same as Kitsu. |
| *AoT* S3 Part 2 / *The Final Season* | S3 → S3P2 → *Final Season* → *Final Season Part 2*; *Chronicle* as `SUMMARY` | Same as Kitsu. |
| *JoJo* (TV, 2012) | sequel *Stardust Crusaders*; the 2007 *Phantom Blood* film as `ALTERNATIVE` | Linear. |
| *JoJo* 1993 OVA | prequel the 2000 OVA; both OVAs `ALTERNATIVE` of *Stardust Crusaders* | Story order, like Kitsu. |
| *Nekomonogatari (Kuro)* | sequel *Bakemonogatari*; prequel *Kizumonogatari III* | **Story order** — the opposite of Kitsu's *Monogatari* edges. |
| *Bakemonogatari* | prequel *Nekomonogatari (Kuro)*; sequel *Nisemonogatari* | Story order. |
| *Fate/stay night* (2006) | prequel *Fate/Zero 2nd Season*; UBW, the films, *Apocrypha*, *EXTRA*, *Prototype*, *Grand Order* as `ALTERNATIVE` | Story order; "alternative" spans unrelated series. |
| *Gintama* | sequel *Gintama'*; specials `SIDE_STORY`; films `ALTERNATIVE`/`OTHER`; *Saiki Kusuo* and a *Keroro* film as `CHARACTER` | Same chain as Kitsu, more noise. |

[inference] AniList agrees with Kitsu on every linear run sampled and
disagrees on *Monogatari*, where it follows story order despite its
schema describing release order. It adds no franchise that Kitsu's
chain misses in these samples. Using it as the source would cost an
id bridge per entry (AniList → MAL → Kitsu, through Kitsu's mappings),
because the renderer keys everything on Kitsu ids. **Recommendation:
Kitsu's relations are the source; AniList's is not needed for this
feature.** It stays listed as unused metadata.

### What the dates give

[inference] Every relation already carries its destination's start
date, so **release order is computable** for any set of entries the
app has collected: sort by start date. **Story order is not**: it
exists only where the databases' editors chose to encode it in the
prequel/sequel edges, and as shown above they did so for some
franchises and not others, with Kitsu and AniList disagreeing. The app
can show "the order the databases link them in" and "the order they
aired", and say when the two differ; it cannot promise a correct story
order.

### What the streaming provider adds

[data] The provider lists versions as separate shows: the author's
development-build history holds both "JoJo's Bizarre Adventure Part 2: Stardust
Crusaders" and its "(Uncensored)" listing, both recorded against Kitsu
8063, and the provider names that entry "Part 2" where Kitsu and the
franchise call it Part 3. hianime also renames *Gintama*'s entries
"Season 2", "Season 4", "Season 5"; in the same history those rows
carry no Kitsu id, where other rows written the same day do.
[inference] Somewhere between the play and the history write, the
app did not settle which Kitsu entry those listings are; the exact
cause was not traced.

[inference] Every surface below works on Kitsu entries, so provider
versions collapse into the one Kitsu entry and the existing play path
picks the listing. Rows that never resolved to a Kitsu id cannot join
a franchise grouping — they stay outside it, as they stay outside
other Kitsu-keyed features.

## 3. What the author's library says

Read-only copies, taken 2026-11-01, of the installed app's history
and both metadata caches (the installed build's and the development
build's). The tracker list cache holds the author's MyAnimeList list
(294 entries) and AniList list (83 entries). Development histories mix
real watching with test plays, so single-episode rows are not read as
intent.

[data, approximate] Grouping the 294 MyAnimeList entries by title
family (a crude title match, which over-counts — *Fullmetal Alchemist* and
*Brotherhood* are separate adaptations, not a sequel pair): about
205 entries, 70 %, sit in 43 families of two or more. The largest:
Dragon Ball 32, Gintama 20, Digimon 12, Saint Seiya 12, JoJo 10,
Naruto 10, Bleach 8, Yu-Gi-Oh! 7. [inference] For this viewer,
franchises are the normal case, not the edge case, and they are long:
most of them carry films or specials beside the main run.

[data] Where "what's next" would have mattered, from the installed
build's history:

- ***Bleach: Thousand-Year Blood War***. Part 1 was played to episode
  13 of 13 — its finale — on 2026-10-05. [inference, from the
  Continue rule in `frontend/src/lib/play/next-episode.ts`] Its card
  now offers episode 13 again. The history then has *The Separation* (part 2)
  and *The Calamity* (part 4) rows, and no row for *The Conflict*
  (part 3); neither tracker list has *The Conflict* either.
  [inference] Whatever the reason, a show page for *The Calamity*
  that named its prequel and marked it unwatched would have made the
  gap visible.
- ***Bleach*** (episode 286 of 366) and ***Naruto Shippuden*** (425 of
  500) are long single entries mid-run: "next" inside an entry already
  works; these gain only a sense of the franchise around them.
- ***Gintama***: the 201-episode series at 175 (a rewatch — the
  MyAnimeList list has it completed). The list holds the run's TV
  entries — *Gintama*, *Gintama'*, *Enchousen*, *Gintama°*,
  *Gintama.* and both *Silver Soul* parts — beside the films, *The
  Semi-Final*, *The Final* and several specials; the provider names
  some of the TV entries "Season 2/4/5".
- ***JoJo***: ten list entries — Parts 1 to 4 and 6, the two OVAs and
  *Steel Ball Run* (planned); none for Part 5 — the franchise behind
  the original request.

## 4. Design options

All four options share one backend piece [proposal], two reads of
Kitsu's relations:

- `chain(kitsu_id)`: a bounded walk along prequel/sequel edges from
  that entry (say up to 40 entries), each with title, format, year,
  episode count, status and poster. It reads Kitsu's relations
  role-filtered to `sequel,prequel`, so the 20-per-page limit cannot
  hide an edge — a new, separately cached query beside the existing
  `kitsu:relations:v3` read, which asks for neither titles nor
  posters. One request per entry on the chain, cached; JoJo's TV run,
  from the 2012 series to *Steel Ball Run*, is nine entries.
- `around(kitsu_id)`: one entry's other relations — side stories,
  recaps, films and alternative versions — read in full, paging past
  20 where Kitsu counts more. They are attached as leaves and not
  walked from, because `alternative_version`/`ALTERNATIVE` links
  unrelated series (Fate's *Apocrypha* and *Grand Order* hang off
  *Fate/stay night*).

Neither makes a provider request. Kitsu is the metadata service the
app already pages through on every show page.

### (a) A "Watch order" strip on the show page

**How it looks.** A row under the hero, before the episodes: the
entry before, this entry, the entry after — poster, title, year,
format (TV, Movie, OVA), episode count, and your progress on each
(from the local history: "Watched", "EP 9 of 13", nothing). The
current entry is marked in the show's accent colour; its position in
the main run uses the app's large tabular numerals ("Part 04" for
*The Calamity*, counted from part 1 of *Thousand-Year Blood War* — or
from *Bleach* itself, which is the chain's start: one of the choices
the design pass has to make). Once the franchise page (c) exists, a
trailing link — "Side stories, films and recaps (5)", counted by
`around` — opens it; before then the strip has no trailing link. If you have progress elsewhere
in the franchise, the strip says so: "You're on *Golden Wind* · EP 12
→ Resume". That one line answers the JoJo request on whichever JoJo
page the viewer lands.

**Release versus story order.** The strip shows the databases'
prequel/sequel links, and labels an entry whose air date runs against
the link: "Next in the story · aired 2006" for *Fate/stay night* on
the strip of *Fate/Zero 2nd Season*.
No toggle on the strip.

**Edge cases.**

- Several sequels (*Fate/Zero 2nd Season* has four on Kitsu: the
  2006 series, two films and the UBW prologue; *Bakemonogatari* has
  two): the "next" slot becomes a small fan — "4 ways to continue" —
  in air-date order, not a guess.
- Recaps (`summary`) never take a slot; the role filter leaves them
  out.
- An unaired sequel (a start date in the future — the repository's
  fixture for Kitsu 48677 has a sequel starting 2027-01-10 with no
  episode count): shown dated,
  "Coming January 2027", not playable.
- An entry the provider does not carry: the strip shows it anyway, as
  the catalogue does; the show page it opens runs the existing
  availability check. A cached negative verdict, where one exists, can
  dim the card without any new request.
- No relations, or Kitsu unreachable: the strip is absent.

**Trackers.** None. The strip reads; every play it leads to is an
ordinary play of a Kitsu entry, filed through the existing episode
map.

**Cost.** Medium: the `chain` walk and its cache, one backend route,
one component, and one pure module that turns the chain into slots
and a position (unit- and property-testable, per the project's rule
that component logic lives in a tested `.ts` module). **Build
first.**

### (b) "Up next: season 2" at a finale, and Continue moving on

**How it looks.**

- *In the player*: when the last episode of an entry reaches its
  ending credits (the point the existing "Skip Ending" button already
  knows from skip data, else the final seconds), the button becomes
  an end card — the sequel's banner, its title, "Up next · Episode 1".
  With auto-play on, the card counts down and plays it; the countdown
  can be cancelled.
- *On Continue Watching*: once the finale has been played through,
  the card stops offering the finale again and shows the sequel —
  its art, its title, "EP 1". When the sequel has its own history row
  (you started it), the finished entry's card gives way to it, so the
  strip does not show both.

**Rules** [proposal]. Advance only when all hold: the entry is
finished airing; the episode played is its last; exactly one sequel
remains after dropping recaps; that sequel is a TV, ONA or OVA
continuation or a film the databases link as the sequel; and it has
aired. Otherwise the end card shows the choices (or nothing) and
Continue keeps today's behaviour.

**Edge cases.** Story-order links run backwards in time — the 2000
*JoJo* OVA's one sequel is the 1993 OVA: still offered, since it is
what the databases call next, with "aired 1993" on the card.
Branching runs never advance: *Fate/Zero 2nd Season* has four
sequels, so its finale shows the choices. A film
inside a run is offered when the databases link it as the sequel,
and not otherwise. A sequel the provider does not carry fails on
click the way any unavailable show does today; an option worth the
maintainer's decision is to check the one sequel's availability while
the finale plays, as the player already warms the next episode in the
background — one check, only at a finale, only for one show.

**Trackers.** No new writes. The finale's play writes the finished
entry's progress as now; the sequel's first play writes the sequel's
entry, through its own episode map. Where Kitsu and the tracker split
entries differently (one Kitsu entry against two MyAnimeList ones, or
the reverse), the existing episode map already decides which tracker
entry each Kitsu episode belongs to; nothing here bypasses it.

**Cost.** Medium. The Continue Watching card is the most intricate
surface in the app (row matching, repair, resume positions, removal);
moving a card to another Kitsu entry touches its matching and removal
rules, and wants its own acceptance tests. The player end card is
smaller.

### (c) A franchise page

**How it looks.** `/anime/<id>/franchise`, reached from the strip's
trailing link and from search results that belong to a franchise. A
header named after the earliest entry of the main run, in that
entry's accent colour, with your position: "Part 5 of 9 · *Golden
Wind* · EP 12" and a Resume button. Below it, sections:

1. **Main story** — the prequel/sequel chain, numbered, with progress
   bars per entry and a "you are here" marker.
2. **Films**, **Side stories & specials**, **Recaps** (collapsed by
   default), **Other versions** (remakes and routes: the 1993 and
   2000 OVAs for JoJo; *Fate/stay night*'s routes).

A two-way switch, *Linked order* / *Air date*, appears only when the
two differ for this franchise, with a one-line note that linked order
is how the databases connect the entries and may follow the story.

**Data.** `chain` and `around` for each entry of the run, plus progress from the local history
(Kitsu id per row) and, where a tracker is connected, the cached
tracker list (status and episode count per MyAnimeList id, through
Kitsu's mappings, which the app already caches for 30 days).

**Edge cases.** Huge franchises (Dragon Ball, Gintama): the walk is
bounded and the page says "showing 40 of more"; Kitsu pages are
cached for a week, so a second visit costs nothing. Branching chains
(Fate) render as the chain up to the branch and the routes side by
side. Entries without a Kitsu id in the history (the hianime "Gintama
Season N" rows) contribute no progress.

**Trackers.** Read-only.

**Cost.** Medium to large: a new route, `around` and its paging, the
classification into sections, and the design work AGENTS.md §7 asks
of a new surface. It reuses (a)'s `chain`.

### (d) One Continue card per franchise

**How it looks.** Continue Watching shows one card per franchise —
the most recently watched entry — with the franchise named as an
eyebrow ("JoJo's Bizarre Adventure · Part 5"). The entries stay
separate everywhere else: their own pages, histories and tracker
entries.

**Edge cases.** Two entries of one franchise watched in parallel
(a rewatch of part 1 while part 6 airs) collapse to one card and
hide the other — the case Netflix avoids by owning the numbering.
Rows without a Kitsu id do not group. Grouping on the home page means
walking relations for every Continue row on load (Kitsu requests on
the busiest screen), or caching franchise membership as rows are
written.

**Trackers.** None.

**Cost.** Medium, with the most risk of the four: it changes what
Continue means.

[inference] Most of what (d) promises is already delivered by (b)
(the card moves forward at a finale and the finished entry gives way
to the sequel) and by (a)/(c)'s "You're on *Golden Wind*" line. What
(d) adds beyond that is hiding parallel watches, which is the part
most likely to surprise.

## 5. Recommendation and plan for 0.19.0

**Do not merge entries.** Keep every Kitsu entry its own show; build
the connections around it. This matches what the research found:
platforms that merge own their catalogue, and the databases this app
reads store edges, not franchises.

**Treat order honestly.** Show the databases' links as the watch
order, mark where they run against air dates, offer air-date order on
the franchise page, and never claim a "story order" the data does not
support.

**Phases**, each shippable on its own:

1. **Watch order strip** (option a), with the `chain` walk — the
   strip's position number and its "You're on …" line both need the
   whole main run, not just the neighbours — and that line. Read-only, no provider requests, no
   change to Continue or trackers. On its own it answers "what
   comes before and after" and most of the JoJo request.
2. **Up next at a finale** (option b): the player end card first, then
   Continue moving to the sequel and the finished entry's card giving
   way. Decide first whether to check the sequel's availability during
   the finale.
3. **Franchise page** (option c), reusing phase 1's walk and adding
   the side stories, films, recaps and other versions around it; the
   strip gains its trailing link to the page here.
4. **One card per franchise** (option d): not planned. Revisit after
   phase 2 ships, if hunting for the current part is still a
   complaint.

**Decisions for the maintainer.**

- Whether phase 2 may make one availability check for the single
  sequel while a finale plays (otherwise a sequel the provider lacks
  is found out on click).
- Whether auto-play may cross into another entry at all, or only
  shows the card.
- Whether the franchise page belongs in 0.19.0 or follows it.

## Live requests made for this proposal

All on 2026-11-01, sent one at a time with a pause of two seconds or
more between them, 21 in total:

- Kitsu REST, 17: the franchise list (one page of two), four
  installments routes (all HTTP 500), one slug lookup, one
  MyAnimeList-id mapping lookup, nine `media-relationships` reads
  (818, 3919, 43078, 46598, 8063, 7730, 41982, 46903, 5649) and the
  role-filter test on 818.
- Kitsu GraphQL, 1: a franchise lookup the schema does not have.
- AniList GraphQL, 3: two relations queries (nine and eight entries)
  and one schema introspection of `MediaRelation`.

The Fate and Ranma relation samples are the repository's recorded
fixtures under `tests/fixtures/tracker-map/wire/`, and the unaired
sequel of Kitsu 48677 one under `tests/fixtures/hianime/appended/`;
none of them are live reads.
