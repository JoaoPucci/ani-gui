# Privacy Policy

_Last updated: 2026-11-03_

This document explains how ani-gui handles your data. It applies to
the open-source ani-gui desktop application maintained at
<https://github.com/JoaoPucci/ani-gui>.

The plain version: **ani-gui runs entirely on your machine. We do
not operate a server, and the app collects no analytics or
telemetry. What leaves your computer is the requests the app makes
to public services to do its job: the anime catalogues and the hosts
that serve their images, the streaming site and the video hosts it
names, Aniskip, GitHub's list of the app's releases, and, if you
connect one, your anime tracker. Each is described below.**

If you have not connected an account (AniList or MyAnimeList), none
of these requests carries an account or any other identifier of
yours. With one connected, two kinds do: the requests to your tracker
carry your sign-in, and the request for your tracker avatar goes to an
address that names your account (below). Like any request over the
internet, each one shows the service your IP address.

## What ani-gui stores locally

ani-gui keeps the following on your computer only:

- **Settings** — your playback preferences, locale, external-player
  configuration, etc. Stored as plain TOML under your OS's
  configuration directory (`$XDG_CONFIG_HOME/ani-gui/config.toml` on
  Linux, equivalent paths on macOS and Windows).
- **Metadata cache** — anime details, episode lists and similar
  information fetched from public APIs. Stored in a local SQLite
  database under your OS's cache directory.
- **Image cache** — posters, covers, banners, episode thumbnails and,
  with an account connected, your tracker avatar, as files under your
  OS's cache directory (`$XDG_CACHE_HOME/ani-gui/images/` on Linux).
  The oldest are removed once the cache passes the size limit set in
  Settings, and Settings can clear it.
- **Watch history** — a plain-text file in the app's own state
  directory. Lists what you've watched, where you left off and when,
  and the Kitsu id of the show you played. More about each show you
  watch is kept beside it: in the metadata cache, when you last
  watched it, the Kitsu entry it maps to and whether a play stored
  that link, the Kitsu entry its title matched (for a show watched
  before the history recorded the Kitsu id), the stream addresses
  resolved to play it, and the opening and
  ending times fetched for the episodes you played; and, in a file
  beside the history, how the show's episode numbering lines up with
  Kitsu's. Deleting a show from your history deletes its row and all
  of these for that show, including the copies earlier versions of the
  app wrote, except a Kitsu entry matched by title that another row
  still in your history also searches — the same title on the same
  catalogue, or once a trailing episode count is dropped, or on either
  catalogue under an older version's record — which stays with that
  row. If the streaming catalogue renamed the show, the Kitsu
  entry matched under its old title is found by the entry it names,
  and stays only if the history never linked that entry to the show or
  another show still in your history claims it. Opening and ending
  times are found by the Kitsu ids the history knows for the show:
  times under an id another show still in your history claims stay
  with that show, and times fetched under an id the history never
  linked to the show stay until they expire, which the app deletes
  when it starts, or you clear your history. Clearing your
  history deletes every row, all of these for every show, every cached
  stream address and every cached opening and ending time, including
  ones a page resolved ahead for a show you never played. Clearing
  removes the numbering of the shows in your history at the time,
  under either streaming catalogue's listing the app linked to them;
  the numbering of a show with no row then stays — one a page resolved
  but you never played, or one removed from your history by a version
  of the app from before removal took its numbering too. A removal
  also stands against what the app had begun before it and not yet
  finished writing — a play, or a page resolving a stream ahead, still
  resolving; a watch still being recorded; the home page still
  matching the row: none of it is written for the removed show
  afterwards. Playing the show again after removing it records it
  again. A removal that fails part-way leaves what it has not yet
  removed findable, so trying again finishes it, except as below. Five
  things are not covered. The app knows a removed show by
  its row and by the Kitsu entry it linked the row to — the one the
  row recorded, the ones it was matched to, the page it was played
  from while the app was running: for a show the history never linked
  to a Kitsu entry, opening and ending times fetched for it, and a
  stream address and numbering a page resolved for it on the other
  streaming catalogue — already stored, or still on their way when you
  remove it — are not recognised as that show's. A stream address a
  page resolved on the other streaming catalogue under a version of
  the app from before stream addresses recorded the page they were
  resolved for is missed for any show, with the numbering beside it.
  The app finds the other catalogue's numbering through the stream
  addresses resolved there, so numbering whose stream address is gone
  is missed too: one a hand-off to an external player resolved, which
  keeps no address, when the player never started; one whose address a
  later resolve of the same episode replaced with an address on a
  different listing; one whose address stopped working and was
  dropped. A row of the same show under the other catalogue's listing,
  written by a play that switched catalogues, is a row of its own: the
  home page groups and removes it with the show once it has shown it,
  and otherwise it stays in your history. And when the app cannot
  write the file that holds the numbering, the numbering is missed:
  the show's own stays, and so does the other catalogue's, whose
  stream addresses are removed all the same, so trying again no longer
  finds it. A stream address that is missed stays until it expires —
  the app deletes expired stream addresses when it starts — or you
  clear your history; numbering that is missed stays as that of any show a page
  resolved and you never played does. Catalogue data shared with
  browsing — anime details, artwork, episode lists and which catalogue
  carries a show — is not part of your history and stays in the cache,
  as it does for shows you only browsed. When Kitsu answers that an
  entry a show in your history names no longer exists, the app notes
  that for up to a year, so the show can be found under its new entry;
  deleting the show removes the note for the entries it was known by
  that no other show in your history names — though not for an entry
  the show was known by only through a link an earlier version of the
  app stored, which the app no longer keeps; that note stays out its
  year — and clearing your history removes every note.
