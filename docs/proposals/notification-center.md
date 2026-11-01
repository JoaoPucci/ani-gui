# Proposal: a notification center

**Status**: draft, proposed as the main feature of the 0.20.0 release.
Nothing below is built. It supersedes the "A notification center" entry
in [`deferred-work.md`](../deferred-work.md). The research was done on
2026-11-01 and 2026-11-02; service prices and limits quoted below are
as their pages stated them on those days and will drift.

TODO(research): this draft is unfinished. Sections marked
`TODO(research):` still need writing or checking before review.

Labels used below: **[code]** is something read in this repository;
**[source]** is a fact taken from the external page cited next to it;
**[unverified]** is a claim no source was checked for; **[proposal]**
is a design choice.

## The problem in plain terms

The app already tells the user several things, but each in its own
place and each gone once the moment passes:

1. **An update** is a topbar badge with a glowing dot, and a dialog
   behind it. It lasts as long as a newer release exists. [code:
   `frontend/src/lib/components/UpdateBadge.svelte`,
   `frontend/src/lib/update/store.svelte.ts`]
2. **A finished or failed download** is a row in the download dock.
   The rows live in memory, so a restart forgets them. [code:
   `frontend/src/lib/download/store.svelte.ts`, which has no storage]
3. **A tracker sign-in that needs attention** is a toast that stays
   until it is dismissed, and then only the amber dot on the account
   chip is left. [code: `frontend/src/routes/+layout.svelte`,
   `frontend/src/lib/account/sign-in-toast-copy.ts`,
   `frontend/src/lib/components/AccountChip.svelte`]
4. **A file the app deleted for the user at startup** is a section on
   the diagnostics page, which few people open. [code:
   `frontend/src/lib/diagnostics/legacy-sweep.ts`]
