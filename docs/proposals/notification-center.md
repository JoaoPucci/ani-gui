# Proposal: a notification center

**Status**: research for the 0.20.0 release, alongside the
[in-app updater](in-app-updater.md) proposal, which would use the same
signature verifier as this one's remote notices (section 3). Nothing
below is built. It supersedes the "A notification center"
entry in [`deferred-work.md`](../deferred-work.md). Service prices and
limits were checked on 2026-11-01 and 2026-11-02 against the pages
cited next to them, and will drift.

Labels used below: **[code]** is something read in this repository,
with its file and line; **[source]** is a fact taken from the external
page cited next to it; **[unverified]** is a claim no source was
checked for; **[proposal]** is a design choice.

## The problem in plain terms

The app already tells the user several things, but each in its own
place, and each is gone once the moment passes:

1. **An update** is a topbar badge with a pulsing light, and a dialog
   behind it. It lasts as long as a newer release exists. [code:
   `frontend/src/lib/components/UpdateBadge.svelte:112`,
   `frontend/src/lib/update/store.svelte.ts:5`]
2. **A finished or failed download** is a row in the download dock.
   The rows live in memory, so a restart forgets them. [code:
   `frontend/src/lib/download/store.svelte.ts:224`–`232`; the store
   has no storage]
3. **A tracker sign-in that needs attention** is a toast that stays
   until it is dismissed. After that, an expired sign-in leaves the
   amber dot on the account chip, and a refused renewal of a sign-in
   that is still valid leaves nothing. [code:
   `frontend/src/routes/+layout.svelte:150`–`190`,
   `frontend/src/lib/account/expiry-toast.ts:57`,
   `frontend/src/lib/account/chip-descriptor.ts:64`]
4. **Files the app deleted at startup** are a section on the
   diagnostics page, which few people open. [code:
   `backend/src/commands/app_info.rs:23`,
   `frontend/src/lib/diagnostics/legacy-sweep.ts`]
5. **An outage of the streaming source** has no surface at all. Each
   failed play shows its own error ("The streaming source is down or
   under maintenance right now…" for an upstream 5xx), and background
   probes stop quietly when the provider's circuit breaker opens.
   Nothing says "the source is down" once, and nothing says it before
   the user presses Play. In the provider failure of 2026-08-27, the
   one that led the app to hianime, every uncached play failed as
   unreachable and the app had nowhere to say the problem was the
   provider's. [code:
   `frontend/src/lib/play/source-answer.ts:38`,
   `frontend/messages/en/play.json:131`,
   `backend/src/scraper/gate.rs:18`–`25`]

The maintainer also wants a place for things the app cannot see by
itself, such as an announcement of an outage or of news, and for a
respectful donation ask.

## 1. What goes in it

Each notice is a **condition** or an **event**. A condition is true for
a while and then resolves itself: an update exists, a sign-in has
expired, the source is failing. An event happened once: a download
finished. Conditions resolve themselves; events are dismissed or age
out (section 2). [proposal]