- **Episode positions** — how far into an episode you got, so
  reopening it resumes there: an episode is marked started when you
  choose it to play, a point in its first 15 seconds is kept as its
  start, and one in its last 90 seconds — or an episode played to its
  end — is forgotten as finished. At most the 200 most recently kept
  are held. Kept in
  the app's local browser storage (under your OS user-data directory),
  keyed by the show's Kitsu id and the episode number. Clearing your
  watch history forgets them all. Removing a show from Continue
  Watching forgets that show's positions — found by the Kitsu id its
  history row recorded, even one Kitsu has since deleted, and by the
  card's matched show, or, for a card whose match never loaded, by the
  show id → Kitsu id mapping a play records, read before the removal
  deletes it. Where none is available — an older row that recorded no
  id, whose mapping was refused or failed to save, expired thirty days
  after the show's last play, was dropped as wrong, or cannot be read
  — that show's positions stay until the history is cleared or they
  are pushed out by newer ones. They also stay while another history
  row for the same show remains — it is still a Continue card, its
  show told by the Kitsu id it recorded unless Kitsu answers that
  entry deleted, then by its match, then by its mapping — or while a
  remaining row's show cannot be told (its match has not resolved, and
  Kitsu cannot be asked about the id it recorded, or it has no live
  recorded id and no readable mapping). Separately, a position
  written while playing from a Continue card records that card's
  history row, and removing the card forgets every position its plays
  wrote last — whichever show it is under, including a show the card
  was matched to by a guess that a later load corrected — except one
  under a show another remaining row is still a card of, and none
  while a remaining row's show cannot be told, as above. Playing the same episode again from the show's own
  page takes the position over, and it then follows the rules above,
  as positions kept before rows were recorded do. Forgetting a
  show's positions, a card's, or all of them, also forgets where a
  stream that was recovering from a dropped connection stood, which
  the app keeps in memory until that episode is opened again. What
  removing a card forgets is what was kept when it was removed:
  playing the show again afterwards keeps its positions again, and
  where its stream stood while recovering, the same way the history
  records a show played again after removing it.
- **OAuth tokens** — if you connect an account (see below).
  Encrypted via your operating system's keychain (libsecret on Linux,
  Keychain on macOS, DPAPI on Windows) through Electron's
  `safeStorage` API, then written to your OS user-data directory.
