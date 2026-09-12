# Deferred work

Valid work that was found during a change and deliberately not done in
that change. An entry says what the work is and why it waited, plus
anything genuinely surprising about it — enough for someone who was
not in the conversation to pick it back up.

These are reminders, not specifications. An entry does not have to
enumerate the files that will change, state acceptance criteria, or
define what "done" looks like; whoever takes the work scopes it
against the code as it is then, which is the only scoping worth
trusting. So an entry that states something false is a defect, and an
entry that leaves things out is not.

This file is tracked, so an entry survives leaving the checkout it was
written in and a pull request thread can cite it. That is the whole
point: the internal planning directory is git-ignored and this
repository has issues disabled, so neither can hold a record anyone
else is able to read.

Adding an entry is not a way to avoid the work. `AGENTS.md` §14 lists
it third of four options, after doing it here and doing it in its own
pull request.

Write an entry about the code, not about the work in flight. An entry
stands on its own: what is wrong, where it lives, and what should
change, all readable without following anything. If taking the links
out would take the point with them, the entry is not written yet.

On that condition a commit sha or a merged pull-request number is
welcome. Both resolve forever, and where an entry is about this
repository's own history they are the precise evidence for it. Cite
them underneath the explanation, never in place of it — the reader
should reach for one to confirm what the entry already told them, not
to find out what it meant.

What stays out is state that expires: a branch name, a commit count, a
sentence like "the PR for this is open". A rotted entry is worse than
no entry, because it sends the next reader to rebuild finished work or
to reason from a state that no longer exists. References out of the
repository that do not rot — an upstream issue, a specification — are
fine on the same terms.

Remove an entry when the work lands, in the change that lands it.

---

## Known gaps in the deferral checks

What `tests/arch/deferral_record.sh` gets wrong today.

**Intent-to-add is read out of porcelain rather than index metadata.**
`record_is_recoverable` rejects a `git add -N` entry by matching `" A"`
in `git status --porcelain`. Delete that file from the working tree
afterwards and the porcelain line changes, so the check stops
recognising the state it is there to catch. Reading the intent-to-add
bit from index metadata is the robust form. A deleted-file variant of
the same state is worth handling in the same change.

The failure message is wrong for this case too. `why_unrecoverable`
sees `ls-files` succeed and reports "tracked as a symlink or submodule
rather than a file" — neither true nor actionable, when what the
reader needs is `git add` on a path that is already, in a sense,
added. A check that names the wrong reason is a defect in its own
right rather than a cosmetic one, because it sends whoever hits it to
fix something that is not broken. Correct the message in the change
that corrects the detection.

**A setext H2 does not end the section.** The body scan recognises
`^ {0,3}## ` only. A heading written as a line of text underlined with
`-` is also an H2, so the scan runs past it into the section below.

Adding setext handling is the wrong response. This is the
regex-interpretation trap `AGENTS.md` §2 describes, where each rule
added reveals the next one; the way out is to stop parsing document
structure, not to parse more of it.

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**Treat every entry as a lead, not a fact.** These were carried across
from session state and reviewing them turned up more than a dozen
errors — mostly work that had already shipped, a couple of problems
that never existed as described. Check an item against the code before
starting it, and delete it when you find it done.

## Correctness in the app

- **Decide the resolution cache's final shape after the opt-in
  trial.** Caching play resolutions is a setting now
  (`cache_resolutions`), default off: every play resolves fresh, the
  page-mount warm narrows to the single next episode, and the
  day-one quality pin — a cached master URL replaying a release-day
  encode for up to seven days — cannot bite unless the user opts in.
  Rows written while the setting was on are ignored when it is off,
  never cleared, so toggling is lossless.

  What remains is the decision the trial informs: keep the setting,
  reshape the cache (re-resolve only the master on a hit, keeping
  the disambiguation — ~2–2.5s per play, closes the pin with the
  cache on), or remove resolution caching outright. The measured
  frame (2026-09-01, live provider, debug build, one residential
  connection; per-request timings land in the transport's debug
  log): a clean resolve is ~4.7s over 7 requests, a franchise-heavy
  title needing sibling probes ~6.8s over 10, a cache hit ~0.5s;
  candidate disambiguation dominates at ~2–3s, the post-pick episode
  leg is ~1.8–2.4s. The pin itself was only ever confirmed as a
  hypothesis — a day-one Bleach episode improved after a full cache
  clear, which also reran candidate selection; nobody captured the
  two master URLs to compare.

## Testing and CI