| Notice | Source today | Kind | Action | In the list? |
|---|---|---|---|---|
| Update available | `/api/update-check`, at each app open [code: `backend/src/api/mod.rs:135`, `frontend/src/routes/+layout.svelte:276`] | condition | Open the update dialog | Yes |
| What's new after an update | Nothing; the update check already fetches the notes [code: `backend/src/meta/github.rs:139`–`141`] | event | Detail: the release notes | Yes, cheap |
| Streaming source not answering | Inferred from the provider's gate (section 4) | condition | Detail: since when, what is known | Yes; the main reason for the feature |
| Announcement (outage, news, a domain move) | Nothing; needs a remote source (section 3) | condition with an expiry | Detail; optional link | Yes, if section 3 is adopted |
| Downloads finished | `markDone` [code: `download/store.svelte.ts:224`] | event | Show in folder | Yes, one row per batch |
| Download failed | `markError` [code: `download/store.svelte.ts:230`] | event | Detail with the reason; open the dock | Yes |
| Tracker sign-in expired, refused or unreachable | Pinned toast [code: `sign-in-toast-copy.ts`] | condition | Reconnect, or retry | Yes; the list becomes its record |
| Progress not saved to a tracker | 4-second error toast [code: `frontend/src/lib/account/busy-notice.ts:76`, `frontend/src/lib/toasts/store.svelte.ts:53`] | event | Open the show | Yes; today it is gone after 4 s |
| Watch Later list could not be refreshed | Nothing; the rail renders its saved snapshot [code: `frontend/src/lib/account/watch-later-refresh.ts:4`] | condition | Retry | Yes, low priority |
| Files removed at startup | Diagnostics page | event | Open diagnostics | Yes, once |
| Donation ask | About page only [code: `frontend/src/routes/about/+page.svelte:106`–`147`] | scheduled event | Open About; not now; never | Yes, under section 5's rules |
| New episode of a show in progress | Nothing; airing rows are already cached for the home rail [code: `backend/src/commands/airing_seed.rs`] | event | Play it | Open question 9 |
| Tracker write held (saving in N s) | Pinned toast with a countdown [code: `busy-notice.ts:60`] | condition, minutes | None | No: it resolves within minutes; stays a toast |
| External player or Syncplay failed to start | `ErrorOverlay` modal [code: `frontend/src/routes/play/[id]/+page.svelte:4959`–`4990`] | event | None; Syncplay's links to its download page | No: it answers the click the user just made |
| ffmpeg missing for a download | `ErrorOverlay` modal [code: `+layout.svelte:722`–`737`] | event | A link to ffmpeg's download page | No, same reason |
| Stream stall and recovery | Toasts during playback | event | None | No: bound to the moment |

## 2. Surface design options

What exists: a topbar holding the breadcrumb (on a page with
somewhere to go back to) and the search form, then the update badge,
the download dock, the account chip and the window controls [code:
`+layout.svelte:587`–`594`, `:684`–`:687`]; a left rail with home,
search, account, settings and diagnostics; toasts in a stack of at
most three rows, where pinned ones count and passing ones give way to
them [code: `toasts/store.svelte.ts:51`, `:63`–`70`].
The dock's popover is the vocabulary to reuse: `--ink-050` surface,
`--radius-card`, 24rem wide, anchored with `inset-inline-end`,
`role="menu"`, and opening it marks every row seen [code:
`frontend/src/lib/components/DownloadDock.svelte:49`–`51`, `:189`,
`:454`–`:468`].

### (a) A list like the downloads

The maintainer's sketch. A bell in the topbar, before the update
badge, opens a popover in the dock's style. Each row has an icon by
kind, one line of text, a time, and at most one action; a chevron where
there is a detail page.

```
 [ search…                    ]  (!)2  ^  [v]  (avatar)  _ □ ×
                                 ┌──────────────────────────────┐
                                 │ Notifications      Mark read │
                                 ├──────────────────────────────┤
                                 │ ● Source not answering       │
                                 │   since 14:02 · 6 plays    › │
                                 │ ● AniList sign-in expired    │
                                 │   [Reconnect]                │
                                 │   3 episodes downloaded      │
                                 │   Frieren · 14:40  [Folder]  │
                                 └──────────────────────────────┘
```

- For: it is the shape the user already knows from the dock, and the
  CSS and the open-marks-seen behavior exist.
- Against: 24rem is too narrow for an announcement or release notes,
  so long notices need somewhere else. On its own it is a place the
  user has to go and look; nothing tells them when something arrives.

### (b) A bell with a side drawer

The bell opens a full-height panel from the inline end, over the page.

```
 [ search…                    ]  (!)2  ^  [v]  (avatar)  _ □ ×
 ┌────┬──────────────────────────────┬──────────────────────────┐
 │home│                              │ Notifications         ×  │
 │srch│    (page, dimmed)            ├──────────────────────────┤
 │acct│                              │ TODAY                    │
 │ set│                              │ ● Streaming source not   │
 │diag│                              │   answering since 14:02  │
 │    │                              │   6 plays have failed.   │
 │    │                              │   It may be down, or     │
 │    │                              │   blocked from here.     │
 │    │                              │ EARLIER                  │
 │    │                              │   3 episodes downloaded  │
 └────┴──────────────────────────────┴──────────────────────────┘
```

