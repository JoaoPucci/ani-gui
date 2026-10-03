<h1 align="center">ani-gui</h1>

<p align="center">
  <em>A desktop app for browsing and watching anime.</em>
</p>

<p align="center">
  <img width="2751" height="1300" alt="home image" src="https://github.com/user-attachments/assets/ee2e3d80-01e8-46cb-afa0-a132cd3e3273" />
</p>

ani-gui is a Rust + SvelteKit desktop application for browsing and watching anime — discovery, search, an embedded player, downloads, persistent watch history, Picture-in-Picture, and OP/ED skip. It began as a graphical front end over [pystardust/ani-cli](https://github.com/pystardust/ani-cli) and resolves streams itself now, in Rust.

See [`docs/architecture.md`](./docs/architecture.md) for the full picture.

## Features

|  | |
|---|---|
| **Discovery** | Trending and Top Rated rails, a rotating hero — AniList + Kitsu. |
| **Search** | Full-text against Kitsu, instant as you type. |
| **Detail page** | Synopsis, episodes with thumbnails, similar-titles strip. |
| **Embedded player** | HLS / MP4, quality switch, native or custom controls — no `mpv` window. |
| **Subtitles** | Tracks inside the stream's playlist, and the sidecar `.vtt` tracks a provider lists beside it — served through the local proxy, written beside downloads, handed to external players. |
| **OP / ED skip** | aniskip intervals — one-click or fully automatic. |
| **Picture-in-Picture** | Pop the player out while it is open. |
| **Background prefetch** | Adjacent episodes warm in advance. |
| **Downloads** | Per-episode or ranged, progress dock. yt-dlp bundled; ffmpeg sourced per platform (apt `Recommends:` on `.deb`, installer-time fetch on Windows, system PATH on AppImage). |
| **Watch history** | Continue Watching — and a show's Play button — takes you back to the last episode you watched when the app kept the point you left it at, and resumes there; otherwise it goes on to the next episode. A point is kept for an episode left between its first 15 seconds and its last 90, for the 200 most recently kept episodes. Remove a single card or clear the lot from the rail. |
| **External player** | One click to mpv / VLC / IINA / custom. |
| **Watch together** | Hand the current stream to [Syncplay](https://syncplay.pl/) for a watch party. |
| **Trackers** | Connect AniList or MyAnimeList — a Watch Later rail on the home page, and your progress synced back automatically as you watch. |
| **Localised** | English, Brazilian Portuguese, Latin American Spanish, Russian. |
| **No telemetry** | No analytics or tracking. Outbound traffic is metadata, the stream you picked, a launch-time check of the app's own GitHub releases, and — with an account connected — your tracker's list + progress sync. Localhost-only listener on a kernel-assigned port. See the [privacy policy](docs/PRIVACY.md). |

## Install

ani-gui is distributed as a desktop bundle. The transport it uses to reach the stream providers ships inside it, so browsing and playback need nothing else. Downloads additionally want `ffmpeg`, which each platform sources differently — see the tier notes below.

Platform support tiers:

| Tier | Platform | Status |
|---|---|---|
| 1 | Linux | Actively tested on Ubuntu. Other distros work via AppImage. |
| 2 | Windows | Most features verified end-to-end. Edge cases may surface. |
| — | macOS | Not packaged. No installer is built or shipped; the dev loop runs from source. |

<details>
<summary><strong>Linux</strong> — tier 1 (tested on Ubuntu)</summary>

- **AppImage** — download from the [releases page](https://github.com/JoaoPucci/ani-gui/releases), `chmod +x`, double-click. Install `ffmpeg` from your distro if you want downloads: the AppImage carries yt-dlp but not ffmpeg, and yt-dlp hands off to it whenever a stream arrives as MPEG-TS and has to be repackaged as MP4. Playback itself needs neither. The bundle launches with Chromium's setuid sandbox disabled (AppImage's read-only FUSE mount can't carry the SUID bit `chrome-sandbox` requires); the localhost-only architecture means the sandbox isn't load-bearing for the threat model. If you'd rather keep the sandbox, install the `.deb` instead.
- **Debian / Ubuntu (`.deb`)** — `sudo apt install ./ani-gui_<version>_amd64.deb`. apt pulls in the recommended `ffmpeg` package (needed for the download feature) along the way; the post-install script sets the `chrome-sandbox` SUID bit Electron needs, so the sandbox stays on. `sudo dpkg -i …` still works but won't auto-install ffmpeg — drop into `apt --fix-broken install` or run `sudo apt install ffmpeg` separately if you used dpkg directly.

</details>

<details>
<summary><strong>Windows</strong> — tier 2 (most functions tested)</summary>

NSIS installer (`.exe`). Run it; it installs per-user by default and creates Start menu and desktop shortcuts.

The installer will fetch ffmpeg automatically the first time it runs (~80 MB) so the download feature works out of the box; the impersonating transport and yt-dlp are bundled directly. The ffmpeg fetch runs even when you already have ffmpeg installed via a per-user package manager (scoop, winget user-scope) — the installer's elevated context doesn't see per-user PATH entries, and the bundled copy is what the app uses at runtime in either case.

</details>

## Build from source

Tested on Linux and Windows. The dev loop (steps 5–6) runs on both: the Electron `dev` script is a Node launcher with no shell-dialect syntax, and on Windows it stages the bundled tools itself, as step 6 notes. On macOS the dev loop launches and browses metadata, but playback needs an impersonating transport and no fetcher stages one there. The packaging scripts (step 7) build per-host artifacts — run on x86_64 Linux for `.AppImage` / `.deb`, on x64 Windows for the NSIS installer. There is no macOS packaging target yet.

1. **Install Rust** (toolchain pinned by `rust-toolchain.toml`). Linux / macOS:
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   . "$HOME/.cargo/env"   # (or re-open the shell) so `cargo` is on PATH
   ```
   Windows: download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs) — it sets up the MSVC toolchain and offers to install the Visual Studio build tools it needs.
2. **Install Node 20–24 and enable corepack.** The repository pins its pnpm version in `package.json`, so `corepack enable` is the whole pnpm setup — corepack fetches the pinned version on first use. Node 25+ no longer bundles corepack; on those, install it first (`npm install -g corepack`) and the rest is unchanged. Linux / macOS (via nvm — skip the curl step if you already have nvm or installed Node another way):
   ```sh
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
   # re-open the shell (or `source ~/.bashrc`) so nvm is on PATH
   nvm install 20
   corepack enable
   ```
   Windows: install Node from [nodejs.org](https://nodejs.org) (or `winget install OpenJS.NodeJS.LTS`), then run `corepack enable` once in an **elevated** terminal opened after the install — the command writes its pnpm shims beside Node in `C:\Program Files\nodejs`, which a regular terminal cannot write to (the failure is `EPERM`). A user-scoped Node install (nvm-windows, or Node placed under your profile) needs no elevation.
3. **System build deps** (Linux only — Windows got its build tools with rustup in step 1):
   ```sh
   sudo apt install -y build-essential libssl-dev pkg-config
   ```
4. **Clone and install JS deps** — one workspace install covers frontend and electron. From here on, every snippet is line-per-command and runs unchanged in bash, PowerShell, or cmd:
   ```sh
   git clone https://github.com/JoaoPucci/ani-gui.git
   cd ani-gui
   pnpm install
   ```
5. **Build the backend binary** (required before the first run, and after every Rust change):
   ```sh
   cd backend
   cargo build --bin ani-gui-backend
   ```
   On x86_64 Linux, also stage the bundled tools next to it once per checkout — playback needs the impersonating transport:
   ```sh
   cd ../electron
   pnpm run fetch:linux-deps
   ```
   The fetcher downloads x86_64 Linux builds (the architecture every package ships for), so skip it on any other host — the staged directory outranks PATH, and incompatible binaries staged there would shadow any transport you do have. Windows needs no staging step here: the dev launcher in step 6 stages its own.
6. **Run the dev app** — two terminals, each opened at the repository root (the step-5 shell is sitting in `backend/` or `electron/`, so don't continue in it), started in this order. Terminal A, the Vite dev server with HMR on :5173:
   ```sh
   cd frontend
   pnpm dev
   ```
   Terminal B, the Electron shell — spawns the backend binary from step 5, and on Windows also stages the bundled tools (impersonating transport, yt-dlp) next to it, so playback works:
   ```sh
   cd electron
   pnpm dev
   ```
7. **Build a distributable bundle** — a fresh terminal at the repository root. On an x86_64 Linux host (`pnpm package` instead builds only the `.AppImage`, for faster iteration):
   ```sh
   cd electron
   pnpm package:release
   ```
   On an x64 Windows host — any shell, with Rust, Node and pnpm installed natively; the dep fetcher needs `bsdtar`, which Windows 10+ ships as `tar.exe`:
   ```sh
   cd electron
   pnpm package:win
   ```
For lints, git hooks, and the bash test toolchain see [`docs/development.md`](./docs/development.md).

## First run

On first launch the app:

1. Spawns the Rust sidecar on a kernel-assigned localhost port (no fixed port, no internet-reachable service).
2. Loads the discovery surface.

After that, click anything that looks clickable. The app routes the click through Kitsu / AniList for metadata, its own resolver for the stream, and the embedded player for playback.

Upgrading from a version before 0.12 adds one step: those releases kept their own copy of the `ani-cli` script under your cache directory and refreshed it at every launch. Nothing reads it now, so the first launch after upgrading deletes it and says so on the **/diagnostics** page.

<p align="center">
  <img width="2751" height="1300" alt="player image" src="https://github.com/user-attachments/assets/db9f1816-d622-40ab-aa15-88a86f14f1d1" />
</p>

## Accounts & trackers

Connecting a list provider is optional — the app works fully without an account. From the **Account** page you can connect **AniList** or **MyAnimeList** (OAuth in your browser; the token is stored with your OS keychain via Electron's `safeStorage`, never in plaintext).

Once connected:

- **Watch Later rail** — your Plan-to-Watch list surfaces as a rail on the home page, bridged to local cards you can play in one click.
- **Automatic progress sync** — as you watch, the episode is pushed back to the tracker. The sync only ever moves progress *forward* (replaying or stepping back never lowers your count), promotes a Plan-to-Watch title to *Watching* on first play, preserves a *Rewatching* row, and marks a series *Completed* when you start the last episode of a finished show.

Everything stays on your machine: your OAuth token is encrypted through your OS keychain and written to the app's user-data directory (the Rust backend never persists it — each request carries its own bearer), and your tracker list is cached in a local SQLite database to render the Watch Later rail. ani-gui runs no server of its own. See the [privacy policy](docs/PRIVACY.md) for exactly what's sent where.

## Configuration

User settings live in `$XDG_CONFIG_HOME/ani-gui/config.toml`. The Settings page exposes everything you'd normally edit:

- audio mode (`sub` / `dub`) and quality (`best`, `1080`, `720`, `480`, `worst`)
- UI locale
- external-player kind, command, and custom arguments
- Syncplay binary path
- image-cache size cap
- auto-play next episode
- auto-skip OP / ED
- custom-vs-native player controls
- the download progress bar
- whether the update check includes pre-releases
- remembering resolved streams for quicker replays (off by default)

Full table with defaults and effects is in [`docs/architecture.md`](./docs/architecture.md#user-settings).

## How it works

A two-line summary: a Rust sidecar embedded inside an Electron shell speaks to Kitsu / AniList / aniskip and resolves streams from the providers itself — anidb.app first, hianime when the walk moves on from anidb.app (unreachable, refusing or rate-limiting, a page the app cannot read, a background request its gate turned away, or an answer that settles nothing — the show found with nothing said about the audio asked for, or a denial of a show a live record remembers it carrying), and, for a show found on hianime, hianime first while the app still remembers that (about a day from the last resolve that found the show there — a play, a download, a hand-off, or the background warm a page runs as it follows what is in view: the detail page for the episode its Play button targets, the play page for the next episode, or with resolution caching on for every aired episode in view, again as the grid or strip is paged — or, for a show a probe alone found and nothing resolved since, a day for an ongoing one and thirty days for a finished one; a play, hand-off or page warm served from the resolution cache leaves that memory as it is while it lasts and starts a day of it again once it has lapsed, while a download always resolves afresh). A streaming proxy in the sidecar carries the `Referer:` a provider's CDN checks for — hianime's wants the embed page's origin, anidb.app's wants none — and rewrites HLS playlists so the embedded `<video>` element can play upstream content without CORS or referer issues. SQLite caches metadata; the filesystem caches images.

For the long version — diagrams, cache TTLs, the title-resolution bridge, the player — see [`docs/architecture.md`](./docs/architecture.md), [`docs/title-resolution.md`](./docs/title-resolution.md), and the rest of [`docs/`](./docs/).

## Contributing

See [`docs/development.md`](./docs/development.md).

## Acknowledgements

ani-gui only exists because of the projects it builds on:

- **[pystardust/ani-cli](https://github.com/pystardust/ani-cli)** — the Bash scraper this project grew out of, and the source of the provider pipelines the Rust resolver reimplements.
- **[Kitsu](https://kitsu.io/)** and **[AniList](https://anilist.co/)** for the metadata, posters, and trending data behind the discovery surface.
- **[aniskip](https://aniskip.com/)** for the community-submitted OP/ED intervals.
- **[hls.js](https://github.com/video-dev/hls.js/)** for the HLS playback inside the embedded player.

## Disclaimer

ani-gui is a tool. Like any tool, the responsibility for how it's used lies with the user. The app makes no claim on the content it surfaces — it hosts nothing, talks to the same providers you'd reach in a browser, and routes their output through your machine.

## License

[GPL-3.0](./LICENSE).
