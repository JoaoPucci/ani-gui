# Proposal: watch order — what comes before, what comes next, and the franchise around a show

**Status**: proposed as the main feature of the 0.19.0 release. The
product questions are decided (section 6); the design questions are
open (section 5). Nothing below is built. The research was done on
2026-11-01; the live samples it quotes describe the catalogues on that
day and will drift.

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
   fourth part of *Thousand-Year Blood War*, the fifth entry since
   *Bleach* began, which part comes before it, or that you have
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
automatically play the next episode in a series" ([Netflix Help: How
to autoplay the next
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
read held every relation Kitsu counts. *Ranma ½* (Kitsu 186, a
repository fixture) has 15, *Gintama* (818) 15. [inference] Long
franchises with many specials can pass 20, and then a `sequel` edge
may be on a page not read. [data] Kitsu honours a role filter on this
endpoint: `filter[role]=sequel,prequel` on *Gintama* answered `count:
1` instead of 15 — a cheap way to ask only for the edges a watch order
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
| *Gintama* (818) | sequel *Gintama'* (2011); four specials as `side_story`; film 1, the 2012 *Yorinuki* film and the 2026 film as `alternative_version`; film 2 as `other`; the 2025 *Ginpachi-sensei* spin-off as `side_story` | Linear; films classified loosely. |
| *Bakemonogatari* (3919) | sequels *Nisemonogatari* (2012) **and** *Kizumonogatari I* (2016); a recap as `summary` | **Release order**, and two sequels. |
| *Kizumonogatari I* (5649) | prequels *Bakemonogatari* and *Koyomimonogatari*; sequel *Kizumonogatari II* | Mostly **release order** (*Bakemonogatari*); its *Koyomimonogatari* prequel aired two days later. In story order *Kizumonogatari* comes first. |
| *Fate/Zero* 2nd season (7658, repository fixture) | prequel *Fate/Zero*; **four sequels**: *Fate/stay night* (2006), the *Unlimited Blade Works* film (2010), the *Heaven's Feel* film (2017), the UBW prologue (2014) | **Story order** (2006 is the "sequel" of 2012), and one sequel per route. |
| *Fate/stay night: UBW* (7882, repository fixture) | prequel the UBW prologue; sequel UBW 2nd season; the 2006 series and both films as `alternative_version` | The routes are alternatives of each other. |

[inference] Kitsu's prequel/sequel edges are reliable for linear runs —
every TV continuation sampled (Bleach, TYBW, Attack on Titan, JoJo's
TV parts, Gintama) is linked correctly and recaps are marked
`summary`. They are **not** consistently release order or story
order: *Fate/Zero* and the *JoJo* OVAs follow story order,
*Monogatari* mostly release order (the next section reads every link
in ten franchises). And an entry can have several
sequels (Fate's routes; *Bakemonogatari*), so "the next one" is not
always a single answer.

### Is Kitsu's link order the story order?

The question matters because the proposal's default (section 4)
follows Kitsu's links. To answer it, every prequel/sequel link in ten
franchises was read: the seven largest in the author's library (Dragon
Ball, Gintama, Digimon, Saint Seiya, JoJo, Naruto, Bleach) plus
*Attack on Titan*, *Monogatari* and *Fate*. The read started from
known entries of each franchise and followed prequel/sequel links
outward until no new entry appeared (live reads on 2026-11-01, listed
at the end). It saw only links listed by entries it read; a link
listed solely by an entry nothing else points to stays invisible — the
2000 *JoJo* OVA was reached only because it was a starting point, and
the app's own walk has the same blind spot. A link is a pair of
entries joined as prequel and sequel, from either side; 101 pairs were
found. One more pair, *Kamitsubaki City Under Construction*, was read
because it had looked wrong in use.

[data] Each pair was compared with the two entries' start dates:

| Franchise | Pairs | Agree with release dates | Run against them | Dates missing |
|---|---|---|---|---|
| Dragon Ball | 13 | 12 | 0 | 1 |
| Saint Seiya | 8 | 5 | 3 | 0 |
| JoJo | 10 | 8 | 2 | 0 |
| Digimon | 14 | 14 | 0 | 0 |
| Naruto | 4 | 3 | 0 | 1 |
| Bleach | 4 | 4 | 0 | 0 |
| Attack on Titan | 7 | 7 | 0 | 0 |
| Fate | 9 | 7 | 2 | 0 |
| Gintama | 11 | 10 | 1 | 0 |
| Monogatari | 21 | 18 | 2 | 1 |
| Kamitsubaki | 1 | 0 | 1 | 0 |
| **Total** | **102** | **88** | **11** | **3** |

The eleven links that run against the release dates, and what each
follows ([inference] — the story placement is general knowledge of
these works, not something the data says):