- For: room for grouping by day and for a notice's full text inline,
  without leaving the page.
- Against: a new pattern for this app, with its own focus trap,
  dimming and motion, for a list that will usually hold a few rows.

### (c) An inbox page in the rail

A `/notifications` route beside Diagnostics, the list on one side and
the selected notice on the other.

```
 ┌────┬─────────────────────────┬──────────────────────────────┐
 │home│ Notifications           │ Source not answering         │
 │srch│ ● Source not answ.  ›   │ Since 14:02. 6 plays failed, │
 │acct│ ● Sign-in expired   ›   │ the last at 14:31. It may be │
 │ set│   Downloads (3)     ›   │ down, or blocked from here.  │
 │diag│   Update 0.20.1     ›   │                              │
 │[!] │                         │ [Try again]                  │
 └────┴─────────────────────────┴──────────────────────────────┘
```

- For: unlimited room, a real URL for each notice, and simple to make
  accessible.
- Against: reading a notice means navigating away, which is the wrong
  cost during an episode, and the rail entry is easy to ignore.

### (d) Hybrid: toasts announce, the list keeps, a page explains

Toasts stay how a notice arrives. Every notice that earns a place
(section 1) also lands in (a)'s popover, so a dismissed toast is not
lost. A notice with more to say than one line opens a detail route,
`/notifications/<key>`, which is (c)'s right-hand pane on its own.

```
 arrival                    later                   long notice
 ┌──────────────────────┐   (!)2 opens (a)'s list   /notifications/<key>
 │ Streaming source not │   ┌───────────────────┐   ┌────────────────────┐
 │ answering.         › │   │ ● Not answering  ›│──>│ Source not         │
 │                      │   │   Downloads (3)   │   │ answering, 14:02   │
 └──────────────────────┘   └───────────────────┘   │ ...  [Try again]   │
                                                    └────────────────────┘
```

- For: no new pattern; the toast is the existing arrival channel, the
  popover is the dock's, the detail page is an ordinary route.
- Against: three pieces to keep consistent, which the store below has
  to make one source of truth.

### Recommendation: (d)

[proposal] The rest of the design:

- **One store, keyed rows.** Every notice has a stable key, such as
  `outage:hianime`, `signin:anilist`, `download-batch:<id>` or
  `remote:<id>`. Raising a notice with a key that already exists
  updates that row (its count, its time, its text) instead of adding
  one. So fifty failed plays are one outage row with a count, not
  fifty rows.
- **Read and unread.** The bell's count is unread rows. Opening the
  popover marks them read, as the dock does.
- **Retention.** Events persist in `localStorage`, at most 50 rows and
  30 days, oldest dropped first. Conditions are not stored; they are
  recomputed at each start from what raises them, so a stale "sign-in
  expired" cannot outlive the sign-in it described. A condition that
  resolves while the app runs stays as "resolved" for the rest of the
  session, up to a day, so the user can see that the outage they read
  about ended; a restart drops it.
