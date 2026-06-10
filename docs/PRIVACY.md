# Privacy Policy

_Last updated: 2026-06-10_

This document explains how ani-gui handles your data. It applies to
the open-source ani-gui desktop application maintained at
<https://github.com/JoaoPucci/ani-gui>.

The plain version: **ani-gui runs entirely on your machine. We do
not operate a server. We do not collect telemetry. The only data
that leaves your computer is what you explicitly send to public
services (your anime tracker, the anime catalogue, the video host).**

If you have not connected an account (AniList or MyAnimeList),
ani-gui never sends any data identifying you anywhere.

## What ani-gui stores locally

ani-gui keeps the following on your computer only:

- **Settings** — your playback preferences, locale, external-player
  configuration, etc. Stored as plain TOML under your OS's
  configuration directory (`$XDG_CONFIG_HOME/ani-gui/config.toml` on
  Linux, equivalent paths on macOS and Windows).
- **Metadata cache** — anime details, episode lists, thumbnails, and
  similar information fetched from public APIs. Stored in a local
  SQLite database under your OS's cache directory.
- **Watch history** — the same `ani-cli` history file the underlying
  CLI uses (`$XDG_STATE_HOME/ani-cli/ani-hsts`). Lists what you've
  watched and where you left off.
- **OAuth tokens** — if you connect an account (see below).
  Encrypted via your operating system's keychain (libsecret on Linux,
  Keychain on macOS, DPAPI on Windows) through Electron's
  `safeStorage` API, then written to your OS user-data directory.
- **Tracker list cache** — if you connect an account, a snapshot of
  your AniList / MyAnimeList list is cached in the same local SQLite
  database so the Watch Later rail and your progress render without a
  round-trip on every launch. Each row holds which provider it came
  from and your user id on that provider, the show's identifiers (the
  provider-native id plus its cross-provider MyAnimeList id), your
  status, episodes watched, score, and title, and two timestamps (the
  provider's last-updated time and when the row was cached). Cleared
  when you disconnect the account.

## What ani-gui transmits to public services

ani-gui only contacts external services when you take an action that
requires it. For each kind of request:

- **Anime catalogue lookups** — Kitsu, AniList, MyAnimeList (the last
  only if connected), and the underlying allmanga / allanime catalogue
  used by `ani-cli`. These requests carry the search terms you typed
  or the anime IDs you're browsing; they do not carry any account
  identifier unless you've connected one.
- **Video playback** — the chosen episode URL is fetched directly
  from its source CDN. The CDN sees a normal `Referer` matching the
  catalogue origin so it serves the file.
- **Tracker integration (optional)** — only if you sign in to AniList
  or MyAnimeList:
  - Your OAuth bearer token is sent to that provider's API on every
    request that fetches your list or updates your progress.
  - We send your watch progress (episodes watched, status, score)
    when you mark an episode watched.
  - We never send tracker tokens or progress to any third party.
  - The provider's own privacy policy governs how it handles the
    requests:
    - AniList: <https://anilist.co/terms>
    - MyAnimeList: <https://myanimelist.net/about/privacy_policy>
- **Update check** — by default ani-gui checks GitHub's public
  releases API to notify you when a new version is published. No
  account information is sent; this can be disabled in settings.
- **Aniskip OP/ED timing** — when the auto-skip toggle is on, ani-gui
  asks the Aniskip community service for crowd-sourced opening /
  ending timestamps for the episode you're watching. No account
  information is sent.

There is no analytics or telemetry. ani-gui does not "phone home" on
launch or on any other schedule.

## Connecting and disconnecting an account

Connecting AniList (and, in a future release, MyAnimeList) uses the
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

Issues and questions about this policy are tracked publicly at
<https://github.com/JoaoPucci/ani-gui/issues>.
