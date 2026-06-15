<h1 align="center">ani-gui</h1>

<p align="center">
  <em>A desktop app for browsing and watching anime.</em>
</p>

<p align="center">
  <img width="2751" height="1300" alt="home image" src="https://github.com/user-attachments/assets/ee2e3d80-01e8-46cb-afa0-a132cd3e3273" />
</p>

ani-gui is a graphical front-end for [pystardust/ani-cli](https://github.com/pystardust/ani-cli). It embeds the upstream Bash scraper unmodified and wraps it in a Rust + SvelteKit desktop application — discovery, search, an embedded player, downloads, persistent watch history, Picture-in-Picture, and OP/ED skip on top of the same scraping engine.

The CLI still exists. The GUI does not replace it; the two share the script and coexist in this repository. See [`docs/architecture.md`](./docs/architecture.md) for the full picture.

## Features

|  | |
|---|---|
| **Discovery** | Trending, This Season, Top Rated, Recently Released — AniList + Kitsu. |
| **Search** | Full-text against Kitsu, instant as you type. |
| **Detail page** | Synopsis, episodes with thumbnails, similar-titles strip. |
| **Embedded player** | HLS / MP4, quality switch, native or custom controls — no `mpv` window. |
| **Subtitles** | Upstream `.vtt` via `<track kind="subtitles">`. |
| **OP / ED skip** | aniskip intervals — one-click or fully automatic. |
| **Picture-in-Picture** | Persists across navigation. |
| **Background prefetch** | Adjacent episodes warm in advance. |
| **Downloads** | Per-episode or ranged, progress dock. aria2c bundled; ffmpeg sourced per platform (apt `Recommends:` on `.deb`, installer-time fetch on Windows, system PATH on AppImage). |
| **Shared history** | Continue Watching reads/writes `$XDG_STATE_HOME/ani-cli/ani-hsts` — same file as the CLI. Remove a single card or clear the lot from the rail. |
| **External player** | One click to mpv / VLC / IINA / custom. |
| **Watch together** | Hand the current stream to [Syncplay](https://syncplay.pl/) for a watch party. |
| **Trackers** | Connect AniList or MyAnimeList — a Watch Later rail on the home page, and your progress synced back automatically as you watch. |
| **Localised** | English, Brazilian Portuguese, Latin American Spanish, Russian. |
| **No telemetry** | No analytics or tracking. Outbound traffic is metadata, the stream you picked, launch-time update checks (the app's own GitHub releases + the bundled script's self-update), and — with an account connected — your tracker's list + progress sync. Localhost-only listener on a kernel-assigned port. See the [privacy policy](docs/PRIVACY.md). |

## Install

ani-gui is distributed as a desktop bundle. The bundled script is updated automatically on launch (see *Self-update* below); a separate `ani-cli` install is **not** required.

Platform support tiers:

| Tier | Platform | Status |
|---|---|---|
| 1 | Linux | Actively tested on Ubuntu. Other distros work via AppImage. |
| 2 | Windows | Most features verified end-to-end. Edge cases may surface. |
| 3 | macOS | Untested. Builds the same way; should work — please file an issue if it doesn't. |

<details>
<summary><strong>Linux</strong> — tier 1 (tested on Ubuntu)</summary>

- **AppImage** — download from the [releases page](https://github.com/JoaoPucci/ani-gui/releases), `chmod +x`, double-click. The bundle launches with Chromium's setuid sandbox disabled (AppImage's read-only FUSE mount can't carry the SUID bit `chrome-sandbox` requires); the localhost-only architecture means the sandbox isn't load-bearing for the threat model. If you'd rather keep the sandbox, install the `.deb` instead.
- **Debian / Ubuntu (`.deb`)** — `sudo apt install ./ani-gui_<version>_amd64.deb`. apt pulls in the recommended `ffmpeg` package (needed for the download feature) along the way; the post-install script sets the `chrome-sandbox` SUID bit Electron needs, so the sandbox stays on. `sudo dpkg -i …` still works but won't auto-install ffmpeg — drop into `apt --fix-broken install` or run `sudo apt install ffmpeg` separately if you used dpkg directly.

</details>

<details>
<summary><strong>Windows</strong> — tier 2 (most functions tested)</summary>

NSIS installer (`.exe`). Run it; it installs per-user by default and creates Start menu and desktop shortcuts.

ani-gui drives the upstream `ani-cli` Bash script via `bash`, which on Windows ships as part of [Git for Windows](https://gitforwindows.org/). If Git for Windows isn't installed when you launch the app, you'll see a dialog with a one-click link to its download page. The installer will fetch ffmpeg automatically the first time it runs (~80 MB) so the download feature works out of the box; aria2c and fzf are bundled directly. The ffmpeg fetch runs even when you already have ffmpeg installed via a per-user package manager (scoop, winget user-scope) — the installer's elevated context doesn't see per-user PATH entries, and the bundled copy is what the app uses at runtime in either case.

</details>

<details>
<summary><strong>macOS</strong> — untested</summary>

A `.dmg` is produced by the same `electron-builder` config and should install via the standard drag-into-Applications flow. macOS isn't part of the regular acceptance pass, so if you hit a problem please [open an issue](https://github.com/JoaoPucci/ani-gui/issues) — the app is shipped for it but unverified.

</details>

## Build from source

Tested on Linux. The dev loop (steps 5–6) works the same on macOS and Windows; the packaging scripts (step 7) build per-host artifacts — run on Linux for `.AppImage` / `.deb`, on Windows for the NSIS installer.

1. **Install Rust** (toolchain pinned by `rust-toolchain.toml`):
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   . "$HOME/.cargo/env"   # (or re-open the shell) so `cargo` is on PATH
   ```
2. **Install Node 20+ and pnpm** (via nvm — skip the curl step if you already have nvm or installed Node another way):
   ```sh
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
   # re-open the shell (or `source ~/.bashrc`) so nvm is on PATH
   nvm install 20 && nvm use 20
   corepack enable && corepack prepare pnpm@latest --activate
   ```
3. **System build deps** (Linux):
   ```sh
   sudo apt install -y build-essential libssl-dev pkg-config
   ```
4. **Clone and install JS deps**:
   ```sh
   git clone https://github.com/JoaoPucci/ani-gui.git && cd ani-gui
   (cd gui/frontend && pnpm install)
   (cd gui/electron && pnpm install)
   ```
5. **Build the backend binary** (required before the first run, and after every Rust change):
   ```sh
   cd gui/backend && cargo build --bin ani-gui-backend
   ```
6. **Run the dev app** — two terminals, started in this order:
   ```sh
   # Terminal A — Vite dev server, HMR on :5173
   cd gui/frontend && pnpm dev

   # Terminal B — Electron shell, spawns the backend binary from step 5
   cd gui/electron && pnpm dev
   ```
7. **Build a distributable bundle**:
   ```sh
   cd gui/electron

   # Linux host
   pnpm package           # .AppImage — fast iteration
   pnpm package:release   # .AppImage + .deb

   # Windows host (Git Bash / PowerShell, with Rust + Node + pnpm installed natively;
   # `fetch:win-deps` needs `bsdtar`, which Git for Windows already ships)
   pnpm package:win       # NSIS installer
   ```
   macOS `.dmg` is produced by CI on a `macos-*` runner — for local mac builds see [`docs/development.md`](./docs/development.md).

For lints, git hooks, and the bash test toolchain see [`docs/development.md`](./docs/development.md).

## First run

On first launch the app:

1. Spawns the Rust sidecar on a kernel-assigned localhost port (no fixed port, no internet-reachable service).
2. Materialises the bundled `ani-cli` script to `$XDG_CACHE_HOME/ani-gui/ani-cli` so it can be patched in place by `-U`.
3. Runs `bash ani-cli -U` in the background to pick up any same-day upstream hotfixes.
4. Loads the discovery surface.

After that, click anything that looks clickable. The app routes the click through Kitsu / AniList for metadata, `ani-cli` for the actual stream resolution, and the embedded player for playback.

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
- external-player kind and command
- image-cache size cap
- auto-play next episode
- auto-skip OP / ED
- custom-vs-native player controls
- whether to enter PiP automatically when you navigate away from a playing video
- whether to keep `ani-cli` self-updating on launch

Full table with defaults and effects is in [`docs/architecture.md`](./docs/architecture.md#user-settings).

### Self-update of the bundled scraper

Allmanga (the catalogue `ani-cli` scrapes) changes its API often, and upstream `pystardust/ani-cli` ships hotfixes daily. The bundled snapshot in your install would go stale within a week.

ani-gui handles this for you: on every launch a background task runs `bash <cached-ani-cli> -U`, captures the outcome, and persists the last few attempts. The app itself isn't blocked by the update — startup proceeds normally; the script is patched in place by the next time you trigger a search or a play.

The flow is gated by the **Auto-update ani-cli** setting (default ON). When it's off, the bundle just keeps using whatever script is in your cache, indefinitely. The latest update outcome is visible on the **/diagnostics** page.

## How it works

A two-line summary: a Rust sidecar embedded inside an Electron shell speaks to Kitsu / AniList / aniskip and spawns `ani-cli` as a subprocess for stream resolution. A streaming proxy in the sidecar adds the right `Referer:` headers and rewrites HLS playlists so the embedded `<video>` element can play upstream content without CORS or referer issues. SQLite caches metadata; the filesystem caches images.

For the long version — diagrams, cache TTLs, the title-resolution bridge, the PiP architecture — see [`docs/architecture.md`](./docs/architecture.md), [`docs/title-resolution.md`](./docs/title-resolution.md), and the rest of [`docs/`](./docs/).

## Contributing

See [`docs/development.md`](./docs/development.md). The repository carries one upstream patch (a single source-guard line in `ani-cli` for testability) and otherwise mirrors `pystardust/ani-cli` so script-side fixes flow in without conflict.

## Acknowledgements

ani-gui only exists because of the projects it builds on:

- **[pystardust/ani-cli](https://github.com/pystardust/ani-cli)** — the Bash scraper that does the actual stream resolution. ani-gui ships the script unmodified.
- **[Kitsu](https://kitsu.io/)** and **[AniList](https://anilist.co/)** for the metadata, posters, and trending data behind the discovery surface.
- **[aniskip](https://aniskip.com/)** for the community-submitted OP/ED intervals.
- **[hls.js](https://github.com/video-dev/hls.js/)** for the HLS playback inside the embedded player.

## Disclaimer

ani-gui is a tool. Like any tool, the responsibility for how it's used lies with the user. The app makes no claim on the content it surfaces — it talks to the same providers you'd reach in a browser and routes their output through your machine. The full project disclaimer applies: see [`disclaimer.md`](./disclaimer.md).

## License

[GPL-3.0](./LICENSE), inheriting from upstream `pystardust/ani-cli`.