- **Tracker list cache** — if you connect an account, a snapshot of
  your AniList / MyAnimeList list is cached in the same local SQLite
  database so the Watch Later rail and your progress don't re-download
  your full list on every launch (when online, the app still makes a
  lightweight call to confirm your identity and map titles to local
  cards). Each row holds which provider it came
  from and your user id on that provider, the show's identifiers (the
  provider-native id plus its cross-provider MyAnimeList id), your
  status, episodes watched, score, and title, and two timestamps (the
  provider's last-updated time and when the row was cached). Dropped
  when you disconnect the account; if that local deletion fails (for
  example the database is momentarily unavailable), the rows are left
  on disk. Reconnecting the **same** account and re-syncing replaces
  them; otherwise they remain until you delete the local database.
- **Window size** — the size your desktop gave the app's window when
  the app opened it maximized, per display (the display's identifier, resolution and
  scale identify it), in `window-geometry.json` in the app's folder under
  your OS user-data directory, so the next launch opens at that size.
- **Remote flags cache** — the last copy of the project's remote flags
  file the app fetched (see below), and when it was fetched, in
  `remote-flags.json` under your OS's cache directory. It holds only
  the maintainer's switches, nothing about you.

## What ani-gui transmits to public services

Most requests follow what you open: a page fetches what it shows, and
pages resolve episodes ahead of a play (below). At launch the app
checks for updates and fetches the remote flags file (both below) and, with an account connected, refreshes
its sign-in if it has expired; both happen again when changing the
language reloads the window. The app always opens on the home page,
which as it loads fetches its catalogue rows and their images,
resolves your Continue Watching rows and checks that they are still
available, checks the availability of the shows on the page — asking
the streaming site about those it has no recent answer for — and, with
an account connected, loads the Watch Later rail from your saved
tracker list, asking the tracker to confirm who you are and fetching
the list again when the saved copy is six hours old or more. For
each kind of request:

- **Anime catalogue lookups** — Kitsu, AniList, and the streaming
  catalogue playback resolves against: hianime (reached at hianime.at). anidb.app was asked first
  until it was retired in October 2026, and nothing contacts it now.
  Searches and episode listings are asked for as you browse and play;
  the rows of shows on the home page and the search page, and a show
  page's strip of related shows, ask the streaming catalogue in the
  background whether the shows they list are available there, for the
  shows the app has no recent answer for; and a background warm a page runs asks
  for what is in view — the
  detail page for the episode its Play button targets, which is the
  resume point when there is one, and the play page for the episode
  after the one playing, or, with resolution caching turned on, both
  pages for every aired and playable episode in view (the detail page's
  grid, the play page's episode strip); the warm follows the page rather
  than running once on opening, so paging the grid or the strip to new
  episodes, or changing the audio mode or quality setting, resolves
  what is newly in view. These requests carry the search terms you
  typed or the anime IDs you're browsing, and no account identifier.
  MyAnimeList is reached only as your tracker, and reading a show's
  details from your tracker before writing your progress to it is a
  tracker request, which carries your sign-in (below). A request to hianime's own
  site follows a redirect only while it stays on that site; one that
  points anywhere else is not followed, so the site it names never
  receives the request. Resolving an episode through
  hianime also fetches an embed page from the embed hosts the site names
  for that episode — one at a time, the hosts whose pages the client knows
  how to read first — megaplay's ahead of zokoanime's, in the site's order
  within each — then the rest in the site's order, moving to the next when
  a page cannot be fetched or read or when the stream its page names does
  not answer as a playlist, so one resolve can reach more than one of
  them, and a host the client cannot read is reached only after every one
  it can has failed — as the listing names them: a listed page that
  redirects is followed wherever it goes, so a host the listing did not
  name can be reached through one it did, ahead of hosts listed after it —
  and on megaplay's pages the sources endpoint of the host that served the
  page as well, since that is where its player gets the stream, and the
  host that served a page is not always the one the site's listing named,
  because the site moves those pages between hosts and the request follows
  the move. The site chooses those hosts, and they can change without an
  app update; at the time of writing the listings name zokoanime.video,
  megaplay.buzz — and the numbered mirrors the site serves megaplay's
  player from, `megaplay-1.buzz` and the like, which the client reads as
  it reads the host — and vidtube.site, and the client fetches from
  whichever the site lists for the episode. The site can stream an episode
  from more than one content delivery network, and the sources request
  says which one it is answered for; the app fetches a megaplay page by
  its origin and path, without the network the site's listing named it
  for, and asks every megaplay server for the one network whose streams
  it can play, whichever network the listing named. So the hosts a megaplay page leads
  to are that network's, and no other's.
- **Images** — posters, covers, banners, episode thumbnails and, with
  an account connected, your tracker avatar are fetched from the host
  the catalogue or tracker names for them (Kitsu's and AniList's image
  hosts, for instance), some as soon as a catalogue answer names them,
  and cached on disk. A cached image is not fetched again until the
  size limit removes it or you clear the cache. These requests carry
  no identifier of yours, except the avatar's: its address is the one
  your tracker gives for your account, and it names the account
  (AniList's and MyAnimeList's avatar addresses carry your user id
  when you have set an avatar), so the host serving it can tell whose
  avatar is asked for.
- **Video playback** — the chosen episode's playlist, its segments and
  any sidecar subtitle files are fetched directly from the source CDN
  the catalogue or its embed page names. For hianime that is, at the
  time of writing, a host under aniwatchtv.uk behind the zokoanime
  pages, and behind the megaplay pages the hosts the sources endpoint
  names for the network the app asks for — ncdn.imgnex.top for
  playlists, bb.akirax.buzz for segments, xdw5v.qeltrix.top for
  subtitle files. The site's other networks stream from hosts of their
  own, megap.norami.top and fetch.nexabloom.top among them, and none of
  those is contacted, because the app never asks for a network other
  than the one it plays. The app does not restrict these fetches to a
  list of hosts, so the provider's choice is what it reaches. For hianime the CDN sees a normal `Referer` naming the origin
  of the embed page the stream was read from — the host that actually
  served that page, which is not always the host the catalogue's listing
  named, because the catalogue moves those pages between hosts and the
  request follows the move. Nothing identifying you is sent with it. The CDN is also reached
  before any playback, whenever an episode is resolved or a cached
  resolution is reused: a resolve — a page warm, a play or hand-off
  that finds nothing cached, or a download, which always resolves
  afresh — fetches the episode's master playlist, to
  check that what the page named is a playlist, and, when the quality
  setting is not "best" and the master offers a rendition matching
  it whose address resolves against the master's, that rendition's
  playlist as well — a second fetch, at whatever host the master
  names for it; a master with no such rendition, or one whose
  matching rendition's address does not resolve, is kept as it is,
  with no second fetch; and any play served from the
  resolution cache — a page warm,
  a play in the app, or a hand-off to an external player or Syncplay —
  checks the cached stream and each of its subtitle tracks at the CDN
  before the row is trusted, so a cached replay reaches every listed
  subtitle host whether or not the player goes on to use that track.
  Segments and
  subtitle files are fetched when playback starts — in the app, or in an
  external player or Syncplay the app hands the stream to — and by a
  download, which fetches the segments through the bundled tool and the
  subtitle files beside them without any playback. Syncplay, once the
  app has launched it, connects on its own to the Syncplay server and
  room you choose in its dialog or its own settings, under the name
  you give it there; the app passes it only the stream, its subtitle
  files and the player's options. By default Syncplay shares the
  stream's address and length with that server and the others in the
  room; its own privacy settings can hash or withhold them.
- **Tracker integration (optional)** — only if you sign in to AniList
  or MyAnimeList:
  - Your OAuth bearer token is sent with every request ani-gui makes
    to that provider as your tracker: your profile, your list, the
    show details read to line up episode numbers before a write, and
    the writes themselves. Renewing your sign-in sends its refresh
    token to the provider. AniList requests made as a catalogue lookup
    carry none.
  - We send your watch progress (episodes watched, status, score)
    when you mark an episode watched.
  - We never send tracker tokens or progress to any third party.
  - The provider's own privacy policy governs how it handles the
    requests:
    - AniList: <https://anilist.co/terms>
    - MyAnimeList: <https://myanimelist.net/about/privacy_policy>
- **Update check (automatic, on launch)** — ani-gui asks GitHub's
  public releases API (`api.github.com`) for the project's newest
  release, so it can tell you when a new version is out. It asks again
  when changing the language reloads the window. Besides standard
  headers that say nothing about you (GitHub's `Accept` and
  API-version headers, and the compression it accepts), the request
  carries only the User-Agent `ani-gui`: not the version you run,
  which is compared on your machine, and no account information. Which release it asks for follows the
  "Include pre-releases in update check" setting, and the choice shows
  in the request's address. With the setting on (the default), it asks
  for the newest release of any kind
  (`/repos/JoaoPucci/ani-gui/releases?per_page=1`). With it off, it
  asks for the newest full release
  (`/repos/JoaoPucci/ani-gui/releases/latest`). The check cannot be
  turned off; the setting only chooses between those two requests.

  Versions before 0.12 made a second automatic request here: the
  bundled copy of the `ani-cli` script fetched its own latest version
  from upstream at every launch. The app no longer carries or runs
  that script, so that request is gone.
- **Remote flags (automatic, on launch)** — once per launch, ani-gui
  downloads a small file of switches from the project's GitHub
  repository, through GitHub's content server
  (`raw.githubusercontent.com`). If that download fails, it is tried
  again later in the same session (after a few minutes, then at most
  hourly) until it succeeds. The maintainer uses it to switch off a
  feature that turned out to be broken, without waiting for a release.
  Like the update check, each request tells GitHub your IP address,
  when it was made, and that it comes from ani-gui.
  Nothing else is sent: no account
  information, nothing about what you watch, and no identifier of your
  install. The file holds only the switches; GitHub's own privacy
  statement governs the request:
  <https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement>.
- **Aniskip OP/ED timing (automatic, when an episode plays in the
  app)** — once an episode playing in the app's own player has a
  known length, ani-gui asks the Aniskip community service
  (`api.aniskip.com`) for crowd-sourced opening and ending times. They
  drive the Skip button, and the automatic skip when the auto-skip
  settings are on. The request names the show's MyAnimeList id, looked
  up from its Kitsu entry, the episode number, and the episode's
  length in seconds. A show with no MyAnimeList id, a play with no
  Kitsu entry, and an episode numbered 0 or with a fractional number
  are not asked about. No account information is sent. Answers are cached, so
  playing the episode again does not ask again while the cached times
  last. The auto-skip settings do not stop the request, and no setting
  does.

There is no analytics or telemetry: no request reports how you use
the app, and the update check and the remote flags file are the only
requests that go to the project's own repository.

## Connecting and disconnecting an account

Connecting AniList or MyAnimeList uses the
standard OAuth Authorization Code flow:

1. ani-gui generates a one-time random `state` value and PKCE pair.
2. Your default web browser opens to the provider's consent page.
3. The provider redirects to a local-only callback
   (`http://localhost:53682/callback`) that only ani-gui can read.
4. ani-gui trades the returned code for an access token.
5. The token is encrypted via your OS keychain and stored locally.

When you disconnect from `/account` in the app:

- The local encrypted token file is deleted immediately.
- ani-gui's local cache of your list is dropped.
- The token remains valid on the provider's side until you revoke it
  there (AniList: <https://anilist.co/settings/apps>; MyAnimeList:
  <https://myanimelist.net/apiconfig>). Disconnecting in ani-gui does
  not revoke server-side authorisation.

## Children's privacy

ani-gui is not directed at children under 13. We do not knowingly
collect any data about anyone, including children, because we do not
operate a server.

## Changes to this policy

When this policy materially changes, the change ships in a new release
and is recorded in the git history of `docs/PRIVACY.md` in the
project repository. There is no separate notification channel because
we do not collect contact information.

## Contact

The project has no channel for questions about this policy. Its
GitHub repository has issues and discussions turned off and offers
no private reporting; it accepts pull requests, which are public. The
policy's history is public in the git history of `docs/PRIVACY.md`.