| Link (prequel → sequel, as Kitsu has it) | Aired | Follows |
|---|---|---|
| *Saint Seiya: The Lost Canvas* → *Saint Seiya* | 2009 → 1986 | Story: *The Lost Canvas* is set centuries earlier |
| *The Lost Canvas* 2 → *Saint Seiya* | 2011 → 1986 | Story, as above |
| *Hades – Elysion* → *Tenkai-hen Josou – Overture* | 2008 → 2004 | Story: the film is set after the Hades war |
| *JoJo* 2000 OVA → *JoJo* 1993 OVA | 2000 → 1993 | Story: the 2000 OVA adapts the start of Part 3, the 1993 OVA its end |
| *JoJo* (TV, 2012) → *JoJo* 2000 OVA | 2012 → 2000 | Story: Parts 1–2 come before Part 3 (listed only on the OVA's side) |
| *Fate/Zero 2nd Season* → *Fate/stay night* | 2012 → 2006 | Story: *Fate/Zero* is the earlier war |
| *Fate/Zero 2nd Season* → *Unlimited Blade Works* (film) | 2012 → 2010 | Story, as above |
| *Gintama: The Semi-Final* → *Gintama: The Final* | 2021-01-15 → 2021-01-08 | Story: a lead-in streamed a week after the film opened ([Anime News Network](https://www.animenewsnetwork.com/news/2021-01-07/gintama-the-semi-final-net-anime-special-posts-new-trailer/.168186): the film opened in Japan on 8 January 2021, the special's first episode on 15 January) |
| *Kizumonogatari III* → *Nisemonogatari* | 2017 → 2012 | Story, skipping the entries set in between |
| *Koyomimonogatari* → *Kizumonogatari I* | 2016-01-10 → 2016-01-08 | Neither: *Kizumonogatari* is the earliest story, and the gap is two days of release |
| *Kamitsubaki* (TV) → *Witchling – Zenpen* (special) | 2025-07-09 → 2025-07-03 | Not established: the special aired six days before the series it is listed after |

And among the 88 links that agree with the release dates, some go
against the story where the two differ ([inference], same caveat):
*Bakemonogatari* → *Kizumonogatari I* and *Nisemonogatari* →
*Nekomonogatari (Kuro)* follow release order, though both films and
*Kuro* are set before *Bakemonogatari*. Others link a re-edit or a
remake as a continuation: *Dragon Ball* → *Dragon Ball Kai* (a re-edit
of *Z*), *Dragon Ball Kai (2014)* → *Dragon Ball Super* and → *Dragon
Ball Super: Beerus* (2026). Kitsu marks both *Kai* series as
`alternative_version` of *Z*, which section 4's rules use to keep them
out of the run.

**What this says** [inference]. The hypothesis that a link which looks
wrong is a story-order link holds for the cases sampled: of the
eleven links that run against the release dates, eight follow the
story (nine counting *Kizumonogatari III* → *Nisemonogatari*), one
follows neither and one could not be judged. But the converse does not
hold. Kitsu's links do not consistently encode story order: where the
two orders differ, *Monogatari*'s TV entries follow release, and
*Dragon Ball*'s links treat versions as continuations. The fairest
reading is that each link records what its editor judged to come next.
Usually that is the release order, often the story order, and the
data does not say which.

### Where the links come from

Labels here: [code] is what Kitsu's published server code shows;
[not found] is a search that came back empty; [not checked] is a
question left open.

- **Kitsu** [code]. The relationship table was added to
  Kitsu's server in January 2017
  ([kitsu-server pull request 44](https://github.com/hummingbird-me/kitsu-server/pull/44),
  migration `20170116051758_create_media_relationships.rb`). Creating
  or changing a relationship needs the `database_mod` permission
  ([`media_relationship_policy.rb`](https://github.com/hummingbird-me/kitsu-server/blob/the-future/app/policies/media_relationship_policy.rb)),
  so only accounts holding that permission — Kitsu's database
  moderators — can write a link directly. [not checked] Kitsu also has
  a user-submitted wiki-draft flow (`wiki_submission` mutations, added
  2021); whether it can propose relationship changes for a moderator
  to approve was not checked. The roles are a bare list
  ([`media_relationship.rb`](https://github.com/hummingbird-me/kitsu-server/blob/the-future/app/models/media_relationship.rb);
  the GraphQL enum, [`media_relationship_kind.rb`](https://github.com/hummingbird-me/kitsu-server/blob/the-future/app/graphql/types/enum/media_relationship_kind.rb),
  describes the field as "The relationship kind from one media entry
  to another" and gives the values no descriptions). [not found] No
  definition of the roles, no statement of whether prequel/sequel
  follows story or release order, and no record of where the initial
  2017 data came from (an import from another database, or entry by
  hand) was found in Kitsu's code or in a web search. [inference] With
  no written rule, each link reflects its moderator's judgement, which
  matches the mixed pattern above.
- **AniList** [stated]. Its API schema describes `PREQUEL` as
  "Released before the relation" and `SEQUEL` as "Released after the
  relation" (introspection, section 1). A contributor manual written
  for AniList says relations "should be in ***chronological*** order"
  ([gitbook](https://anilist2.gitbook.io/submission-manual/submission-form/relations);
  the page does not say it is official). The two disagree; AniList's
  *Monogatari* data follows the story (section 2).
- **MyAnimeList** [not found]. No published definition of its "Related
  Entries" prequel/sequel was found; its help centre and guideline
  pages refused automated reads, and a web search turned up only an
  old manga-database guideline on its forum, which does not define
  them for anime.

[inference] For trusting the default: of the sources, only AniList's
schema states a rule, and its own data departs from it. Kitsu states
none. Link order is therefore "what the editors thought comes next":
right for straight runs, and for the out-of-order cases it mostly
gives the story order, which is what someone asking "what comes next"
usually wants. Release order is the one ordering the app can compute
exactly from the data, which is why it is offered as the alternative.

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
touched in 2013, and the route to their entries is broken. Do not
build on it.

### AniList `relations` (not read yet)

[data] Listed in
[`deferred-work.md`](../deferred-work.md#metadata-the-app-does-not-use-yet)
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
can follow the order the databases link entries in, or the order they
aired; it cannot promise a correct story order. Which to use by
default is section 4's question.

### What the streaming provider adds

[data] The provider lists versions as separate shows: the author's
development-build history holds both "JoJo's Bizarre Adventure Part 2:
Stardust Crusaders" and its "(Uncensored)" listing, both recorded
against Kitsu 8063, and the provider names that entry "Part 2" where
Kitsu and the franchise call it Part 3. hianime also renames
*Gintama*'s entries "Season 2", "Season 4", "Season 5"; in the same
history those rows carry no Kitsu id, where other rows written the
same day do. [inference] Somewhere between the play and the history
write, the app did not settle which Kitsu entry those listings are;
the exact cause was not traced.

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
family (a crude title match, which over-counts — *Fullmetal Alchemist*
and *Brotherhood* are separate adaptations, not a sequel pair): about
205 entries, 70 %, sit in 43 families of two or more. The largest:
Dragon Ball 32, Gintama 20, Digimon 12, Saint Seiya 12, JoJo 10,
Naruto 10, Bleach 8, Yu-Gi-Oh! 7. [inference] For this viewer,
franchises are the normal case, not the edge case, and they are long:
most of them carry films or specials beside the run of series.

[data] Where "what's next" would have mattered, from the installed
build's history:

- ***Bleach: Thousand-Year Blood War***. Part 1 was played to episode
  13 of 13 — its finale — on 2026-10-05. [inference, from the Continue
  rule in `frontend/src/lib/play/next-episode.ts`] Its card now offers
  episode 13 again. The history then has *The Separation* (part 2) and
  *The Calamity* (part 4) rows, and no row for *The Conflict* (part
  3); neither tracker list has *The Conflict* either. [inference]
  Whatever the reason, a show page for *The Calamity* that named its
  prequel and marked it unwatched would have made the gap visible.
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

- `chain(kitsu_id)`: the **run** an entry belongs to — a bounded walk
  along prequel/sequel edges from that entry (say up to 40 entries),
  each with title, format, year, episode count, status and poster,
  under the rules below. It reads Kitsu's relations role-filtered to
  `sequel,prequel,alternative_version,alternative_setting` — fewer
  rows than the full list (four instead of fifteen for *Gintama*), and
  paged past 20 where Kitsu counts more, so no link is hidden — a new,
  separately cached query beside the existing `kitsu:relations:v3`
  read, which asks for neither titles nor posters. One request per
  entry the walk considers, cached; JoJo's TV run, from the 2012
  series to *Steel Ball Run*, is nine entries.
- `around(kitsu_id)`: one entry's other relations — side stories,
  recaps, films and alternative versions — read in full, paging past
  20 where Kitsu counts more. They are attached as leaves and not
  walked from, because `alternative_version`/`ALTERNATIVE` links
  unrelated series (Fate's *Apocrypha* and *Grand Order* hang off
  *Fate/stay night*).

Neither makes a provider request. Kitsu is the metadata service the
app already pages through on every show page.

### How the walk builds a run, and keeps other versions out

A prequel/sequel link does not by itself mean "the same run". [data]
The 2000 *JoJo* OVA (Kitsu 7730) names the 2012 TV series as its
prequel and the 1993 OVA as its sequel, and *Dragon Ball* names *Dragon
Ball Kai*, a re-edit of *Z*, as a sequel. A walk that followed every
link would fold the OVAs into JoJo's TV run and *Kai* into Dragon
Ball's, contradicting the franchise page's "Other versions". The walk
is defined from the entry being viewed — the anchor — by these rules
[proposal]:

1. **Walk the links, both ways.** Starting at the anchor, the walk
   reads each entry it adds and follows the prequel and sequel links
   that entry lists, breadth-first, one distance from the anchor at a
   time. An entry first met at a given distance takes as its path the
   one through the listing entry that started earliest (then the
   lowest Kitsu id). Each entry is decided once, on that path: an
   entry left out is not reconsidered through another entry, and is
   not walked from.
2. **Versions side by side keep one.** When an entry lists several
   sequels (or several prequels) of which some list one another, in
   either direction, as `alternative_version` or
   `alternative_setting`, only one of each such group is added: the
   one in the same format as the entry listing them (TV, movie,
   OVA…), else the one that started first, else the lowest Kitsu id.
3. **Versions along the path are left out.** An entry is not added if
   it and any entry on its path to the anchor list each other (in
   either direction) as `alternative_version` or
   `alternative_setting`. The anchor is never left out.
4. **Other versions** are the anime entries rules 2 and 3 left out,
   plus the anime that run members list as `alternative_version`.
   `alternative_setting` is not listed: Kitsu uses it for separate
   series in the same world (*Fate/stay night* lists *Apocrypha*,
   *Prisma☆Illya* and *Grand Order* that way). Manga and novels are
   never listed.

With these tie-breaks every entry has one path and one decision, so
the result does not depend on the order the walk happens to read
things in. [data] Checked on the links read for section 2: with every
one of its 101 entries as the anchor, 300 random read orders each gave
the same run and the same other versions. The walk costs one cached
read for each entry it considers, added or not.

**Order.** The rules above decide which entries form the run. A
single app-wide setting (section 5) decides the order they are shown
in:

- **Link order** (the default): the order the links give, using every
  prequel/sequel link between two run members, whichever of the two
  lists it. A link that a longer path through the run already covers
  is a shortcut and is ignored. Cycles are taken as a block: entries
  that lead round to one another (as ten *Monogatari* entries do)
  count as a single step, links inside a cycle are never shortcuts,
  and a link counts as covered only by a path that does not pass back
  through the entry it starts from. [data] Digimon's *Last Evolution
  Kizuna* lists *tri. 6* as its prequel, while *tri. 6* lists the 20th
  anniversary story as its sequel and that story lists *Kizuna* as its
  sequel, so the direct *tri. 6* → *Kizuna* link adds nothing and
  would otherwise show a fork that is not one. An entry with two
  remaining next entries is a fork; both are shown as the ways to
  continue. An entry with two remaining previous entries is a join.
- **Release order**: every run member in order of start date (an
  unknown date last), as one line with no forks.

**Position numbers.** In link order, positions count along the
anchor's own path from the run's first entry: the entry reached by
following prequels from the anchor (shortcuts ignored), taking the
earliest-starting one at a join and stopping at a cycle. Entries after
a forward fork, and entries in a cycle, are not numbered, and a total
("of 05") is shown only for a run with no fork, join or cycle. In
release order, the position is the entry's place in the date-ordered
line. A position is shown as a plain number ("05"), not as a "part":
franchises use "part" for their own divisions (JoJo's Parts 1–9,
*Thousand-Year Blood War*'s cours), and the run's count does not match
them.

### What the order setting changes

[inference: the [proposal] rules applied to the [data] links read for
section 2] One anchor per franchise, two for JoJo and Fate. The
computation used the links from both entries' listings; following only
each entry's own listing, as rule 1 says, gives the same members and
other versions for every anchor below. "Differ" counts the run members
whose previous or next entry is not the same in the two orders.

| Franchise (anchor) | Run members | Differ | Forks and joins in link order |
|---|---|---|---|
| Attack on Titan (season 1) | 8 | 0 | none |
| Bleach (2004 series) | 5 | 0 | none |
| Naruto (2002 series) | 4 | 0 | none |
| JoJo (2012 series) | 9 | 0 | none |
| JoJo (the 2000 OVA) | 3 | 3 | none |
| Fate (*Unlimited Blade Works*) | 5 | 0 | none |
| Digimon (*Adventure*) | 14 | 0 | none (one shortcut ignored) |
| Fate (*Fate/Zero*) | 6 | 3 | fork after *Fate/Zero 2nd Season* |
| Dragon Ball (1986 series) | 8 | 3 | fork after *Z* |
| Gintama (2006 series) | 11 | 2 | join at *The Final* (one shortcut ignored) |
| Saint Seiya (1986 series) | 8 | 8 | none (one shortcut ignored) |
| Monogatari (*Bakemonogatari*) | 16 | 13 | seven forks, five joins, ten entries in a cycle |

Where they differ, entry by entry:

- **JoJo, from the 2000 OVA.** Link order: *JoJo* (TV, 2012) → 2000 OVA
  → 1993 OVA. Release order: 1993 OVA → 2000 OVA → 2012 series. All
  three entries' neighbours change. Other versions: both *Stardust
  Crusaders* seasons, the 2007 *Phantom Blood* film and the 2012
  re-edit special. From any TV part, the run is the nine entries from
  the 2012 series to *Steel Ball Run* (an ONA), the same in both
  orders. No entry in it lists either OVA as a prequel or sequel, so
  the walk never meets them; both are other versions by rule 4, since
  run members list them as `alternative_version`.
- **Fate, from *Fate/Zero*.** Link order: *Fate/Zero* → *2nd Season* →
  either *Fate/stay night* (2006) or the UBW prologue → UBW → UBW 2nd
  season. Release order: *Fate/stay night* (2006) → *Fate/Zero* → *2nd
  Season* → prologue → UBW → UBW 2nd season. They differ at *Fate/stay
  night* (2006), *Fate/Zero* and its 2nd season. The 2010 UBW film and
  the first *Heaven's Feel* film are other versions: with *Fate/stay
  night* (2006) they list one another as versions, and the TV series
  wins (rule 2). Rule 4 adds *Fate/Prototype* and a UBW 2nd-season
  special, which run members list as versions; the same-world series
  (*Apocrypha* and the rest) are not listed. From the UBW series' own
  page, the run is *Fate/Zero* → *2nd Season* → prologue → UBW → UBW
  2nd season in both orders; *Fate/stay night* (2006) is then an other
  version (rule 3: UBW lists it).
- **Dragon Ball.** Link order: *Dragon Ball* → *Z* → either *GT* (→ its
  1997 special) or *Super* → *Broly* → *Super Hero* → *Ginga Patrol*.
  Release order puts *GT*'s special before *Super*, so *Z*, the special
  and *Super* differ. Other versions include both *Kai* series and
  *Daima*: Kitsu lists *Daima* as a version of *GT*, and rule 2 keeps
  the earlier one. [inference] Viewers may not expect *Daima* there;
  it is what the data says.
- **Gintama.** Both orders run *Enchousen* → the second film →
  *Gintama°* (the direct *Enchousen* → *Gintama°* link is a shortcut).
  They differ only at the end. [data] Kitsu stores *The Semi-Final*
  (special, start 2021-01-15) with *The Final* (film, start
  2021-01-08) as its sequel, and *The Final* names both *The
  Semi-Final* and the second *Silver Soul* part as its prequels. In
  link order, *The Final* is a join: the *Silver Soul* finale and *The
  Semi-Final* both lead to it. In release order, *The Final* comes
  first and *The Semi-Final* follows it. Two entries differ.
- **Digimon.** No difference once the shortcut is ignored. [data]
  *tri. 6* (film, 2018-05-05) lists the *20th Anniversary Memorial
  Story* (ONA, 2019-11-22) as its sequel; that story lists *Last
  Evolution Kizuna* (film, 2020-02-21) as its sequel; and *Kizuna*
  lists both the story and *tri. 6* as prequels. Taken literally, the
  last link would make *tri. 6*'s strip offer "2 ways to continue"
  (the story or *Kizuna*) and *Kizuna*'s strip show two entries before
  it, where release order shows a single line, *tri. 6* → story →
  *Kizuna*. With the shortcut ignored, link order is that same line.
- **Saint Seiya.** Link order: *The Lost Canvas* → *Lost Canvas 2* →
  *Saint Seiya* (1986) (the direct *Lost Canvas* → 1986 link is a
  shortcut) → the Hades OVAs → *Elysion* → *Overture* → *Omega*.
  Release order: *Saint Seiya* (1986) → Hades *Sanctuary* (2002) →
  *Overture* (2004) → Hades *Inferno* (2005) → *Elysion* (2008) → *The
  Lost Canvas* (2009, 2011) → *Omega* (2012). Every entry differs.
- **Monogatari.** Link order is a web, and a circular one. [data]
  *Kizumonogatari I* → *II* → *III* → *Nisemonogatari* → *Nekomonogatari
  (Kuro)* → *Second Season* → … → *Koyomimonogatari* → *Kizumonogatari
  I* closes a loop through ten entries. No link is a shortcut once the
  loop is taken as a block; seven entries fork and five are joins
  (*Owarimonogatari 2nd Season* has three entries before it). Release
  order is a single line from *Bakemonogatari* (2009) to *Off &
  Monster Season* (2024), then its undated special. Thirteen entries
  differ.

[inference] For most of the library the setting changes nothing. It
matters for franchises with prequel stories and remakes (Saint Seiya,
Fate, the JoJo OVAs) and for *Monogatari*, where link order is a web
rather than a sequence.

### (a) A "Watch order" strip on the show page

**How it looks.** A row under the hero, before the episodes: the
entry's run (section above) around it — the entry before, this entry,
the entry after, in the order the app-wide setting chooses — each
with poster, title, year, format (TV, Movie, OVA), episode count, and
your progress (from the local history: "Watched", "EP 9 of 13",
nothing). The current entry is marked in the show's accent colour;
its position in the run uses the app's large tabular numerals ("05"
for *The Calamity*, counted from *Bleach*, the run's first entry).
Once the franchise page (c) exists, a link opens it; before then the
strip has no such link (section 5, "The strip, second round", has its
two forms). If you have progress elsewhere in
the franchise, the strip says so: "You're watching *Golden Wind* ·
EP 12", a link to that entry's page (section 5, "The strip, second
round", compares its forms).
That one line answers the JoJo request on whichever JoJo
page the viewer lands.

**Which order.** One app-wide setting decides (section 5): link order
by default, release order if chosen. The strip has no toggle of its
own and marks nothing as "out of order"; each card already shows its
year.

**Edge cases.**

- A fork, in link order (*Fate/Zero 2nd Season* continues into
  *Fate/stay night* (2006) and the UBW prologue; *Bakemonogatari* into
  *Nisemonogatari* and *Kizumonogatari I*): the "next" slot becomes a
  small fan — "2 ways to continue" — in air-date order, not a guess,
  and the branches are unnumbered. *Fate/Zero 2nd Season*'s other two
  sequel links, the 2010 film and the first *Heaven's Feel* film, are
  other versions (rule 2): not offered as next, and listed on the
  franchise page. In release order there are no forks.
- A join, in link order (*The Final* follows both the *Silver Soul*
  finale and *The Semi-Final*): the "before" slot holds both, side by
  side, the way a fork fills the "next" slot. The position number
  follows the earliest-starting one (section 4).
- Recaps (`summary`) never take a slot; the role filter leaves them
  out.
- An unaired sequel (a start date in the future — the repository's
  fixture for Kitsu 48677 has a sequel starting 2027-01-10 with no
  episode count): shown dated,
  "Coming January 2027", not playable.
- An entry the provider does not carry: the strip shows it anyway, as
  the catalogue does; the show page it opens runs the existing
  availability check. The strip does not dim entries from cached
  verdicts either, for the same reason the finale card does not (see
  option b): the order is shown as it is, and the show page says what
  is unavailable.
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
  an end card — the sequel's banner, its title, its year and format,
  under "Up next". The card waits for a click. It does not count down
  or play the sequel by itself, whatever the auto-play setting:
  auto-play stays within one entry, as it does today.
- *On Continue Watching*: once the finale has been played through,
  the card stops offering the finale again and shows the sequel —
  its art, its title, "Up next".
- *What a click does*, on either card: it opens the sequel's show
  page, where Play starts episode 1. It does not start playback
  directly, because nothing has asked the provider about the sequel
  (below), and the show page is where the app already answers that
  question. When the sequel has its own history row (you started
  it), the finished entry's card gives way to the sequel's ordinary
  Continue card, so the Continue Watching rail does not show both.

**Rules** [proposal]. The end card appears only on the last episode of
an entry that has finished airing. Its candidates are the entry's next
entries in the run, in the order the setting chooses (section 4):
every run member it links to as a sequel in link order, shortcuts
ignored, or the one that started next in release order. Recaps and
other versions are never candidates; no other filter applies. What the
card shows:

- exactly one that has aired: that entry, as "Up next";
- exactly one that has not aired: that entry with its date
  ("Coming January 2027"), as information only;
- several: all of them, as the choices;
- none: no card.

Continue moves to the next entry in the first case only, and only to
an entry the viewer has not played yet; in every other case it keeps
today's behaviour. The second condition matters in a cycle: in link
order *Kizumonogatari III*'s finale names *Nisemonogatari*, and
without it Continue could lead round the ten-entry *Monogatari* loop
to entries already watched. The card still names the entry.

**Edge cases.** The 2000 *JoJo* OVA's finale names the 1993 OVA in
link order and the 2012 TV series in release order — whichever the
setting chooses, as one entry. The 2012 *JoJo* series' finale names
*Stardust Crusaders* in both orders: the series does not list the OVA
at all. In link order, Continue never moves on a fork: *Fate/Zero 2nd
Season*'s finale card shows *Fate/stay night* (2006) and the UBW
prologue as the choices; in release order its next entry is the
prologue alone. A film inside a run is offered when the databases
link it as a sequel, and not otherwise.

**A sequel the provider does not carry** [decided]. The card names the
next entry the relations give, whether or not the provider carries it,
and nothing asks the provider about it at a finale — no new request,
and no cached verdict either, so the card never differs by what
happens to be cached. Offering "the next one that is available"
instead would raise a question the viewer should not have to answer —
whether the app is skipping something — and hide the real order. A
click opens the sequel's own show page, which runs the existing
availability check and says when the show is unavailable (in the
chosen audio, or at all); its watch-order strip then shows what comes
after it, so the viewer can carry on from there.

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
header named after the run's first entry, with your position: "05 ·
*Golden Wind* · EP 12" (with a total only for a run with no fork, join
or cycle) and a Resume button; its hero and accent are set in the
design section. Below it, sections:

1. **Main story** — the run (section 4's rules), in the order the
   setting chooses, numbered as section 4 describes, with branches
   unnumbered beside the numbered path, progress bars per entry and a
   "you are here" marker.
2. **Other versions** — section 4's rule 4: from the JoJo TV run, both
   OVAs; from the UBW run, the 2006 *Fate/stay night*, the 2010 film
   and the first *Heaven's Feel* film among them; *Gintama*'s
   retelling films.
3. From `around`, by role first and format second: **Recaps**
   (`summary`, collapsed by default — *Attack on Titan: Chronicle*
   lands here though it is a film), then **Films** (movies among the
   rest) and **Side stories & specials** (everything else: side
   stories, parent stories, spin-offs, `other`). Links of the
   `character` and `adaptation` roles are not shown: they tie
   unrelated series together (*Gintama* to *Saiki Kusuo*).

There is no order switch on the page; the app-wide setting applies.

**Data.** `chain` and `around` for each entry of the run, plus
progress from the local history (Kitsu id per row) and, where a
tracker is connected, the cached tracker list (status and episode
count per MyAnimeList id, through Kitsu's mappings, which the app
already caches for 30 days).

**Edge cases.** A run longer than the walk's bound (about 40 entries;
the largest run sampled, *Monogatari*, has 16) ends with "showing 40
of more"; Kitsu pages are cached for a week, so a second visit costs
nothing. Forked runs (Fate seen from *Fate/Zero*) render as the
numbered path and the branches side by side; a join shows its earlier
entries side by side above it; the entries of a cycle are listed as
one unnumbered group, in air-date order, where the cycle sits in the
run. Entries without a Kitsu id in the history (the hianime "Gintama
Season N" rows) contribute no progress.

**Trackers.** Read-only.

**Cost.** Medium to large: a new route, `around` and its paging, the
classification into sections, and the design work AGENTS.md §7 asks
of a new surface. It reuses (a)'s `chain`.

### (d) One Continue card per franchise

**How it looks.** Continue Watching shows one card per franchise —
the most recently watched entry — with the franchise named as an
eyebrow ("JoJo's Bizarre Adventure · *Golden Wind*"). The entries stay
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
to the sequel) and by (a)/(c)'s "You're watching *Golden Wind*" line. What
(d) adds beyond that is hiding parallel watches, which is the part
most likely to surprise.

## 5. Design

The interface matters most in this feature, and it binds every phase.
`ani-gui` exists because the interface matters: it began as a
graphical front end to a command-line tool, and people already use
it. The new surfaces must read as parts of the pages they join, in the
app's existing visual language, legible at a glance, and placed where
the question they answer comes up. A surface that works but looks like
a bolted-on widget does not ship.

### The baseline: the app's existing design language

This section describes what the app does today, as read from
`frontend/src/lib/design/tokens.css` and the components named below.
AGENTS.md §7–§9 set the direction: Netflix-level polish, motion as
structure, no generic dark-mode widgets, and the interface as the top
priority.

- **Mood and colour.** Its tokens file describes the look as
  "late-night repertory cinema, not streaming-service bombast". The
  background is a warm near-black (`--ink-000`, `#0c0a09`) with lifted
  surfaces (`--ink-050`, `--ink-100`). Text is bone white rather than
  pure white (`#fff` appears only in a few player controls and the
  window's close button):
  `--bone-100` for primary, then `--bone-200`/`-300` for secondary and
  meta text and `--bone-400` (lifted to about 4.1:1) for placeholders.
  Each show gets one accent from a palette of eight film-ink tones
  (oxblood, ink, persimmon, jade, dusk, ochre, plum, slate), picked by
  hashing its Kitsu id (`$lib/design/accent.ts`). The show page, its
  Play button's glow and each poster card's bottom rule take that
  accent. The app-wide focus ring (`--ring`) is declared once on the
  root, so it is the default ink accent everywhere. The app's own
  brand colour, a warm sienna, is kept apart from the per-show
  accents.
- **Type.** Three voices. A display serif (`--font-display`; Fraunces
  first, then editorial serifs) is set in italics for show titles on
  the show page. A sans (`--font-body`) carries the interface and card
  titles. A mono (`--font-mono`, tabular figures) carries the
  oversized numerals (the Continue card's "EP N") and the show page's
  "Synopsis" heading; the "Episodes" heading is in the sans. The scale
  floor is 12 px for micro text, 14 px for meta and 16 px for body. An
  oversized-numeral size (`--type-numeral-xl`) exists
  for the episode-count motif.
- **Section labels.** Sections open with an "eyebrow" (13 px, the sans
  at weight 600): an uppercase, letter-spaced key, a hairline rule,
  then a quieter value. Examples
  are "Similar titles — via Kitsu search" and, on the show page, the
  subtype and the airing status.
- **Spacing and shape.** Spacing sits on a strict 4 px scale with no
  step above 64 px. Cards in a `Strip` rail sit 24 px apart, and
  sections are spaced 48–64 px. Card corners are 8 px, controls 6 px,
  and pills fully rounded.
- **Motion.** Motion is structural: pages and sections enter, posters
  lift a few pixels on hover with a slight elastic overshoot, and the
  hero parallaxes. There are three durations (140, 260 and 480 ms), all
  set to zero under `prefers-reduced-motion`. Horizontal rails (`Strip`)
  are explicitly not carousels: the viewer scrolls them, arrow buttons
  show whenever the rail can scroll further, and the arrow keys scroll
  it by three quarters of its width when it has focus. Nothing
  advances on a timer.
- **The show page,** top to bottom:
  - a hero band 24:5 wide, with a parallax image and a scrim;
  - a masthead with the poster hanging into the hero, and beside it the
    eyebrow, the italic serif title, the action row (Play, Download,
    and the list editor when a tracker is connected), the audio and
    quality choices and a meta row;
  - a body in one column: the synopsis, then the episodes panel;
  - a "Similar titles" rail of poster cards. That rail is currently
    seeded by a Kitsu title search, so franchise neighbours often
    appear in it.
- **Poster cards** (`PosterCard`): a 5:7 poster whose 2 px accent rule
  draws in on hover or focus, then the title (15 px) and a 12 px meta
  line in tabular figures: the number in full bone white and its word
  ("EPS") faint, at 45 %. It shows the episode count, or else the
  year. The card lifts on hover; its focus ring is
  bone white.
- **The Continue Watching card** is wider: a 16:9 frame with the
  episode's still, an oversized mono "EP N" over a bottom scrim, and
  below it the show title and the episode title. On hover it lifts and
  takes an accent glow.
- **The player** has a bottom-end pill for "Skip Opening"/"Skip
  Ending": dark, translucent, with an accent border that fills with the
  accent on hover. It shows for the first five seconds of the opening
  or ending interval the skip data names, and enters with the app's
  settle transition. The optional custom controls (a setting) hide
  after 2.5 s of stillness in fullscreen; the pill is not part of them.
  Below the player sit the now-playing header, with previous and next
  episode buttons, the episode strip, and a "More like this" rail.

Every new surface below is built from these parts: the same eyebrow,
card anatomy, rail behaviour, accent rules, type and motion. None of
them introduces a new widget style.

### The watch-order strip (show page)

- **Placement.** The strip comes right after the masthead, above the
  synopsis, under its own eyebrow ("Watch order"), as what comes
  before this entry, its position ("05"; a total, "of 05", only for a
  run with no fork, join or cycle) and what comes after (the
  maintainer chose this arrangement over a rail of the whole run;
  "The strip, second round" below has its layout). The
  masthead's eyebrow gains the position in its value
  slot ("TV · Finished · 05 of 09"), only for a run that is one line:
  a bare "05" there reads as nothing in particular. The reason
  for this spot: "where am I, and did I miss something" is answered
  before choosing an episode, not after scrolling past them. The strip
  is one row of entries, so it costs little height. Entries
  already in the strip are removed from the "Similar titles" rail
  below, so no poster appears twice on the page. Whether the strip
  should sit lower, just above "Similar titles", was decided: under
  the masthead ("Decisions recorded", 1).
- **At a glance.** Each card follows `PosterCard`'s anatomy at a
  smaller width:
  - a 5:7 poster with the accent rule;
  - the position as an oversized mono numeral ("03"), as the Continue
    card shows "EP N";
  - the title;
  - one meta line in `PosterCard`'s meta style: format, year, and
    episode count or progress ("TV · 2023 · 13", or "EP 9 of 13").
- **The current entry** is marked rather than linked: its accent rule
  stays drawn at rest (other cards draw theirs on hover) and it carries
  a "You are here" key in the eyebrow voice, the same words the
  franchise page uses. A watched entry shows a check in
  the meta line, never a dimmed poster: dimming would read as
  "unavailable". A line for the entry the viewer is part-way
  through, when it is another entry of the run, links to that entry's
  page; it never plays. Its form is chosen in the second mockup round
  ("The strip, second round", below).
- **On hover or focus.** The card lifts on hover like any poster card,
  draws its accent rule on hover or focus, and shows the full title.
  Long titles are clamped to two lines on the card, so a name like
  "JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 3" or "Gintama.:
  Silver Soul Arc – Second Half War" never breaks the row height. The
  full title is always the card's accessible name and tooltip. Whether
  cards should drop the part of the name the run shares and show only
  the distinguishing part ("Stone Ocean Part 3") was decided: yes
  ("Decisions recorded", 2).
- **Readability.** Titles and meta keep `PosterCard`'s sizes and tones
  so the strip sits with the rails below it. Progress is the one new
  piece of text on these cards; it follows the poster card's meta (the
  figure bright, the word faint), and the mockup review checks that "EP
  9 of 13" reads at that size. The accent is used only for the
  rule and the marker, never as a text colour on dark.
- **Keyboard and motion.** Each entry is a link with the poster
  card's focus ring, and Enter opens it, in reading order: before,
  then after. The line for the entry being watched and the franchise
  link are separate tab stops. Under reduced motion there is no lift and no
  entrance; the tokens already do this.
- **When data is missing.**
  - No relations, or Kitsu unreachable: no strip, no position in the
    eyebrow, and no empty state. The page looks as it does today.
  - An entry the provider lacks: shown like any other (decided in
    option b).
  - A forked run (link order only): the "next" position holds the
    branches side by side, each with its year and format, under a "2
    ways to continue" key, and they are unnumbered.
  - A join or a cycle (link order only): a join fills the "before"
    position the same way; an entry in a cycle shows its linked
    neighbours like any other, without a position number.
  - An unaired entry: a card with its date in the meta line ("Coming
    January 2027") and no episode count.

### The "Up next" card (player and Continue Watching)

- **Placement in the player.** On a finale, the card takes the place
  of the "Skip Ending" pill, in the same bottom-end corner, with the
  same dark translucent surface and accent border. It grows from a
  pill into a small card: the sequel's poster, an "Up next" eyebrow,
  the title clamped to two lines, and a format and year line. The
  reason for this spot: it is where the eye already goes at the
  credits, and it does not cover the picture's centre. Unlike the
  pill, which shows for five seconds, the card stays through the
  credits and after the video ends, until it is used or dismissed.
  With no skip data, it appears in the final seconds instead. Showing
  the sequel in the now-playing header's "next" button (below the
  player) on a finale was decided: no, the button stays within the
  entry ("Decisions recorded", 3).
- **On hover or focus.** On hover the card fills with the accent, as
  the skip pill does; on focus it shows the focus ring. The full title
  shows on hover and is the card's accessible name.
- **Keyboard.** The card is focusable, Enter opens the sequel's page,
  and Esc dismisses it. It never takes focus by itself, so Space and
  the arrow keys keep controlling playback. It is announced politely
  to screen readers once, when it appears. Like the skip pill, it is
  not part of the controls that hide in fullscreen, because it is a
  choice and not chrome.
- **Motion.** It enters with the settle transition the skip pill
  uses. Under reduced motion it simply appears.
- **On Continue Watching.** The card keeps its 16:9 shape, so the rail
  stays even. Its frame shows the sequel's art: its first episode's
  still where Kitsu has one, otherwise its banner or cover. The
  oversized tag reads "NEXT" in place of "EP N". The small line reads
  "After *<finished entry>*", which says why the card changed, with
  the finished entry named by the words that set it apart; the large
  line is the sequel's whole title, two lines deep. Anime titles put
  what tells a franchise's entries apart at the end, so the Continue
  card's one-line show row would cut exactly that.
- **When data is missing.**
  - A fork: the player card lists the branches, a row each. Continue
    keeps today's card.
  - An unaired sequel: the player card shows it with its date and no
    action. Continue keeps today's card.
  - No sequel: no card.

### The franchise page

- **Placement and anatomy.** The page reuses the show page's frame.
  - **Hero:** the entry the franchise is named after (the run's first
    in link order, whatever the order setting), in the hero band and the masthead's poster, so the artwork
    beside the name is that entry's. The entry being watched has its
    own line beside the Resume button.
  - **Masthead:** the run's name in the italic display serif under a
    "Franchise · 15 entries" eyebrow (every entry the page lists, the
    total the strip's "Whole franchise" link stands for), with a Resume
    button styled and
    glowing like the show page's Play. The page takes the accent of the
    entry the hero shows; Resume goes to the entry being watched.
  - **Main story:** a vertical list rather than a rail, so long titles
    have room, numbered along the path from the run's first entry
    (section 4, "Position numbers"). Each row shows the oversized mono
    numeral, a small poster, the title in the body face, a format,
    year and episode-count line, and a thin progress bar in that
    entry's own accent. A "You are here" marker sits on the current
    entry.
  - **Films, side stories and other versions:** standard `Strip` rails
    of poster cards, each under the app's eyebrow.
  - **Recaps:** a collapsed section under the same eyebrow.
- **At a glance versus on hover.** The list shows the numeral, title,
  year and progress. Hover or focus reveals the full title, the
  episode count and the link target, as on the show page.
- **Keyboard and motion.** The list is a list of links in reading
  order. There is no order switch (section 4). The page enters like the
  show page. Under reduced motion it does not
  animate.
- **When data is missing.**
  - A bounded walk that stopped early: the list ends with a quiet
    "Showing 40 of more" line.
  - A forked run: the numbered path continues as a list, and the
    branches stand side by side, unnumbered, from the fork.
  - Entries the provider lacks: listed normally.

### The watch-order setting

[decided: link order is the default; "As released" is the setting]
One setting decides the order everywhere; no surface has a toggle of
its own, and nothing is marked as "out of order". A choice made in
one place and explained once is less to think about than a switch
and a warning on every strip, card and page.

- **Where.** Settings, in the Playback section ("defaults applied to
  every stream"), directly under "Auto-play next episode", as a
  two-option segmented control like the audio and quality choices
  there.
- **Wording.** Key: "Watch order". Options: "As linked" (the default)
  and "As released". Hint, in the field-hint voice: "How a series'
  seasons, films and specials are ordered: as the anime databases link
  them, or by release date. The links generally follow the story, but
  they are maintained by each database's community and can be wrong."
- **Effect.**
  - The watch-order strip shows the previous and next entries, and
    the position number, in the chosen order. Forks appear only in
    "As linked".
  - The "Up next" card and Continue Watching offer the next entry in
    the chosen order. In "As released" there is always at most one.
  - The franchise page lists the run in the chosen order and numbers
    it the same way.
  - Which entries form the run, and which are other versions, does not
    change (section 4).
- **When it applies.** On the next render of each surface; nothing
  already shown re-orders under the viewer.

### How each phase is designed

1. **Mockups in the real app first.** The surface is built against
   fixture data behind a development flag and run in the real
   Electron app. Screenshots are then reviewed by the maintainer
   before the implementation goes further. The screenshots cover:
   - the show page of a linear run (*Thousand-Year Blood War – The
     Calamity*);
   - a long-titled run (*JoJo: Stone Ocean Part 3*);
   - a branching run (*Fate/Zero 2nd Season*);
   - a show with no relations, and a single film;
   - for the Up next card, the player at a finale and the Continue
     rail;
   - each at a common desktop size and at the smallest window the app
     supports, and with reduced motion on.
2. **Designer and critic.** The design work follows AGENTS.md §8: the
   §7 constraints are passed verbatim to the designer pass, a separate
   critic pass reviews its output, and nothing generated is accepted
   as-is.
3. **Draft pull request.** Each phase opens as a draft with a "Manual
   test required before merge" section. That section names the
   screens, the long titles, the branching and missing-data cases, and
   keyboard-only and reduced-motion runs that must be checked by hand.
4. **Consistency check.** Before merge, the new surface is compared,
   in screenshots placed side by side, with the surfaces it borrows
   from: the "Similar titles" rail and poster cards for the strip and
   the franchise page, the skip pill and the Continue rail for the
   Up next card, and the show page's masthead for the franchise page.

### Phase 0: the mockups, surface by surface

The mockups are drawn in the real app from fixture data: real Kitsu
entries, with titles, formats, dates, episode counts and images as
Kitsu served them on 2026-11-02, and made-up progress so every state
shows. The runs are *Bleach* (a straight line, from the 2004 series to
the fourth *Thousand-Year Blood War* part), *Attack on Titan*, *JoJo*
(long romanised titles), *Gintama* (a join at *The Final*), *Fate*
seen from *Fate/Zero* (a fork) and *Monogatari* (a loop of ten
entries, and *Off & Monster Season*'s undated special). They are
turned on in Diagnostics; nothing is fetched or walked at runtime, and
`frontend/src/lib/watch-order/mock/` is removed when the feature
lands. Where a choice is real, the mockup offers two options; the
recommendation follows each.

The rules below hold on every surface:

- **Locales.** Every string is in the four catalogues. Russian and
  Brazilian Portuguese run longest. The screenshots are taken in
  English and Russian at 1920 and 1280 px, in Brazilian Portuguese
  for the strip, the franchise page, the setting, Continue and the
  player, and in English at 960 px, the smallest window the app
  allows. Counts that sit beside a
  noun ("10 entries", "2 ways to continue") use plural variants; the
  rest put the number where no noun agrees with it ("Comes after these
  2"). Order words follow the setting's wording: "As linked" is "Por
  vínculo" in Portuguese, since "ligado" reads as "switched on".
- **Reduced motion.** Every transition is zero under
  `prefers-reduced-motion`: the cards use the duration tokens, and the
  Up next card enters with the skip pill's own transition, which
  checks the preference itself. Nothing moves on a timer.
- **Logical properties** only, so a right-to-left locale is a
  translation.
- **Titles.** A card clamps its title to two lines and keeps the full
  title as its tooltip and accessible name. The "short titles" option
  drops the leading words another entry of the run shares, up to a
  separator (": ", " - "): *JoJo no Kimyou na Bouken: Stone Ocean Part
  3* becomes *Stone Ocean Part 3*. A title with no shared part, such
  as *Steel Ball Run: JoJo no Kimyou na Bouken*, stays whole.

#### The watch-order strip (show page)

- **Options.** *Whole run*: the run as a rail of compact poster cards,
  scrolled so the entry being viewed sits three quarters of the way
  along with what came before it in sight. *Before and after*: three
  columns, the entries before, this entry's oversized numeral with "of
  09", and the entries after, as small landscape tiles.
  **Decided: before and after** (the maintainer's answer; its second
  round is "The strip, second round" below). It answers the question
  the strip exists for ("what came before, what comes next") without scrolling,
  and a fork or a join reads as two tiles under one key ("2 ways to
  continue", "Comes after these 2"). The whole run belongs on the
  franchise page, one click away.
- **Placement.** Under the masthead (decided), or just above
  "Similar titles". Lower down, the strip competes with "Similar
  titles" and is found only after the episodes; under the masthead it
  costs one row and answers "did I miss something" before an episode
  is chosen.
- **Structure in the rail.** A flat row would draw *Fate/stay night*
  (2006) → the UBW prologue → UBW as one sequence, though they are two
  ways on from *Fate/Zero 2nd Season*. So each branch opens with a key
  ("Way 1 of 2") and a hairline in the gap before it; a loop opens
  with "Part of a loop of 10 entries"; an entry that only leads into a
  join says so ("Leads into 10", by the number of the entry it leads
  into, since a title would not fit the key line). Every member of a
  loop carries a rule over its poster that runs across the gaps, so
  the loop reads as one stretch even where "You are here" takes its
  key. Positions count
  along the path to the run's first entry: *The Semi-Final*, which no
  prequel reaches, has no number rather than a second "01". The key
  line is reserved on every card, so titles share one baseline.
- **Missing data.** No run: no strip, no position in the masthead.
  First or last entry: "Start of the run", "End of the run" in the
  empty column. Unknown date: "Date not announced" in place of the
  year and count. An entry the provider lacks looks like any other.
- **Keyboard.** The rail is the existing `Strip` (focusable, arrow
  keys page it); every card and tile is a link with the poster card's
  bone focus ring; the entry being viewed is not a link. The line for
  the entry being watched and "Whole franchise" are separate tab
  stops.

#### The "Up next" card (player)

- **Options.** *Poster*: the next entry's 5:7 poster beside its title
  and meta, in the skip pill's surface and corner. *Still*: the next
  entry's banner above its title, wider. **Recommended: poster.** It
  covers less of the end credits (at 1280 px the still variant covers
  more than half of the picture's height), and banners crop
  unpredictably at that size.
- **States.** One aired next entry: a link to its page. Several (a
  fork): "Ways to continue" over one row per branch. One not yet
  aired: "Next, when it airs", its date or "Date not announced", and
  no link. None: no card. The mockup shows the card on an entry's
  last episode, from the start of playback, so a screenshot can catch
  it over the credits.
- **Keyboard and screen readers.** The card never takes focus; Esc
  dismisses it while it has focus; the dismiss button is its own tab
  stop. The announcement is written into a polite live region after
  the card mounts, since a region that already holds its text when it
  appears is not read by every screen reader.

#### Continue Watching's hand-off

- **Options.** The tag reads *NEXT*, or *EP 1* like any other card.
  **Recommended: NEXT.** "EP 1" reads as progress on a show not yet
  started, and the tag is the only thing that says the card changed
  before the text is read.
- **Placement.** The hand-off takes the finished entry's place in the
  rail; that entry's own card is not shown beside it. Once the next
  entry has a history row of its own, the hand-off gives way to it.
- **Truncation.** See "On Continue Watching" above: the sequel's
  whole title gets the two-line row.

#### The franchise page

- **Options.** *List*: one row per entry, the oversized numeral, a
  small poster, the title, the meta line and a progress bar in the
  entry's accent (only where something was watched). *Shelf*: the
  rail's cards in a grid. **Recommended: list.** Long titles have the
  width they need, and the fork and the loop can be laid out as what
  they are.
- **Structure.** Straight stretches are numbered from the run's first
  entry; a stretch after a loop is not, since the loop gives it no
  single place, and its key says so. A fork is a group
  under "2 ways to continue", its branches side by side as unnumbered
  columns, each under "Way n of N". A loop is a group under "Part of a
  loop of 10 entries" with one sentence on why it has no single order;
  its entries are listed along their links, from where the run enters
  the loop. In release order every one of these is a single numbered
  line.
- **Masthead.** The franchise is named after the run's first entry,
  without Kitsu's format suffix ("(TV)", "(Movie)", …) unless another
  entry of the franchise would then read the same, ignoring case; the
  suffix then stays, since it is what tells the two apart. *JoJo no
  Kimyou na Bouken (TV)* keeps it, because the franchise's 1993 OVA is
  titled *JoJo no Kimyou na Bouken*. The hero and the masthead's poster
  show the entry the name comes from (the first round showed the entry
  being watched, which put *Steel Ball Run*'s artwork beside the 2012
  series' name); the entry being watched has its own line beside the
  Resume button. A note under the button says which order the list
  follows, with a link to Settings.
- **Missing data and keyboard** as on the show page; the recaps stay
  folded behind a "Show recaps" button that reports its state.

#### The setting

- **Options.** Two buttons, like the audio and quality choices; or two
  options each with a one-line description. **Recommended: buttons.**
  They keep the Playback section's two-column field rhythm; the
  described options bring a bordered-box control no other setting
  uses. The field's hint carries the explanation either way.

### Decisions recorded (first review round)

The maintainer answered the eight questions the first mockups asked:

1. **Strip.** Before and after, under the title (the masthead). The whole-run rail
   was liked, but forks and joins break it up. The strip needed a
   second round, below.
2. **Short titles** on cards: yes, with the full title as tooltip and
   accessible name.
3. **The player's "next episode" button** stays within the entry; only
   the Up next card crosses to another entry.
4. **Accents.** Each entry keeps its own accent.
5. **Up next** shows the poster; Continue's hand-off carries the
   **NEXT** tag. The hand-off card opens the next entry's show page,
   not its first episode, as the mockup already does: the viewer
   arrives where the strip, the synopsis and the episode list say what
   they are about to start.
6. **Franchise page** as a list.
7. **The setting** uses buttons. Its hint adds that the links
   generally follow the story but are maintained by each database's
   community and can be wrong (section 5, "The watch-order setting").
8. **The franchise's name** drops Kitsu's format suffix, guarded
   against clashes: the suffix stays when another entry of the same
   franchise would read the same without it (Phase 0, "The franchise
   page"). In the fixtures this keeps *JoJo no Kimyou na Bouken (TV)*
   whole; *Zoku Owarimonogatari (TV)* and the film *Zoku
   Owarimonogatari* are the other real clash, though neither names a
   franchise.

### The strip, second round

The second round revises the before-and-after strip only. It is a
mockup like the first, behind the same Diagnostics switch, and each
variant below is one of the switch's options.

- **Balance.** The three columns are mirrored about the centre: the
  entries before read towards it (poster on the side nearest the
  centre, text aligned to it), the entries after read away from it,
  and the outer columns take equal shares of the width, so the
  position numeral is the strip's centre whichever side is longer. The
  grid has three rows (the keys, the join or fork line, the entries),
  so the three keys share one baseline whatever sits under them, and
  the centre's two hairlines run through all three rows. An empty
  side keeps its column, drops its key and says why it is empty
  ("Start of the run", "End of the run"), so the numeral does not move
  at either end of a run. The centre column stays at every window
  width; the first round hid it below 1100 px.
- **The band.** The strip sits on one faint band: the page's own ink,
  one step up (`--ink-050`), at 72 % opacity, with the card corner
  radius. No border, blur, shadow or gradient. Its footer, under a
  hairline, holds the line for the entry being watched at the start
  and the way to the franchise page at the end.
- **Wording of the two columns.** Four options:
  - *Relation*: Kitsu's role for the link. "Prequel" and "Sequel",
    plural at a join or a fork ("Sequels", over "2 ways to continue");
    a side story's strip names its "Parent story" (two on *Fate/Zero:
    Onegai! Einzbern Soudanshitsu*, which Kitsu relates to both
    *Fate/Zero* seasons), with no "Comes after these 2" line, since two
    parents are not a join. A role names a link, so in "As released"
    order, where the neighbours are neighbours by date, this option
    says "Released before" and "Released after": calling the 2006
    *Fate/stay night* the "Prequel" of *Fate/Zero* would be false.
  - *Previous / Next*.
  - *Watch before / Watch after*.
  - *Before / After*, the first round's words.

  **Recommended: Relation.** It is the only wording that is right on a
  side story: "Previous: *Gintama°*" on *Gintama°: Aizome Kaori-hen*
  puts the two in one sequence, and "Watch before: *Gintama°*" is an
  instruction the data does not give; "Parent story" says what Kitsu
  says. Kitsu names both branches of *Fate/Zero 2nd
  Season*'s fork `sequel`, so roles do not tell the branches apart;
  the "2 ways to continue" line under the key does.
- **"Whole franchise".** Two variants, both the same link, at the end
  of the band's footer:
  - *Button*: the show page's outline button at its full size (the
    size of Download), with the number of entries the franchise page
    lists, recaps included, and the app's arrow ("Whole franchise |
    15 entries →" on JoJo).
  - *Poster stack*: three posters fanned (the run's first entry, a
    film or other version, a side story), "See the whole franchise",
    and what the page holds ("9 in the main story · 4 films and
    versions · 1 side story · 1 recap", the same 15 the button
    counts). The posters spread slightly on hover or
    focus. No border, so it does not read as a twin of the watching
    card.

  **Recommended: button** (the maintainer chose the stack; see the
  second round's decisions). It is the app's own control, at the size of
  the page's other buttons, and the count tells the viewer there is
  more than the strip shows. The stack shows *what* there is, which
  the button cannot, but it is a widget no other page has; it is the
  option to take if the button reads as too quiet in use.
- **The entry being watched.** The first round's "You're on *Steel
  Ball Run* · EP 1 · Resume" (on *Golden Wind*) read as a second play
  button. Two
  variants, both linking to that entry's show page, neither playing:
  - *Card*: a small link with the entry's poster, "You're watching",
    its title and episode, and an arrow. Its visible text is its
    accessible name.
  - *Sentence*: "You're watching *Steel Ball Run* · EP 1." followed by a
    text link, "Go to its page".

  Either way the line shows only when the entry being watched is not
  already one of the strip's tiles: on *Stone Ocean Part 3* the sequel
  tile is *Steel Ball Run*, with its progress, so the line would only
  repeat it.

  When the footer has no room for both on one row, the franchise link
  takes the upper row.

  **Recommended: card.** The poster says "this is a show" before the
  words are read, which is the confusion the round set out to fix.

### Decisions recorded (second review round)

The maintainer answered the second round's six questions:

1. **Column wording: Relation.** In Brazilian Portuguese "Prequela"
   goes: it reads as translated or European, and is not what a
   Brazilian viewer says. A prequel is "Anterior" (plural
   "Anteriores") and a sequel "Continuação"; a side story's parent is
   "História principal" ("Histórias principais"), beside the existing
   "História paralela". "Lançado antes / depois" stays for release
   order. Russian and Latin American Spanish keep their reviewed words.
   In Portuguese the relation wording and the "Previous / Next" option
   now share "Anterior" for the column before, and a parent story's
   key reads as the franchise page's "História principal" section; a
   side story's parents are main-story entries, so the two agree.
2. **"Whole franchise": the poster stack**, not the button. Its third
   round is below: the fan on hover, a highlight on the whole link,
   focus, reduced motion, and one, two or no posters.
3. **The entry being watched: the card.** When it and the stack share
   the footer, both are centred at matching widths (third round,
   below).
4. **A parent story's strip does not list its side stories.** It would
   repeat "Similar titles"; what that row does with a franchise's side
   stories is a separate discussion.
5. **No "of N" at forks, joins and loops** stays. In "As released"
   order every run is one line, and the strip says "N of total" there:
   *Nisemonogatari* is "02 of 16", and *Off & Monster Season Shin
   Episode*, which has no date and sorts last, "16 of 16".
6. **The franchise page's masthead** takes its artwork from the entry
   the name comes from, the run's first in link order (on JoJo, the 2012 series, not
   *Steel Ball Run*, which the viewer is watching); the entry being
   watched keeps its own line beside Resume. The name keeps Kitsu's
   "(TV)" where dropping it would clash with another entry's title,
   as decided in the first round. The maintainer first asked for the
   year in its place ("JoJo no Kimyou na Bouken (2012)") and then
   reversed it: the year would read as naming only the first season,
   not the franchise. The franchise-name function is unchanged.

### The poster stack and the footer, third round

The third round revises the strip's footer only.

- **The fan.** The posters stand on one arc, as if pinned at a point
  below the stack: each is drawn by rotating it about the arc's centre
  (`stack-fan.ts`, a pure function with fast-check properties), so the
  transition between the two poses moves each card along the arc,
  outwards, with the app's elastic card easing. At rest three cards
  are 6.5° apart; on hover or focus 9° apart, the second round's
  hovered fan, with a deeper shadow. The way back to rest uses the
  soft easing, so the cards do not overshoot inwards. Two cards split
  half a step to each side of the middle; one stands upright and
  lifts.
- **No poster.** A franchise none of whose entries has a poster shows
  three blank frames in the same fan, the front one carrying the
  number of entries the franchise page lists, upright, in the strip's
  numeral face. The posters the stack fans are only entries that have
  one, and a poster that fails to load is drawn as a blank frame, so
  a missing image never leaves an empty card among real ones.
- **The whole link is the target.** The link's padding is the area a
  highlight marks; three options:
  - *Surface*: a faint plate one step up the page's ink appears under
    the whole link, the hover the strip's own entry tiles use;
  - *Underline*: the label's underline, faint at rest, turns full;
  - *Border*: a hairline ring appears round the whole link.

  Keyboard focus shows the same state as hover (the highlight, the
  fan open, the arrow forward) under the app's focus ring.
- **Reduced motion.** The stack is drawn fanned out and does not move;
  the highlight still appears on hover and focus, without a
  transition. The preference is read through `$lib/motion`.
- **The footer.** Three arrangements:
  - *Centred*: the card and the stack side by side in the middle of
    the band at one width (24rem at most); one above the other, at
    the same width, where the band is under 50rem (a 1100 px window
    and below);
  - *On the centre line*: on the strip's own columns, the card under
    the entries before, towards the centre, and the stack under the
    entries after, at one width (22rem at most); centred, one above
    the other, where the band is under 56rem (about a 1210 px
    window);
  - *Opposite ends*: the second round's arrangement, for comparison.

  One above the other, the card comes first, in reading order; the
  second round put the franchise link on the upper row. In the first
  two the card drops its resting border and takes the surface plate
  on hover and focus (the stack's own, with the surface highlight), so the two read as one matched
  pair rather than a framed card beside unframed text; its episode
  number keeps its place and the title gives way when the width is
  short. Without the card (on *Stone Ocean Part 3*, where *Steel Ball
  Run* is already a tile) the stack sits alone in the middle, at its
  own width.

### Open design questions

Each with a recommendation; the maintainer decides.

1. **The stack's highlight.** Surface (recommended), underline or
   border. Surface is the only one that marks the whole link as one
   target, and it is the hover the strip's own tiles already use.
   Underline marks only the label; border reads as a second framed
   card beside the watching card, and under the focus ring as two
   rings. The design critic agreed.
2. **The footer.** Centred (recommended), on the centre line, or
   opposite ends. Centred is what was asked for and holds at every
   width; the centre-line version ties the pair to the strip's
   columns but only fits side by side from about a 1210 px window,
   and below that stacks them, centred. The design critic agreed.
3. **The watching card's border.** In the centred footers the card
   loses its resting border so it pairs with the stack (both bare at
   rest, the same plate on hover and focus). Recommended: keep that;
   the alternative is a border on both, which makes the stack read as
   a second card. This one came from the design critic's review.

## 6. Recommendation and plan for 0.19.0

**Do not merge entries.** Keep every Kitsu entry its own show; build
the connections around it. This matches what the research found:
platforms that merge own their catalogue, and the databases this app
reads store edges, not franchises.

**Treat order simply.** Follow the databases' links by default, offer
release order as one app-wide setting, keep other versions out of the
run, and never call either order "the story order": the data does not
support that claim (section 2).

**Phases**, each shippable on its own, and each going through the
design gate in section 5 (mockups in the real app reviewed before
implementation, designer and critic, a draft pull request with a
manual test section, a consistency check) before it merges:

1. **Watch order strip** (option a), with the `chain` walk and the
   "You're watching …" line, both of which need the whole run, not just the
   neighbours. Read-only, no provider requests, no change to Continue
   or trackers. On its own it answers "what comes before and after"
   and most of the JoJo request.
2. **Up next at a finale** (option b): the player end card first, then
   Continue moving to the sequel and the finished entry's card giving
   way. The card waits for a click, and no provider is asked about the
   sequel.
3. **Franchise page** (option c), in 0.19.0, reusing phase 1's walk
   and adding the side stories, films, recaps and other versions
   around it; the strip gains its trailing link to the page here.
4. **One card per franchise** (option d): not planned. Revisit after
   phase 2 ships, if hunting for the current part is still a
   complaint.

**Decided by the maintainer.**

- **No provider check at a finale.** "Up next" names the next entry
  the relations give, carried or not; the sequel's own show page
  reports when the provider lacks it, and its strip shows what
  follows.
- **No auto-play across entries.** The finale shows the "Up next"
  card and waits; auto-play stays within one entry.
- **The franchise page (phase 3) is part of 0.19.0.**
- **Link order is the default**, with "As released" as an app-wide
  preference in Settings (section 5). The maintainer disagrees with
  some of the links Kitsu's editors chose, but does not want the app
  to keep its own list of differences from the source for now: the
  app follows the source, and the one setting is the way out for a
  viewer who prefers release order.

## Live requests made for this proposal

All on 2026-11-01, sent one at a time with a pause of two seconds or
more between them, 38 in total:

- Kitsu REST, 34. For the first survey: the franchise list (one page
  of two), four installments routes (all HTTP 500), one slug lookup,
  one MyAnimeList-id mapping lookup, nine `media-relationships` reads
  (818, 3919, 43078, 46598, 8063, 7730, 41982, 46903, 5649) and the
  role-filter test on 818. For the link-order reading in section 2:
  one batched mapping lookup of six MyAnimeList ids, one test of a
  batched read (`/anime?filter[id]=…&include=mediaRelationships.destination`),
  thirteen batched reads of up to twenty entries each that followed
  the prequel/sequel links of the ten franchises to closure, one
  search for *Kamitsubaki*, and one batched read refused with HTTP 403
  (a client without a browser-like user agent), repeated with `curl`.
- Kitsu GraphQL, 1: a franchise lookup the schema does not have.
- AniList GraphQL, 3: two relations queries (nine and eight entries)
  and one schema introspection of `MediaRelation`.

For the second mockup round, on 2026-11-02, three more Kitsu REST
`media-relationships` reads, one at a time with a pause of three
seconds: 6928, 11759 and 12064, the side stories whose parent stories
the mockup's fixtures record.

The Fate and Ranma relation samples are the repository's recorded
fixtures under `tests/fixtures/tracker-map/wire/`, and the unaired
sequel of Kitsu 48677 one under `tests/fixtures/hianime/appended/`;
none of them are live reads.
