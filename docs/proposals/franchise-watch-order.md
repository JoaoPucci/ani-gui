# Proposal: watch order — what comes before, what comes next, and the franchise around a show

**Status**: proposed as the main feature of the 0.19.0 release. The
product questions are decided (section 6); the design questions are
open (section 5). Built so far, and documented where the app's
documentation describes it: the backend's franchise read, `GET
/api/kitsu/franchise/:kitsu_id`, in
[`title-resolution.md`, "Kitsu's relations, walked into a franchise"](../title-resolution.md#kitsus-relations-walked-into-a-franchise);
the watch-order strip on the show page, the order the surfaces show a
run in, the "Watch order" setting, the player's Up next card at a
finale and Continue Watching's hand-off, in
[`architecture.md`, "Watch order"](../architecture.md#watch-order). The sections that described them here (the research of
sections 1 and 2, the order rules of section 4, options (a) and (b)
and their parts of section 5) now point there. Not built yet: the
franchise page, which exists only as a design mockup drawn from
fixture data, behind a Diagnostics switch.

## The problem in plain terms

Anime is published in pieces. A story that a viewer thinks of as one
show — *JoJo's Bizarre Adventure*, *Gintama*, *Bleach: Thousand-Year
Blood War* — reaches the catalogues as many separate entries: seasons,
"parts" and "cours", movies, OVAs, specials and recaps, each with its
own title, poster and episode list. `ani-gui` takes its catalogue from
Kitsu, which models every one of those pieces as its own anime, so
the app does too.

Before the watch-order surfaces, the app knew nothing about how those
entries connect. Three things followed from that; the strip
([`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip)) answers the second, the player's Up next card
([`architecture.md`, "The player's Up next card"](../architecture.md#the-players-up-next-card)) and Continue Watching's hand-off ([`architecture.md`, "Continue Watching's hand-off"](../architecture.md#continue-watchings-hand-off)) the first; the
franchise page, still to build, answers the third:

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

Moved. What the research found, and what the app takes from it (do
not merge entries; shelve films and side stories around the run;
admit ambiguity rather than guess), is the opening of
[`architecture.md`, "Watch order"](../architecture.md#watch-order).

## 2. What data the app has

The franchise read is built. What Kitsu's relations say and do not
say, why the walk reads Kitsu's links and neither Kitsu's franchise
model nor AniList's relations, and the listing it reads are documented
in [`title-resolution.md`, "Kitsu's relations, walked into a franchise"](../title-resolution.md#kitsus-relations-walked-into-a-franchise).
What the streaming provider adds matters to the surfaces still to
come:

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

The library survey this plan was made from showed franchises to be
the normal case, and long: about 70 % of the author's 294 MyAnimeList
entries sit in franchises of two or more, most with films or specials
beside the run of series. Where "what's next" would have mattered:
*Bleach: Thousand-Year Blood War* part 1 played to its finale, its
Continue card offering episode 13 again, and part 3 never played
between parts 2 and 4; *JoJo*, the franchise behind the original
request, with ten list entries and no Part 5.

## 4. Design options

All four options share one backend piece, `GET
/api/kitsu/franchise/:kitsu_id`, which is built and documented in
[`title-resolution.md`, "Kitsu's relations, walked into a franchise"](../title-resolution.md#kitsus-relations-walked-into-a-franchise):
how the walk builds a run, the main story every page of a franchise
shares, a page's own chain outside it, and what sits around the run,
its other versions among them. The order the surfaces show a run in
is built too, below.

### Order and position numbers

Built: how a run is ordered as linked and as released, how positions
are counted, and what the setting changes across ten franchises are in
[`architecture.md`, "Order and position numbers"](../architecture.md#order-and-position-numbers).

### (a) A "Watch order" strip on the show page

Built: [`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip).
The strip's way to the franchise page arrives with option (c).

### (b) "Up next: season 2" at a finale, and Continue moving on

Built: [`architecture.md`, "The player's Up next card"](../architecture.md#the-players-up-next-card) and [`architecture.md`, "Continue Watching's hand-off"](../architecture.md#continue-watchings-hand-off).

### (c) A franchise page

**How it looks.** `/anime/<id>/franchise`, reached from the strip's
trailing link and from search results that belong to a franchise. A
header named after the run's first entry, with your position: "05 ·
*Golden Wind* · EP 12" (with a total only for a run with no fork, join
or cycle) and a Resume button; its hero and accent are set in the
design section. Below it, sections:

1. **Main story** — the run (the walk's main story), in the order the
   setting chooses, numbered as
   [`architecture.md`, "Order and position numbers"](../architecture.md#order-and-position-numbers) describes, with branches
   unnumbered beside the numbered path, progress bars per entry and a
   "you are here" marker.
2. **Other versions** — the walk's other versions: from every JoJo page, both
   OVAs. Movies are not listed here but under Films: the 2010 UBW film
   and the three *Heaven's Feel* films, *Gintama*'s retelling films.
3. From the franchise read's sections: **Films** (every movie around the run, whatever its
   role: a version, a recap or a side story), **Side stories &
   specials** (everything else: side stories, parent stories,
   spin-offs, `other`) and **Recaps** (`summary`, a TV series aside,
   which is an other version). Links of the `character` and `adaptation`
   roles are not shown: they tie unrelated series together (*Gintama*
   to *Saiki Kusuo*).

There is no order switch on the page; the app-wide setting applies.

**Data.** The franchise read (one answer for the page's anchor), plus
progress from the local history (the row each entry's page resumes
from) and, where a
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
run. A history row counts toward the entry whose page would resume
from it: the entry it recorded while Kitsu still has it, else the one
its stored mapping names, else, for a row whose recorded entry Kitsu
deleted, the one Home's title match under the row's own cour names.
A row with none of these contributes no progress.

**Trackers.** Read-only.

**Cost.** Medium: the franchise read already serves the sections; what
is left is the page and the design work AGENTS.md §7 asks of a new
surface. It reuses the strip's ordering.

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

This section describes the app as it was before the watch-order
surfaces (the strip now sits under the show page's masthead, and
"Similar titles" leaves out what it shows), as read from
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

Built: [`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip).

### The "Up next" card (player and Continue Watching)

Built: [`architecture.md`, "The player's Up next card"](../architecture.md#the-players-up-next-card) and [`architecture.md`, "Continue Watching's hand-off"](../architecture.md#continue-watchings-hand-off).

### The franchise page

- **Placement and anatomy.** The page reuses the show page's frame.
  - **Hero:** the entry the franchise is named after (the run's first
    in link order, whatever the order setting), in the hero band and the masthead's poster, so the artwork
    beside the name is that entry's. The entry being watched has its
    own line beside the Resume button.
  - **Masthead:** the run's name in the italic display serif under a
    "Franchise · 15 titles" eyebrow (every entry the page lists, the
    total the strip's "Whole franchise" link stands for), with a Resume
    button styled and
    glowing like the show page's Play. The page takes the accent of the
    entry the hero shows; Resume goes to the entry being watched.
  - **Main story:** a vertical list rather than a rail, so long titles
    have room, numbered along the path from the run's first entry
    ([`architecture.md`, "Order and position numbers"](../architecture.md#order-and-position-numbers)). Each row shows the oversized mono
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
  order. There is no order switch (one setting decides everywhere). The page enters like the
  show page. Under reduced motion it does not
  animate.
- **When data is missing.**
  - A bounded walk that stopped early: the list ends with a quiet
    "Showing 40 of more" line.
  - A forked run: the numbered path continues as a list, and the
    branches stand side by side, unnumbered, from the fork.
  - Entries the provider lacks: listed normally.

### The watch-order setting

Built: [`architecture.md`, "The order setting"](../architecture.md#the-order-setting).
For the surface still to come, the setting's effect: the franchise
page lists the run in the chosen order and numbers it the same way.

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

The rules every surface follows (whole titles, plain words, a
franchise read in part, motion and direction) are in
[`architecture.md`, "Watch order"](../architecture.md#watch-order).

#### The watch-order strip (show page)

Built: [`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip).

#### The "Up next" card (player)

Built: [`architecture.md`, "The player's Up next card"](../architecture.md#the-players-up-next-card).

#### Continue Watching's hand-off

Built: [`architecture.md`, "Continue Watching's hand-off"](../architecture.md#continue-watchings-hand-off).

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
  columns, each under "Way n of N". A loop is a group under "10
  linked titles, order unclear" with one sentence on why it has no
  clear order; its entries are listed along their links. In release
  order every one of these is a single numbered line.
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
- **Missing data and keyboard** as on the show page ([`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip)); the recaps stay
  folded behind a "Show recaps" button that reports its state.

#### The setting

Built: [`architecture.md`, "The order setting"](../architecture.md#the-order-setting).

### Decisions recorded (first review round)

The maintainer answered the eight questions the first mockups asked.
The answers on the strip, short titles, the player's next-episode
button, the Up next card and Continue's hand-off, and the setting (1,
2, 3, 5 and 7) are built and documented in [`architecture.md`, "Watch order"](../architecture.md#watch-order);
the rest:

4. **Accents.** Each entry keeps its own accent.
6. **Franchise page** as a list.
8. **The franchise's name** drops Kitsu's format suffix, guarded
   against clashes: the suffix stays when another entry of the same
   franchise would read the same without it (Phase 0, "The franchise
   page"). In the fixtures this keeps *JoJo no Kimyou na Bouken (TV)*
   whole; *Zoku Owarimonogatari (TV)* and the film *Zoku
   Owarimonogatari* are the other real clash, though neither names a
   franchise.

### The strip, second round

The second round revised the before-and-after strip. Its balance, band,
column wording and the card for the entry being watched are built and
documented in [`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip).
The way to the franchise page, at the end of the band's footer, is
not built yet:

- **"Whole franchise".** Two variants, both the same link, at the end
  of the band's footer:
  - *Button*: the show page's outline button at its full size (the
    size of Download), with the number of entries the franchise page
    lists, recaps included, and the app's arrow ("Whole franchise |
    15 titles →" on JoJo).
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
  When the footer has no room for both the card for the entry being
  watched and the franchise link on one row, the franchise link takes
  the upper row.

### Decisions recorded (second review round)

The maintainer answered the second round's six questions. The
answers on the strip itself (1, 4 and 5, and the card in 3) are built
and documented in [`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip);
the rest:

2. **"Whole franchise": the poster stack**, not the button. Its third
   round is below: the fan on hover, a highlight on the whole link,
   focus, reduced motion, and one, two or no posters.
3. **The entry being watched and the stack**, when they share the
   footer, are both centred at matching widths (third round, below).
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

Each with a recommendation; the maintainer decides. The footer's
"You're watching" card is built, centred at 24rem at most and with no
border at rest; what is open is the franchise link beside it.

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
support that claim (see [`title-resolution.md`, "Kitsu's relations, walked into a franchise"](../title-resolution.md#kitsus-relations-walked-into-a-franchise)).

**Phases**, each shippable on its own, and each going through the
design gate in section 5 (mockups in the real app reviewed before
implementation, designer and critic, a draft pull request with a
manual test section, a consistency check) before it merges:

1. **Watch order strip** (option a): built, with the "You're watching"
   card and the order setting
   ([`architecture.md`, "The show page's strip"](../architecture.md#the-show-pages-strip)).
2. **Up next at a finale** (option b): built, the player's card and
   Continue Watching's hand-off ([`architecture.md`, "Continue Watching's hand-off"](../architecture.md#continue-watchings-hand-off)).
3. **Franchise page** (option c), in 0.19.0, reusing the franchise read
   and adding the side stories, films, recaps and other versions
   around it; the strip gains its trailing link to the page here.
4. **One card per franchise** (option d): not planned. Revisit after
   phase 2 ships, if hunting for the current part is still a
   complaint.

**Decided by the maintainer.**

- **No provider check at a finale** and **no auto-play across
  entries**: built, with their reasons, in [`architecture.md`, "The player's Up next card"](../architecture.md#the-players-up-next-card). Continue
  Watching's hand-off asks the provider nothing either.
- **The franchise page (phase 3) is part of 0.19.0.**
- **Link order is the default**, with "As released" as an app-wide
  preference in Settings: built, with its reasons, in
  [`architecture.md`, "The order setting"](../architecture.md#the-order-setting).

## Live requests made for this proposal

All on 2026-11-01, sent one at a time with a pause of two seconds or
more between them, 38 in total:

- Kitsu REST, 34. For the first survey: the franchise list (one page
  of two), four installments routes (all HTTP 500), one slug lookup,
  one MyAnimeList-id mapping lookup, nine `media-relationships` reads
  (818, 3919, 43078, 46598, 8063, 7730, 41982, 46903, 5649), whose
  findings `title-resolution.md` summarises, and the role-filter test
  on 818 (`filter[role]=sequel,prequel` answered one row of fifteen;
  the walk reads unfiltered, so one listing serves it and what sits
  around the run). For the link-order reading (now in
  `title-resolution.md`):
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

The unaired sequel of Kitsu 48677 (starting 2027-01-10), the design's
example of an unaired title ("Coming January 2027"), is a
repository fixture under `tests/fixtures/hianime/appended/`, not a
live read.