- **The CRAP ratchet disagrees between CI and local** — 26 against 25 —
  and three files sit at 29.7–30.0, right on the high-risk boundary.
- **The pre-commit hook and strict TDD are in tension — for frontend
  commits.** `frontend-test` is the only hook command that runs tests,
  so a frontend `test(red):` commit fails by construction and is
  rejected. It has been worked around with `--no-verify`, which
  disables every other check too; the workaround is the bug.

  The non-obvious part: pre-commit cannot see the commit message. Git
  writes `COMMIT_EDITMSG` after pre-commit runs, even for `git commit
  -m` — verified with a probe hook. So the gate has to move to
  `commit-msg` and skip only for a `test(red):` subject.

## Correctness in the app (continued)

- **The play page's same-URL attach shortcut skips the video error
  listener.** The attach effect returns early when the singleton
  already carries the exact media URL (the PiP-return path, kept so
  a working pipeline isn't torn down), but the element's `error`
  listener is registered below that return — so a session entered
  through the shortcut has no element-error recovery: a rotated URL
  dying under it surfaces nothing and retries nothing until the user
  navigates. Found while writing the acceptance case for the
  source-down failure copy, whose first draft accidentally took the
  shortcut and dispatched an error nobody heard. The fix wants the
  same source-scoped treatment the progress and resume listeners
  got, not another registration inside the effect's conditional.

## Interface

- **Localised content fetch** — synopsis and episode titles.
- **Franchise and season grouping** across surfaces.
- **Play-page keep-alive → normal reload with a persisted position.**
- **Search has no sort *direction* control.** Sorting by relevance,
  title, year and rating ships, as do subtype filter chips; only
  ascending/descending is absent. Name any further filters wanted
  before starting, rather than reading this as filters being missing.
- **Update notifier is not resilient to GitHub rate limits.**
- **Adopt the `documentPictureInPicture` browser API** for the player's
  pop-out window. Not a request to write documentation — "Document
  Picture-in-Picture" is the W3C API's name.

  Electron exposes the API but omits the window-creation glue, so it
  cannot work today: electron/electron#39633, open since 2023. Do not
  re-attempt until that lands. PiP as it exists now — the singleton
  video that survives navigation — ships and is described in
  `README.md` and `docs/architecture.md`.
- **A flatpak-only mpv goes undetected by the external-player
  surface.** Upstream ani-cli fixed exactly this for its own player
  launch just before the repositories parted (pystardust/ani-cli
  #1858 and #1863: system-wide installs under `/var/lib/flatpak`,
  user-level under `~/.local/share/flatpak`, app id `io.mpv.Mpv`) —
  kept here as a reference for probing the same locations, not as
  code to port.
- **Illustrated brand assets** — post-1.0.
- **A notification center.** Two jobs, and the second is the reason
  the feature exists. The first is aggregation: the app's notices are
  scattered today — update availability is a topbar badge with its
  dialog, download outcomes live on the dock's terminal rows, and the
  diagnostics page holds boot-time notices — and a single surface
  would give them, and whatever later features emit, somewhere to go
  when the user was not looking. The second is telling users about
  outages like the provider failure of 2026-08-27 (see "Additional
  providers" below): every uncached play failed as unreachable and
  the app had nowhere to say the problem was the provider's, not
  their setup's. That job needs a
  notice source that does not exist yet — the app inferring an outage
  from its own failures, or fetching announcements from somewhere it
  trusts — and choosing one is the design work, along with which
  signals feed the surface and what persistence they get.

- **Nothing enforces the red-before-green pairing.** `AGENTS.md` §2
  requires a `test(red):` predecessor for anything that introduces a
  `feat` or a `fix`, and spells the verification out as a mechanical
  procedure, but nothing runs it. Unpaired commits have reached
  master; each was caught by a reviewer reading the log by hand, or
  missed.

  Four things trip an implementation, and the first is what let the
  known violations through:

  - **The subject is the type, not the scope.** The rule covers every
    `feat` and `fix`, so a gate keyed on `feat(green):`/`fix(green):`
    misses a bare `fix:` — which is the form every unpaired commit
    took.
  - **Ancestry is not pairing.** Asking whether the branch contains
    *a* red passes a green whose red landed on a separately-merged
    branch. Narrowing that to "this green has a red **ancestor**" is
    still too weak: once one honest pair lands, its red is an
    ancestor of everything after it, so the next unpaired green
    inherits it and passes. The gate has to attribute a specific red
    to a specific green — §2 reads that off adjacency, each green's
    parent being its red — not merely find one somewhere behind it.

    Direction matters too, wherever an ancestry test does get used:
    `--is-ancestor <green> <red>` is not the ancestry question
    negated. It exits nonzero for the separately-merged case exactly
    as it does for a correct pair, so all it detects on its own is a
    red committed after its green.
  - **Upstream commits are exempt** (§2). A sync merge imports
    upstream history verbatim, so its `feat` commits have no red of
    ours and cannot acquire one. Provenance is mechanical: reachable
    from the sync merge's second parent.
  - **It has to run before the merge.** A squash merge fuses a
    branch's reds and greens into a single commit, so for anything
    landed that way master's history no longer holds a pairing to
    check and a gate reading master cannot reconstruct one. This
    repository merges both ways, which makes the shortfall silent
    rather than obvious. Run the gate over `master..<branch-head>`
    while the branch is still there. `0dccb527` is one of the fused
    commits, carrying its tests and its fix together.

  Related to the pre-commit and TDD tension above — both are about
  giving the contract teeth instead of restating it.

- **Per-episode audio-mode caps.** Availability answers whether a
  show carries the requested mode, not which of its episodes do, so
  a partly dubbed show advertises its whole listing under the dub
  key and an undubbed episode fails when clicked.

  This is a regression the anidb switch introduced, not longstanding
  behaviour. allanime returned `availableEpisodesDetail` with
  separate `sub` and `dub` arrays, so a single show fetch gave the
  exact per-mode cap and the per-mode extras for free. anidb's
  episode listing carries no audio at all — it lives on each
  episode's languages row — so the same answer now costs one request
  per episode.

  Deriving the prefix anyway was attempted and withdrawn: a
  sub-linear search cannot vouch for rows it never fetched, and
  three review rounds each found the next place where an unfetched
  row got counted (the tail, the front, then anywhere below a
  bisected boundary). Whoever picks this up needs a cheaper source
  of per-episode audio, or a budget for the full scan on listings
  small enough to afford it — not a smarter search over the same
  requests.

## Additional providers

- **Investigate alternative stream providers and add the viable ones**,
  so playback survives the current provider having a bad day. All
  resolution rides a single provider today, and on 2026-08-27 its
  server-rendered routes stalled globally for hours (TLS completed,
  then zero bytes until timeout) while its JSON routes kept answering
  — nothing new could be resolved, and every uncached play was
  correctly reported as unreachable. Plays kept working only where a
  cached resolution sat inside its seven-day lifetime *and* its
  stream URL still answered validation — a dead URL evicts the row and falls
  through to the unreachable provider. That softens the blow without
  changing the lesson. pystardust/ani-cli#1877 records the same
  outage from the outside.

  The investigation half is the real work: which providers are worth
  scraping, what their catalogues and rate limits look like, and how a
  second provider slots into resolution (fallback when the first is
  unreachable, or a per-title choice). The title-resolution bridge
  (`docs/title-resolution.md`) is keyed by provider ids, so every
  cache stamped by provider output is part of the answer, not an
  afterthought.

  That investigation ran on 2026-09-05, during a second, total outage
  of the provider: `docs/proposals/additional-providers.md` holds the
  candidate survey, the integration shape, and a recommendation. The
  survey's liveness claims rot; re-verify them before building.

## Retiring the legacy-script sweep — the v1.0 marker

- **Remove the boot sweep that cleans the retired script from old
  installs**, and everything that exists to report it: the sweep
  module, the field it carries onto app-info, and the diagnostics
  notice that renders it. The maintainer has tied this removal to
  v1.0 — it is the last piece of the app that exists because of what
  the app used to be, and shipping without it is what 1.0 means here.

  Why it waits: the sweep serves installs upgrading from before 0.12,
  which kept a maintained copy of the script in the cache. Removing
  it orphans that copy for anyone who jumps straight from an old
  version to a post-removal one — the file just stays, unreported.
  The judgment of when that cost is acceptable is the whole decision,
  which is why it is a version call and not a cleanup.

## Recovering a download's abandoned claim automatically

- **Take back the empty file an interrupted download left at an
  episode's name**, instead of asking the user to delete it.

  Where the destination has no hard links, publication claims the name
  by creating it empty and renames onto its own claim. A process that
  dies between those two calls leaves the empty file, and the app now
  refuses that name and says so rather than clearing it.

  Clearing it automatically was tried three ways and each failed
  differently. Gating on the download's lock does not work: the lock
  file is named for the target, so two spellings of one name take two
  lock files, and requiring a lock blocks recovery wherever no lock
  can be made. Unlinking and re-claiming leaves a window where the
  name is free. Renaming over the claim closes that window but still
  acts on a classification that can be stale, so two publishers each
  replace the other's finished file — and the target name carries
  neither mode nor quality, so those need not be the same episode.

  What is missing is a conditional replace: swap in a file only if the
  name still holds exactly what was inspected. Nothing portable to
  FAT32 and exFAT on both Linux and Windows offers one. Anyone picking
  this up should start there rather than at the reclaim, and should
  know that every scheme layered on top of a plain classify-then-act
  has been tried on this branch.

## A scratch name that overflows where the target does not

- **Publication's scratch file adds ~55 characters to the destination
  path, so a folder deep enough can accept the episode's own name and
  reject the scratch's** — the transfer then fails on a target the
  user was allowed to choose.

  Closing it is not a rename: the spawned tools receive the scratch
  path as an argument, so the honest fix hands them a name relative
  to the destination (`current_dir`) — but that alone moves the
  failure later instead of removing it, because the app's own stat,
  link and rename still use the full path, and `std` has no
  `renameat`-style relative operations. The whole of it means
  directory-handle-relative filesystem work (a `cap-std`-shaped
  dependency) on Linux and a separate answer for Windows path limits.

## A content security policy for the renderer

- **The renderer runs under no content security policy, in the
  packaged app as much as in development.** Nothing sets one.
  Electron prints its insecure-policy warning on every development
  launch for exactly this and suppresses the same warning once
  packaged, so the warning going quiet is not the gap closing.

  What the policy would guard against is script injected into the
  renderer, and today's exposure is narrow: the window runs with
  context isolation on and node integration off, the packaged app
  loads its bundle over the app's private scheme (development loads
  the Vite server instead, over plain HTTP by default — the URL is
  an environment override — which is one more reason the two builds
  will not share one policy), and no component
  injects HTML — Svelte escapes every string it renders. A policy is
  still the standard hardening for an Electron renderer, and one of
  the items on Electron's security checklist the app does not meet.
  Two others are known. Navigation: the window refuses new windows
  through its open handler but installs no `will-navigate` guard, so
  nothing stops the renderer's own document from navigating away
  from the app's origin; that closes together with the policy, in a
  few lines of the main process. And the sandbox: the AppImage is
  repacked to start Electron with `--no-sandbox`, because the
  unprivileged FUSE mount an AppImage runs from does not honour
  setuid execution, so Chromium's setuid sandbox helper cannot be
  used from inside it (the `.deb` sets the helper's SUID bit in its
  postinst and keeps the sandbox), which is a packaging question of
  its own and not part of this entry.

  Two things a grep will not surface. The manual diagnostic route
  hands a pasted public URL straight to the player, the one place
  the renderer loads a stream from anywhere but the loopback
  backend, so a policy limited to the loopback closes it — which is
  the boundary rule catching up with the route, since every stream
  is meant to pass through the proxy; the work sends it through the
  proxy like every other stream, or retires it. And the HLS player
  attaches media through blob URLs, which no origin-shaped rule
  covers.

  It waited for a play to verify against — one that goes through
  the proxy and the event stream the page listens to. At the time
  of writing the only provider's catalogue answered every request
  with a maintenance page, so a fresh resolution could not be had on
  the default branch; only an installation that had opted into
  cached resolutions, with a row still live, could play through
  that path, and the diagnostic route's pasted URL exercises neither
  the proxy nor the event stream.

## Three listening and watching modes: openings, soundtracks, reactions

Three ideas for what the app could offer beyond episodes, each still
open-ended. What is written here is the state of one round of
research on 2026-09-12 — which sources exist, what each was seen to
do, and what closes a path — not a shape. Whoever picks one up
researches it again against what the services do then, and decides
the shape against the app as it is then.

- **An openings mode: watch a show's opening and ending sequences,
  browse and search them, play them in a queue.** The only known
  source of truth is AnimeThemes, which hosts the opening and ending
  videos themselves (not only the songs): a WebM per theme at 720p or
  1080p with a separate OGG audio file, the song title and artists,
  the theme's number and kind, credited and creditless variants, and
  links to the MyAnimeList, AniList and Kitsu ids of each show —
  the ids the title-resolution bridge already speaks — plus an
  AniDB.net id, which is a database's id and nothing to do with the
  anidb.app streaming provider, resolved by title. Seen on
  the day: the API answers without a key at ninety requests a minute
  and its CDN serves the files to a plain GET with range requests;
  its JSON API is marked deprecated in favour of a GraphQL endpoint
  with a lookup by external site id; and the CDN answers 403 to a
  HEAD request while serving the GET, which any liveness check has
  to allow for. WebM plays natively in the renderer, so no HLS
  machinery is involved, and the proxy already has a progressive
  pass-through route beside its HLS one — but that route is chosen
  by a media kind the session infers from the URL's extension, and
  the kinds it knows are `m3u8` and `mp4`; anything else, `webm`
  included, falls to HLS and is handed to the manifest parser, so
  a session for a theme is not only wiring but a media kind the
  classifier learns or the caller sets outright (the manual
  diagnostic route does not go through the proxy at all: it hands
  its pasted URL straight to the player).
  The alternative — cutting openings
  out of episodes with the skip intervals the player already has —
  was looked at and set aside: the intervals are crowd-sourced and
  shift between releases; a cut without re-encoding lands on a
  segment or keyframe boundary rather than on the interval's exact
  second, and a frame-exact cut re-encodes around the boundaries;
  and the result is the credited opening at stream quality. What it
  does make cheap is a "jump to the opening" inside the normal
  player.

- **A soundtrack mode: a show's music, playable where a source
  allows and linked out where it does not.** Full playback inside
  the app has three known sources: the OP and ED audio AnimeThemes
  serves beside its videos; the rights-holder uploads on YouTube,
  where labels distribute soundtracks through auto-generated artist
  channels (seen for one show: the composer's own channel carrying
  the tracks, the studio's channel carrying the music videos); and
  the user's own library, whether a folder or a Jellyfin, Navidrome
  or other Subsonic-compatible server, all with open DRM-free
  interfaces. What closes the streaming services: Spotify, Apple
  Music, Deezer and TIDAL gate full playback behind a DRM module the
  app's Electron build does not carry, plus a subscription; Spotify
  also removed preview links from its API for new applications in
  November 2024; SoundCloud's API is closed to new applications
  unless approved case by case. What they still give: thirty-second
  previews from the iTunes and Deezer search APIs with no key and
  with cover art, previews through TIDAL's own player module, and a
  link out per track, which needs nothing. For album and tracklist
  data, VGMdb has an unofficial JSON front and MusicBrainz an
  official API; Jikan lists only the theme song titles. So linking
  is the fallback per track, not the design — but which sources are
  worth the maintenance is the open question.

- **A reactions mode: find and watch reaction videos for an
  episode.** No index of reaction videos exists anywhere; YouTube is
  the whole corpus, and every option runs through it. Discovery has
  three known routes and each has a cost: the YouTube Data API
  needs a project key — bundled in the app it gives one project a
  hundred searches a day shared by every user, supplied by the user
  in settings it gives each user their own quota at the cost of a
  setup step; the public Invidious and Piped
  instances are blocked at the IP level and down to a handful; and
  the yt-dlp the app already bundles searches YouTube with no key —
  seen working on the day, with titles, channels, durations and view
  counts — but scrapes a private API that breaks when YouTube
  changes, which means the app would need a way to refresh yt-dlp.
  Playback: YouTube's embedded player in a frame is the sanctioned
  way, common, and plays every quality; its costs are a Google
  origin inside the app (the renderer's content security policy,
  deferred above, would allow that one frame source, and the
  no-cookie embed domain limits tracking), ads in the frame, and
  videos whose owners refuse embedding. And one open question sits
  in front of it: the packaged app serves its page from a custom
  `app://` origin, Chromium sends no Referer from a non-HTTP one,
  and YouTube's player refuses an embed that identifies no site
  (its error 153), so a frame that plays under the dev server's
  HTTP origin is not known to play in the shipped app — that needs
  checking from the packaged origin, and if it fails, a way to
  identify the site (a header the main process adds to the frame's
  requests, or the player's own client-identity parameters), which
  is another piece of the written exception below. Extracting the
  stream with yt-dlp and relaying it through the proxy keeps the
  app's boundary but sits in a grey area of YouTube's terms and caps
  combined audio-and-video at 360p. The proxy rule was written for
  provider streams — the referer their CDNs want, CORS, local
  tokens — and none of that applies to a third-party player, so the
  frame would want a written exception rather than a design bent
  around the rule. A first shape that touches none of that exists:
  discovery in the app from a curated channel list, playback in the
  browser through the link-out that already exists. The external
  player is one step further than it looks: the hand-off gives the
  player a page URL, and mpv plays a YouTube page only by calling
  yt-dlp, which the app bundles but puts on a spawned tool's search
  path in the download flow alone — the external-player spawn
  inherits the app's environment — so that route needs the same
  path wiring on the hand-off, or a yt-dlp the user installed
  themselves.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
