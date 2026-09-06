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
- **Subtitle presentation: user styling, and the anime's own
  colours.** Subtitle cues render as the browser draws them — the
  play page only picks which track shows — with no styling and no
  preference behind it: default font and size, white on a shadow,
  bottom centre, for the tracks inside a playlist and the sidecar
  ones alike. Two directions, not exclusive: a settings surface for
  font, size, colour, background, edge and position, and the
  per-anime accent the app already derives from the cover colour
  tinting the cues, in the spirit of the per-anime theming the
  detail and watch pages do.

  Worth knowing before starting: the browser's cue styling honours a
  short allow-list (colour, background, font, shadow, outline) and
  not position, so moving cues means setting positions on the
  track's own cues or drawing them in an overlay of the app's own
  from the active cues. hls.js delivers in-playlist subtitles as
  native cues too, so one mechanism covers both kinds. hianime's
  sidecar WebVTT carries inline markup (italics) and its own cue
  settings, which an overlay has to honour. External players and
  Syncplay receive the tracks as files and style them themselves —
  this is the embedded player only. A legible default matters more
  than the options: size relative to the video frame rather than the
  window, so Picture-in-Picture and fullscreen both read.
- **Subtitle track selection: a locale-aware default, and a remembered
  choice.** Which track shows when an episode opens is the provider's
  call today, in every app locale: the page lists the tracks the
  video carries and flips one to showing only when the user picks it,
  and the browser turns on the track the provider flagged default —
  English, on every hianime episode seen so far — while the rest
  start hidden and a payload with no flag starts with everything off.
  Nothing persists; the next episode starts from the provider's flag
  again.

  Two halves: prefer a track matching the app language when the
  listing carries one, falling back to the provider's default; and
  remember the user's last pick — a language, or off — across
  episodes.

  It waits on evidence. Every hianime listing captured or played so
  far offered English alone, so the locale half has nothing to
  select from yet; find a show whose listing carries more languages
  before designing against the payload, and check what the label and
  language code look like for them. The remembering half does not
  depend on that and could go first.
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

  The second provider did not change the cost. hianime types each
  server sub or dub, which is the per-episode signal wanted here,
  but it lists servers per episode in its own request, so the
  answer for a show is still one request per episode on either
  provider.

## Filling catalogue gaps from the second provider

- **Ask the next provider when the first answers a clean miss**, so a
  show the first provider does not carry plays from the second
  instead of being hidden. Failover moves to the next provider only
  when the current one was unreachable, refusing or broken; an
  answered miss ends the walk. A finished show with a negative
  verdict is then dropped from the home and search lists, and its
  page disables Play and Download.

  Why it waited: it is a change of meaning, not of one rule. A
  negative verdict means "the provider that answered has nothing";
  with gap filling it has to mean every provider answered a clean
  miss, and every negative row written so far is a single-provider
  verdict about a question nobody would ask any more. The
  availability cache re-keyed for the same reason when the provider
  changed from allanime to anidb.app, and would again. Positive rows
  already name their provider.

  Rate limiting is the biggest risk and the reason to plan before
  building. A clean miss costs a full walk — every alias searched, up
  to five candidates probed — and gap filling makes every genuinely
  absent show cost one such walk per provider, on the page's own
  probe and on the background warm that fills the list views alike.
  hianime's rate-limit temperament has never been measured; the
  survey in `docs/proposals/additional-providers.md` only saw it
  answer quickly. Each provider has its own pacer and breaker, and
  the interstitial check recognises a Cloudflare challenge, but a
  breaker learns after the block, not before. Measure first: what
  the site tolerates for search and AJAX listings at the background
  pace, and whether it answers excess with the challenge page or a
  429.

  Provider affinity for plays exists: a play, a download or a
  handoff starts from the provider the show's positive availability
  row names, and a history row's show key carries its label too.
  What gap filling adds is the walk that produces such a row in the
  first place — the probe runs the base order and, with a miss
  continuing to the next provider, pays a walk per provider on
  every re-probe of a genuinely absent show.

  What makes hianime a fit for this: its entries are season-split
  like Kitsu's, so the count-based picker needs none of the offset
  machinery anidb.app required; a captured search was tight (three
  hits for "cowboy bebop": the series, the movie, one special); every
  decoded embed URL carries the MyAnimeList id, so a pick can be
  cross-checked against Kitsu's MAL mapping after the resolve for
  free (whether the entry page carries it before the pick was not
  confirmed); and it types each server sub or dub, so the bounded
  mode scan applies unchanged.

  Keep hiding, but only on a miss from every provider, and only for
  finished shows as now: a card no provider can play is a dead
  click. Per-title manual choice is not needed for this. If the
  second provider becomes load-bearing for catalogue breadth rather
  than a fallback, its domain churn arrives sooner: the canonical
  domain is filtered per ISP, and the origin is a constant with a
  test override only.

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

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