5. **An outage of the streaming source** has no surface at all. Each
   failed play shows its own error ("The streaming source is down or
   under maintenance right now…" for an upstream 5xx), and the
   background probes stop quietly when the provider's circuit breaker
   opens. Nothing says "the source is down" once, for everyone, and
   nothing says it before the user presses Play. [code:
   `frontend/src/lib/play/source-answer.ts`,
   `backend/src/scraper/gate.rs`]

The maintainer also wants a place for things the app cannot see by
itself: an announcement of an outage or of news, and a respectful
donation ask.

## 1. Inventory

Each notice is classified as a **condition** (true for a while, then
resolved by itself: an update exists, a sign-in has expired, the source
is failing) or an **event** (happened once: a download finished). The
difference decides persistence and dismissal (section 2). [proposal]

| Notice | Source today | Kind | Urgency | Action it offers | Earns a place? |
|---|---|---|---|---|---|
| Update available | GitHub releases, checked once at launch through `/api/update-check` [code: `backend/src/meta/github.rs`] | condition | attention | Open the releases page; "what's new" detail | Yes |
| Streaming source failing | Inferred from the provider gate's breaker (section 4); today per-play error copy only | condition | attention | Detail: what is known, when it started; "Try again" | Yes, the reason the feature exists |
| Announcement (outage, news) | None; needs a remote source (section 3) | event or condition with expiry | info or attention | Detail page; optional link | Yes, if section 3's source is adopted |
| Download finished | Download store, `markDone` [code] | event | info | Show in folder | Yes, grouped per batch |
| Download failed | Download store, `markError`; ffmpeg missing is a modal [code: `+layout.svelte`] | event | attention | Detail with the reason; open downloads | Yes |
| Tracker sign-in expired / refused / unreachable | `ExpiryToastTracker` pinned toast [code] | condition | attention | Reconnect, or retry the renewal | Yes; replaces the pinned toast as the record |
| Tracker write held (busy, saving in N s) | `noticeBusy` pinned toast with a countdown [code: `frontend/src/lib/account/busy-notice.ts`] | condition, short | info | None; resolves itself | No: lasts at most minutes; stays a toast |
| Progress not saved to a tracker | `reportProgressNotSaved` error toast [code] | event | attention | Open the show's list entry | Yes: today it vanishes after 4 s |
| Watch Later list could not be refreshed | Nothing; the rail keeps its saved copy silently (deferred-work entry) | condition | info | Retry | Yes, low priority |
| Files removed at startup (legacy sweep) | Diagnostics page section [code] | event | info | Open diagnostics | Yes, once |
| What's new after an update | Nothing | event | info | Detail: the release notes | Candidate; the notes body is already fetched by the update check |
| New episode of a show in progress | Nothing; airing rows already cached by the home rail's seed [code: `backend/src/commands/airing_seed.rs`] | event | info | Play the episode | Candidate; must be computed from rows the app already holds, with no extra requests |
| Provider moved to another domain | Treated as an outage until a release (architecture, "Providers and failover") | condition | attention | None the app can take; "an update will fix this" | Only through a remote announcement |
| External player / Syncplay hand-off failed | `ErrorOverlay` modal [code: play page] | event | blocking | Settings | No: it answers a click the user just made; the modal stays |
| Stream stall recovery | Toasts during playback [code] | event | info | None | No: moment-bound |
| Cache or disk problems | Generic `error.io.generic` / `error.cache.generic` on whatever surface hit them; no free-space check exists [code] | condition | attention | Open settings / diagnostics | Later: needs a backend signal first |
| Donation ask | About page only [code: `frontend/src/routes/about/+page.svelte`] | event, scheduled | quiet | Open About; not now; never | Yes, under section 5's rules |

TODO(research): confirm the ranking with the maintainer; check the
AniList-throttled state ("When AniList cannot answer" in
`architecture.md`) as a further candidate.

## 2. Surface design options

The existing chrome: a topbar holding the search form, then the update
badge, the download dock, the account chip and the window controls, in
that order [code: `+layout.svelte`]; a 5.5rem left rail; toasts
bottom-right above the download bar. The dock's popover is the
vocabulary to reuse: `--ink-050` surface, `--ink-300` border,
`--radius-card`, 24rem wide, anchored with `inset-inline-end`.
[code: `DownloadDock.svelte`, `frontend/src/lib/design/tokens.css`]

### (a) A list like the downloads (the maintainer's sketch)

A bell in the topbar, left of the update badge, opens a popover list in
the dock's style. Each row: an icon by kind, one line of text, a time,
and one action button; a chevron where there is a detail page.

```
 [ search…                     ]  🔔•  ⇣  (avatar)  _ □ ×
                                   ┌──────────────────────────────┐
                                   │ Notifications      Mark read │
                                   ├──────────────────────────────┤
                                   │ ● Streaming source failing   │
                                   │   since 14:02 · 6 plays  [›] │
                                   │ ● AniList sign-in expired    │
                                   │   [Reconnect]                │
                                   │   Update 0.20.1 available [›]│
                                   │   3 episodes downloaded      │
                                   │   Frieren · 14:40 [Folder]   │
                                   └──────────────────────────────┘
```

TODO(research): tradeoffs, detail-page placement (inside the popover
versus a route), read/unread, retention, keyboard, RTL.

### (b) A bell with a side drawer

TODO(research): drawer from the inline end, full height; room for
grouping and inline detail; a new pattern for this app.

### (c) An inbox page in the rail

TODO(research): `/notifications` route beside Diagnostics; master and
detail; costs a navigation away from the player.

### (d) Hybrid: toasts arrive, the list keeps

TODO(research): leaning recommendation. Toasts stay the arrival
channel; every notice of attention or info level lands in (a)'s list
with stable keys and upsert semantics (one outage row with a count,
not fifty failures); long notices open a detail route. The download
dock keeps live progress; the update badge folds into the bell (open
question). OS notifications through Electron opt-in, only while the
window is unfocused, only for download outcomes and an outage.

Facts gathered for this section:

- Electron: "Notifications can be displayed directly from the renderer
  process with the web Notifications API"; on Linux "Notifications are
  sent using libnotify" and work on desktops following the freedesktop
  Desktop Notifications Specification; on Windows the app needs a
  Start Menu shortcut with an AppUserModelID and a ToastActivatorCLSID.
  [source: <https://www.electronjs.org/docs/latest/tutorial/notifications>]
  `Notification.isSupported()` exists in the main process.
  [source: <https://www.electronjs.org/docs/latest/api/notification>]
- `electron/package.json` sets `appId` to `net.thirdmovement.ani-gui`
  and builds NSIS on Windows. [code] Whether the NSIS install's
  shortcut carries that id as its AppUserModelID is
  TODO(research): check electron-builder's NSIS docs.
- Reduced motion: `tokens.css` sets the duration tokens to 0 under
  `prefers-reduced-motion: reduce`. [code]
- Four locales ship (en, pt-BR, es-419, ru), none RTL; the CSS uses
  logical properties. [code: `docs/i18n.md`]

## 3. Remote, on-the-fly notifications

TODO(research): write the comparison and the recommendation. Facts
gathered (all fetched 2026-11-01):

- **GitHub REST, unauthenticated**: 60 requests an hour per IP
  address. [source:
  <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>]
  A 304 from a conditional request is free only for authorized
  requests, so an unauthenticated ETag check still costs one of the
  60. [source:
  <https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api>]
  The releases listing the update check reads reports
  `x-ratelimit-resource: core`, the same budget. The contents API
  serves files up to 1 MB with all media types. [source:
  <https://docs.github.com/en/rest/repos/contents>]
- **raw.githubusercontent.com**: served from a CDN with
  `cache-control: max-age=300` and an ETag, no `x-ratelimit-*`
  headers (checked with `curl -I`). GitHub's 2025-05-08 changelog
  lists raw downloads among unauthenticated traffic under new rate
  limits but publishes no figure. [source:
  <https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/>]
- **GitHub Pages**: free for public repositories on GitHub Free; 1 GB
  site, soft 100 GB a month bandwidth, may answer 429. [source:
  <https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>]
  "the visitor's IP address is logged and stored for security
  purposes". [source:
  <https://docs.github.com/en/pages/getting-started-with-github-pages/about-github-pages>]
  Served with `cache-control: max-age=600`.
- **GitHub Discussions**: only through GraphQL, whose docs describe
  authenticated access only; a third party reports a 401 for
  unauthenticated calls. [partly unverified]
- **Signed feed**: minisign signs with Ed25519 (BLAKE2b-512 prehash by
  default); a public key is one base64 line. [source:
  <https://jedisct1.github.io/minisign/>] The Rust crate
  `minisign-verify` 0.3.0 (MIT, no dependencies). [source: crates.io]
  Tauri's updater requires minisign signatures and cannot turn them
  off. [source: <https://v2.tauri.app/plugin/updater/>] Precedent:
  Firefox Remote Settings signs collections and pins a root. [source:
  <https://remote-settings.readthedocs.io/en/latest/client-specifications.html>]
  Signal Android fetches a release-notes/megaphone JSON every three
  days, filtered by version and country, with per-locale text, checked
  by MD5 only. [source: `RetrieveRemoteAnnouncementsJob.kt` in
  signalapp/Signal-Android]
- **ntfy.sh**: free public server, 250 messages a day, 4,096-byte
  messages, 12-hour message cache, topics public ("the topic is
  essentially a password"); paid from $5 a month. [source:
  <https://docs.ntfy.sh/publish/>, <https://ntfy.sh/#pricing>]
  IP addresses used for rate limiting and "may be temporarily
  logged". [source: <https://docs.ntfy.sh/privacy/>]
- **Firebase Cloud Messaging**: free [source:
  <https://firebase.google.com/pricing>], but Electron does not
  support persistent notifications or service-worker push (electron
  issue #13041, open). [source:
  <https://github.com/electron/electron/issues/13041>]
- **Cloudflare Workers free plan**: 100,000 requests a day, 10 ms CPU
  per invocation; paid from $5 a month. [source:
  <https://developers.cloudflare.com/workers/platform/pricing/>]
  Workers Logs on by default, kept 3 days on the free plan. [source:
  <https://developers.cloudflare.com/workers/observability/logs/workers-logs/>]

Leaning recommendation: a minisign-signed JSON file in this repository,
read from raw.githubusercontent.com by the backend at most once a day
and at launch, with version and platform targeting, an expiry, and
per-locale text with an English fallback; costs nothing in money and
no API budget. TODO(research): finish the privacy statement (what the
request reveals: IP and timing, as the update check already does) and
the PRIVACY.md change it needs.

## 4. Automatic outage detection

TODO(research): build on the provider gate's breaker (three
consecutive failures open it for 60 s; `backend/src/scraper/gate.rs`)
exposed to the renderer as a condition; require persistence across
several breaker cycles and an interactive failure before showing;
never add a request to the provider to detect it; tell "your network
is down" (Kitsu also failing) apart from "the source is down".

## 5. Donation asks

TODO(research): rules. Precedent: KDE Plasma 6.2 shows one donation
notification a year, in December; any interaction dismisses it until
next year, and it can be turned off. [source:
<https://pointieststick.com/2024/08/28/asking-for-donations-in-plasma/>,
as quoted by <https://lwn.net/Articles/987715/>]

## 6. Phasing and open questions

TODO(research): phase 0 is mockups in the real app, reviewed by the
maintainer before anything is built.

## Related work

A separate change in progress makes the update notifier offer only
releases that carry a download for the user's platform. It is not part
of this proposal.