- **Priority.** Three levels: *attention* (outage, failed download,
  sign-in), *info* (finished downloads, what's new, announcements),
  *quiet* (the donation ask). Attention rows sort first and colour the
  bell's count; quiet rows never count.
- **The update badge stays.** The update becomes a row whose detail is
  the update dialog, and the badge keeps its place: the in-app updater
  proposal puts a progress ring on it while an update downloads
  ([`in-app-updater.md`](in-app-updater.md), "Where it shows"). The
  bell does not count the update while the badge shows it. See open
  question 2.
- **The dock keeps live downloads.** Progress, speed and cancel stay
  in the dock; only outcomes go to the list.
- **During playback,** while a video plays in either window, a new
  notice raises no toast except a download failure, which shows in the
  main window as download toasts do today; everything else lands in
  the list and the bell's count. The 0.21.0 mini-player window's own
  compact toast host carries only the play page's stall and recovery
  toasts, never notices.
- **Keyboard and assistive technology.** The bell is a button with
  `aria-expanded` and a label that carries the unread count; the list
  takes the dock's `role="menu"` and its menu keys: arrows, Home and
  End move between rows, and Escape, Tab and Shift+Tab close it with
  focus back on the bell [code: `DownloadDock.svelte:80`–`84`,
  `frontend/src/lib/menu/menu-keys.ts:11`].
- **Right to left.** None of the four shipped locales (en, pt-BR,
  es-419, ru) is right to left [code:
  `frontend/project.inlang/settings.json:4`], and the CSS uses logical
  properties, so the popover anchors with `inset-inline-end` as the
  dock does.
- **Reduced motion.** The duration tokens are already zero under
  `prefers-reduced-motion: reduce` [code:
  `frontend/src/lib/design/tokens.css:206`–`212`]; the bell does not pulse.

### System notifications

Opt-in, off by default, and only for download outcomes and an outage.
They show only while no window of the app is focused, so the 0.21.0
mini-player window ([`mini-player.md`](mini-player.md)) counts as the
app's focus. [proposal]

- **They go through the main process.** The window's permission
  handler denies by default and allows only fullscreen and clipboard
  writing, so the renderer's web `Notification` is refused [code:
  `electron/lib/permissions.cjs:18`]. The renderer sends a small IPC
  message; the handler registers through the IPC guard, which acts
  only on a call from the app's own document [code:
  `electron/lib/ipc-guard.cjs:3`–`5`], and the main process shows an
  Electron `Notification`. Its text comes from the renderer, already
  localized. [proposal]
- **Electron.** `Notification.isSupported()` exists in the main
  process [source:
  <https://www.electronjs.org/docs/latest/api/notification>]. On Linux
  "Notifications are sent using libnotify", for desktops that follow
  the freedesktop specification. [source:
  <https://www.electronjs.org/docs/latest/tutorial/notifications>]
  The main process does not use `Notification` anywhere today; the
  one mention in `electron/` is the end-to-end check that the
  renderer's request is refused. [code: no match in `electron/main.js`
  or `electron/lib/`; `electron/e2e/smoke.spec.ts:1370`]
- **Windows needs an app id.** A notification needs a Start Menu
  shortcut carrying an AppUserModelID and a ToastActivatorCLSID.
  [source: the tutorial above]
  electron-builder's NSIS installer already writes the `appId` into
  its shortcuts (`WinShell::SetLnkAUMI` in its `installer.nsh`)
  [source:
  <https://github.com/electron-userland/electron-builder/blob/master/packages/app-builder-lib/templates/nsis/include/installer.nsh>],
  and the app's `appId` is `net.thirdmovement.ani-gui` [code:
  `electron/package.json:40`]. The app must also call
  `app.setAppUserModelId` with the same id before any window opens
  [source: <https://www.electron.build/docs/nsis>], and
  `electron/main.js` does not today [code: no match]. The installer
  sets no ToastActivatorCLSID [source: the `installer.nsh` above], and
  whether a plain notification shows without one is untested.
  [unverified] So the Windows half is that call, a check on a Windows
  machine, and, if the check fails, the CLSID added to the shortcut.
  The check has to be made by hand: CI builds the installer, but
  nothing installs it or starts the app it installs [code:
  `.github/workflows/packages.yml:30`–`31`, `:153`–`161`], and a
  notification only shows from an installed shortcut.

## 3. Remote notices

The app cannot see a domain move, a planned outage or news. A remote
notice lets the maintainer say those things without a release.

### Options

| Option | Money | Publishing | Privacy | Notes |
|---|---|---|---|---|
| **A signed JSON file in the repository, read from raw.githubusercontent.com** | $0 | Edit, sign, push | IP, time and `User-Agent` reach GitHub's CDN | No API budget used |
| The same file through the GitHub contents API | $0 | Same | Same | Spends the 60-an-hour API budget the update check uses |
| GitHub Pages | $0 | A Pages site and its build | GitHub says it logs visitor IPs | 1 GB site, 100 GB a month soft limit, may answer 429 |
| The release notes | $0 | Only at a release | Nothing new | Not on the fly, which is the point |
| GitHub Discussions | $0 | Read through GraphQL | — | Documented with a token only |
| ntfy.sh | $0, or $5–$20 a month | A push channel | IPs "may be temporarily logged" | Without access control the topic name is the password; messages kept 12 h |
| A Cloudflare Worker | $0 to 100,000 requests a day, then from $5 a month | A server to run | Logs on by default | Error 1027 past the free limit |
| Firebase Cloud Messaging | $0 | Push | Google | Electron cannot receive it |

The facts behind the table:

- **GitHub's API**, unauthenticated, allows 60 requests an hour per IP
  address. [source:
  <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>]
  A 304 answer is exempt from the limit only for an authorized
  request [source:
  <https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api>],
  so an anonymous ETag check presumably still costs one. [inference]
  The releases listing the update check reads answers with
  `x-ratelimit-resource: core` and `x-ratelimit-limit: 60` (checked
  with `curl -I` on 2026-11-02); the update check reads it once per
  app open, and again when a language change reloads the window
  [code: `backend/src/meta/github.rs:163`; `docs/PRIVACY.md:350`–`351`].
- **raw.githubusercontent.com** answers with `cache-control:
  max-age=300`, an ETag, `via: 1.1 varnish`, and no rate-limit headers
  (checked with `curl -I` on 2026-11-02). GitHub's changelog of
  2025-05-08 says unauthenticated raw downloads face new rate limits
  but gives no figure. [source:
  <https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/>]
  So a limit exists and is undocumented; one request per app open is
  unlikely to meet it. [unverified: the actual limit]
- **GitHub Pages**: 1 GB per site, a soft 100 GB a month of
  bandwidth, and it may answer 429 [source:
  <https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>];
  "the visitor's IP address is logged and stored for security
  purposes" [source:
  <https://docs.github.com/en/pages/getting-started-with-github-pages/about-github-pages>].
- **GitHub Discussions** is reachable only through GraphQL, whose
  guide describes calls made with a token. [source:
  <https://docs.github.com/en/graphql/guides/forming-calls-with-graphql>]
  Whether an anonymous call is refused was not tested: the test
  machine's API budget was spent. [unverified]
- **ntfy.sh**: messages up to 4,096 bytes and a 12-hour cache [source:
  <https://docs.ntfy.sh/publish/>]; paid plans at $5, $10 and $20 a
  month [source: <https://ntfy.sh/#pricing>]; without access control
  "the topic name is your password" [source:
  <https://docs.ntfy.sh/faq/>], so anyone who reads the app's source
  could publish to its topic, and a signature would still be needed;
  IP addresses "may be temporarily logged" [source:
  <https://docs.ntfy.sh/privacy/>].
- **Cloudflare Workers**: the free plan allows 100,000 requests a day
  and 10 ms of CPU per call; the paid plan starts at $5 a month
  [source: <https://developers.cloudflare.com/workers/platform/pricing/>];
  past the free limit, a Worker set to fail closed answers error 1027
  [source:
  <https://developers.cloudflare.com/workers/platform/limits/>];
  Workers Logs is on by default and kept 3 days on the free plan
  [source:
  <https://developers.cloudflare.com/workers/observability/logs/workers-logs/>].
- **Firebase Cloud Messaging** is free [source:
  <https://firebase.google.com/pricing>], and web push needs a service
  worker that can show a notification, which Electron does not
  support: the issue is open, and a maintainer said in 2020 there were
  "no plans to work on it in the immediate future". [source:
  <https://github.com/electron/electron/issues/13041>]
- **Precedents.** Signal Android fetches a JSON file of announcements
  every three days, filters it by app version and country, carries
  per-locale text, and checks it only by MD5. [source:
  `RetrieveRemoteAnnouncementsJob.kt` in signalapp/Signal-Android]
  Firefox Remote Settings signs its collections and pins the signing
  chain to a root. [source:
  <https://remote-settings.readthedocs.io/en/latest/client-specifications.html>]

### Recommendation: a signed file read from raw.githubusercontent.com

[proposal]

- **Where.** `notices.json` and its signature `notices.json.minisig`
  on a `notices` branch of this repository. A branch of its own
  because `master` is under a ruleset that requires an up-to-date pull
  request, which would put a review and CI between an outage and its
  notice; the signature, not the branch, is what the app trusts.
  The `master` ruleset covers the default branch alone, and the remote
  flags are already read this way, from a `flags` branch whose own
  ruleset only blocks deletion and force-pushes (both checked with the
  GitHub rulesets API on 2026-11-24) [code:
  `backend/src/flags/remote.rs:20`]. A `notices` branch would follow
  that precedent.
- **Trust.** minisign: Ed25519 over a BLAKE2b-512 hash, a public key
  of one line. [source: <https://jedisct1.github.io/minisign/>] The
  backend already verifies a file this way: the remote flags file is
  checked with the `minisign-verify` crate (MIT, no dependencies)
  [source: <https://crates.io/crates/minisign-verify>] against a list
  of public keys built into the app, any of which will do, so a
  rotation ships the next key a release ahead [code:
  `backend/Cargo.toml:85`, `backend/src/flags/signature.rs:1`–`10`].
  The in-app updater proposal recommends the same for releases.
  Notices reuse that verifier with a list of keys of their own: the
  notice key is used often, and must never be the key that can replace
  the app ([`in-app-updater.md`](in-app-updater.md), section 4 and its
  open question 2), nor the one that can turn a feature on for every
  install. Signing follows the flags file's steps with the minisign
  tool (`docs/development.md`, "Signing the flags file").
- **No replay.** The signature's trusted comment carries an issue
  time. The backend keeps the newest it has accepted and refuses an
  older file, so an old signed file cannot be served again to bring
  back a notice.
- **When it is read.** At launch, beside the update check and the
  remote flags, and once
  more when the outage condition of section 4 rises, which is when an
  explanation is most useful. No timer. The backend reads it with
  `If-None-Match`; any failure (offline, 404, a bad signature, a 429)
  shows nothing and is logged, as the update check does.
- **What a notice holds.** An id, a kind (`outage`, `news`,
  `domain-move`), a priority, a start and an expiry, optional version
  and platform ranges, an optional link restricted to
  `https://github.com/JoaoPucci/ani-gui/`, and per-locale title and
  body with English as the fallback. Plain text only; the renderer
  draws paragraphs and the one link, and no HTML.
- **Text from the backend.** The backend never returns its own
  localized strings (`AGENTS.md` section 4). Notices are the
  maintainer's text relayed as data, the same shape as the release
  notes body the update check already relays [code:
  `backend/src/meta/github.rs:139`–`141`]. See open question 7.
- **Targeting** stays coarse: version range and platform. Nothing
  about the user is sent to choose what they see; every install reads
  the same file and filters it locally.
- **An outage notice and the detected outage share a key**
  (`outage:<provider>`), so the remote text explains the detected row
  instead of adding a second one.
- **A switch.** "Show announcements from the ani-gui project" in
  Settings, on by default (open question 4). Off means no request.
  It is separate from the update check's switch that the in-app
  updater proposal adds, so turning either off leaves the other.
- **Cost.** $0 and no GitHub API budget. Per notice: edit the file,
  sign it, push the branch. The CDN may serve a copy up to five
  minutes old.

### Privacy

`docs/PRIVACY.md` says the app collects no analytics or telemetry,
and that what leaves the computer is the requests it makes to public
services to do its job [code: `docs/PRIVACY.md:9`–`15`]. Two of those
are automatic at every launch and go to the project's own repository:
the update check, to GitHub's API, and the remote flags file with its
signature, two requests to GitHub's content server [code:
`docs/PRIVACY.md:348`, `:375`–`391`, `:408`–`411`;
`backend/src/flags/launch.rs`, `backend/src/flags/remote.rs:20`]. A
notice read is one more, to that same content server. It sends no
account, install id or version, but it reveals the user's IP address,
the time and a `User-Agent: ani-gui` header, which the flags requests
already reveal to that server at the same moment. Neither the update
check nor the flags fetch can be turned off today [code:
`docs/PRIVACY.md:369`; the flags fetch has no setting], and the
updater's proposed switch would stop only the update check. So the
notices switch stops the notice read and nothing else: with every
switch off, the app still fetches the flags file and its signature at
launch. `docs/PRIVACY.md` gets an entry for the notice read naming
its switch, its summary at the top names the automatic requests, and
the sentence that ends its list of requests, which names the update
check and the flags file as the only requests to the project's own
repository, adds the notices.

## 4. Detecting an outage

The app already has the signal; it does not show it.

- **The gate.** Each provider has its own [code:
  `backend/src/provider_set.rs:80`]. Its breaker opens after 3
  consecutive failures and stays open for 60 s; then one trial is let
  through, and one failure reopens it [code:
  `backend/src/scraper/gate.rs:47`, `:51`]. A click always passes the
  gate [code: `gate.rs:14`–`17`]. The gate already answers whether the
  provider has answered since it last failed [code: `gate.rs:313`].
  The renderer cannot see any of it.
- **What feeds it.** A play's outcome is mapped before it reaches the
  gate: transport errors, 403, 5xx and pages the client cannot read
  count as failures; a 429 or an advertised rate limit opens a pause
  window at once instead of counting towards the breaker; a 404 and
  other answers that are "not here" count as the provider answering
  [code: `backend/src/commands/play_native_outcome.rs:55`–`77`,
  `gate.rs:335`]. (The header comment at `gate.rs:18`–`19` still
  lists 429 among breaker failures; the mapping is what runs.)

The rule [proposal]:

1. The backend exposes per provider: whether the breaker is open, when
   the current failure run started, how many failures it holds,
   whether a click failed during it, and whether a pause window is
   running. The renderer reads it from the loopback API after every
   play, failed or not, and when the window gains focus. **No request
   is made to the provider to find out**; the run is built only from
   traffic the app already sends.
2. The row "Streaming source not answering" appears when, without a
   success between, the breaker has opened at least twice or a pause
   window has been reopened, and at least one click failed in that
   run. One flaky minute, or only background probes failing, raise
   nothing.
3. **Is it the user's network?** If the metadata service (Kitsu) has
   also failed with a transport error in the same window, the row says
   the app cannot reach the internet instead, and blames no one.
4. **Wording.** "Not answering", not "down", and never "nothing is
   wrong on your end": an ISP block and a real outage look the same
   from the app, and only the first is the user's to work around. The
   detail page says both are possible.
5. **It resolves itself.** The first success through the gate marks
   the row resolved ("back since 15:10"), and it leaves the list a day
   later.
6. A failed play keeps its own error text as today
   (`source-answer.ts`); the row adds "this has happened 6 times since
   14:02", which no single play can say.

Whether a dead embed host, as opposed to the provider's own pages,
feeds the gate decides whether it reads as an outage; the phase that
builds this settles it with a test. [unverified]

## 5. Asking for donations

Today the ask is the About page's "Support" section: an Ethereum
address and a copy button. [code: `frontend/messages/en/about.json:7`,
`frontend/src/routes/about/+page.svelte:106`–`147`]

Precedents:

- **KDE Plasma 6.2** shows one donation notification a year, in
  December. Any interaction with it dismisses it until the next year,
  it can be turned off, and distributors can turn it off. [source:
  <https://pointieststick.com/2024/08/28/asking-for-donations-in-plasma/>,
  reported by <https://lwn.net/Articles/987715/>] On its first day
  KDE "received double the prior two months' Paypal donations", and
  the author found one non-positive comment about it. [source:
  <https://pointieststick.com/2024/12/02/i-think-the-donation-notification-works/>]
- **Thunderbird**, in a proposal page (not necessarily what shipped):
  a tab on a major update, not again for about 42 weeks, with
  "donate", "ask later" and "stop asking", and a hidden preference to
  suppress it. [source:
  <https://wiki.mozilla.org/User:RKent/Donations_Proposal>]

The rules [proposal]:

- At most once in 12 months.
- Never on the first launch, and never in the first 14 days after it.
- Shown on the first launch after the app updated, when the user has
  just received something, and only while no video plays in either
  window; otherwise it waits for the next launch.
- One toast and one quiet row. No count on the bell, no pulse, no
  system notification.
- Any interaction (Donate, Not now, closing it) dismisses it for the
  12 months. "Don't ask again" ends it, and a Settings switch does the
  same.
- Donate opens the About page's Support section; the ask carries no
  address of its own.

The first-launch date and the last ask live in `localStorage`; losing
them restarts the 14 days, which errs towards not asking.

## 6. Plan

Each phase is a pull request on its own. Effort as in the in-app
updater proposal: **S** is a day or less, **M** a few days, **L** a
week or more.

0. **Mockups in the real app** of the bell, the popover, a toast, the
   detail page and the outage row, reviewed by the maintainer before
   anything is built, together with the updater's dialog states. S.
1. **The store and the surface.** The keyed store, retention, the
   bell, the popover and the detail route; downloads, sign-ins,
   progress-not-saved and the startup sweep moved onto it; strings in
   all four locales. The store and its rules are a `$lib` module with
   `vitest` tests, and an acceptance test covers the flow. M.
2. **Outage detection.** The backend's per-provider state on the
   loopback API, `cargo test` and a `proptest` for the pure rule of
   section 4, and the row. M.
3. **System notifications.** The main-process IPC handler with the
   sender check, `app.setAppUserModelId`, the setting, tests in
   `electron/lib/*.test.cjs`, and a check on Windows. S–M.
4. **Remote notices.** The notice key, signing as the flags file is
   signed, the backend's read, verification with the flags' verifier
   and the filter (`wiremock` tests, a `proptest` for the filter), the
   setting, and the `docs/PRIVACY.md` entry. M.
5. **The donation ask.** The schedule as a pure function with tests,
   the toast, the row and the setting. S.
6. **The update and what's new.** The update row, opening the existing
   dialog, and the release notes as a detail page after an update. S.
7. **New episodes**, if the maintainer wants them (open question 9):
   a row per show in progress whose next episode has aired, computed
   from the airing rows the home rail already caches. S–M.

## 7. Open questions for the maintainer

Each with a recommended answer.

1. **Which surface?** Recommended: (d), the hybrid.
2. **Does the update badge stay?** Recommended: yes, beside the bell,
   because the updater puts its progress on it; the update is also a
   row.
3. **System notifications on by default?** Recommended: off, with a
   switch in Settings.
4. **Remote notices on by default?** Recommended: on, with a switch
   and a `docs/PRIVACY.md` entry; the request reveals to GitHub what
   the remote flags fetch already reveals at the same moment.
5. **When is the feed read?** Recommended: at launch and when an
   outage is detected, never on a timer.
6. **Where does it live?** Recommended: a `notices` branch here, out
   of the `master` ruleset, trusted by its signature.
7. **May the backend relay the maintainer's per-locale text?**
   Recommended: yes, as data, like the release notes body; the
   renderer still owns every string of its own.
8. **How often to ask for donations?** Recommended: at most once a
   year, on the first launch after an update, never in the first 14
   days.
9. **"New episode" notices?** Recommended: yes, as phase 7, computed
   only from the airing rows the home rail already caches, never from
   a new request.
10. **Retention?** Recommended: 50 rows or 30 days, whichever comes
    first.

## Related work

- [An in-app updater](in-app-updater.md), proposed for the same
  release: the update notice's detail is its dialog, and it shares the
  minisign verifier under a separate key.
- The 0.18.0 notifier change, which offers only releases with a
  download for the user's platform. The update row inherits it.
- The window's permission handler, which denies every web permission
  but fullscreen and clipboard writing, and the IPC guard, which
  checks who sends each call; they are why system notifications go
  through the main process.
- [A mini-player window](mini-player.md), proposed for 0.21.0: its
  focus counts as the app's, download outcomes keep showing in the
  main window, and its own toast host shows no notices.
- [`deferred-work.md`](../deferred-work.md#following-a-providers-legitimate-domain-move),
  "Following a provider's legitimate domain move": a signed notice can
  tell users why the source stopped answering until a release follows
  the move.
