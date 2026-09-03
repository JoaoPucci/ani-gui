# Development

This page covers the dev environment, the build pipeline, and debugging tips. For architecture, read `architecture.md`. For test discipline, read `testing.md`.

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` | Rust sidecar backend |
| Node | 20+ | renderer + Electron shell |
| pnpm | 9+ | package manager |
| `bats-core`, `bats-mock`, `bats-assert`, `bats-file` | pinned in `tests/bash/helpers/install-bats.sh` | bash tests |
| `shellcheck`, `shfmt` | latest stable | bash linters |
| `mpv` | any | optional escape-hatch player |

## First-time setup

### Ubuntu / Debian (copy-paste)

Group these by what subsystem you intend to work on. You only need a group's tools when you're building or testing in that subsystem.

```sh
# Bash subsystem (the arch checks + their bats harness)
sudo apt install -y shellcheck python3-yaml
# shfmt is not in 24.04 apt; install the static binary:
sudo curl -sSL -o /usr/local/bin/shfmt \
  https://github.com/mvdan/sh/releases/download/v3.10.0/shfmt_v3.10.0_linux_amd64 \
  && sudo chmod +x /usr/local/bin/shfmt
```

```sh
# Rust sidecar backend
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install -y build-essential libssl-dev pkg-config
```

```sh
# Renderer + Electron shell (Node + pnpm via nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
# (open a new shell, or `source ~/.bashrc`)
nvm install 20 && nvm use 20
corepack enable && corepack prepare pnpm@latest --activate
```

```sh
# Quality of life (optional)
sudo apt install -y mpv jq ripgrep
```

### Then in the repo

```sh
git clone git@github.com:JoaoPucci/ani-gui.git
cd ani-gui

# Bash test toolchain (vendored bats + plugins at pinned tags)
./tests/bash/helpers/install-bats.sh

# Frontend + Electron deps. The frontend `pnpm install` also installs
# Lefthook and writes the `pre-commit` / `pre-push` git hooks. To skip
# the hooks for a single command set `LEFTHOOK=0`.
pnpm install   # one workspace install covers frontend/ and electron/

# Verify Rust toolchain
(cd backend && cargo --version)
```

### Other distros

Mostly same packages, different package manager. PRs welcome to add Fedora / Arch / openSUSE recipes.

## Dev loop

Three terminals:

```sh
# Terminal 1 — Vite dev server with HMR
cd frontend && pnpm dev          # http://localhost:5173

# Terminal 2 — build the Rust sidecar (one-shot per Rust change)
cd backend && cargo build --bin ani-gui-backend

# Terminal 3 — Electron shell (spawns the sidecar, points at Vite)
cd electron && pnpm dev
```

The Electron main process resolves the backend binary (`backend/target/debug/ani-gui-backend`), spawns it, and parses its stdout `ANI_GUI_LISTENING <url>` handshake to discover the loopback port. The renderer reads that URL from `window.aniGui.apiBase` (set by the Electron preload script) and uses it for every `fetch()` call.

## Build for distribution

```sh
cd electron
pnpm package          # AppImage only — fast iteration
pnpm package:release  # AppImage + .deb
```

Artifacts land in `electron/dist/`. There is no release-packaging CI —
no workflow triggers on a tag and nothing publishes installers. (The
e2e workflow does run `pnpm run dist` on Linux to produce the
`linux-unpacked/` build it tests against, so electron-builder itself is
exercised in CI; publishing is not.) Release artifacts are built on a
matching host and uploaded to the GitHub release by hand:

| Target | Host | Command |
|---|---|---|
| `.AppImage` + `.deb` | Linux | `pnpm package:release` |
| NSIS installer (`.exe`) | Windows | `pnpm package:win` |

The `electron-builder` config declares no macOS target; nothing
produces a `.dmg`. The dev loop (Vite + Electron from source) runs on
any platform with a POSIX shell — the dev scripts set environment
variables with POSIX prefixes, so on Windows run them from Git Bash —
but no macOS artifact is built or shipped.

## Logging and debugging

The backend uses [`tracing`](https://docs.rs/tracing). Adjust verbosity by setting `RUST_LOG` before launching the backend (or the Electron shell that spawns it):

```sh
RUST_LOG=ani_gui=debug,axum=info pnpm --dir electron dev
```

Logs also tee to `$XDG_DATA_HOME/ani-gui/logs/ani-gui.log` (daily rotation, 7-day retention).

The streaming proxy port is logged at startup:

```
INFO ani_gui::proxy: stream proxy listening on 127.0.0.1:42337
```

Use it to inspect proxied requests with `curl`:

```sh
curl -sI http://127.0.0.1:42337/healthz
```

## Useful environment variables

| Variable | Purpose |
|---|---|
| `RUST_LOG` | tracing filter |
| `ANI_GUI_UPSTREAM_BASE` | dev/test only; redirects `meta_http` to a wiremock instance |
| `VITE_ANI_GUI_API_BASE` | browser-only dev: point the Vite renderer at a separately-running backend |
| `ANI_GUI_DEV` | forces the dev data profile (`ani-gui-dev` dirs) — see below. Auto-set by the Electron dev launcher; rarely needed by hand |

**Dev data isolation.** Any source-built backend (`cargo run`, the
standalone browser-dev flow above, the Electron dev launcher) is a debug
build, and debug builds resolve all ani-gui-owned dirs — config, cache,
`metadata.sqlite`, state — under `ani-gui-dev` instead of `ani-gui`. That
keeps a dev build from migrating the installed app's DB forward, which
the older release binary would then refuse to open. The shipped binary is
built `--release`, so it uses the real `ani-gui` dirs. Set `ANI_GUI_DEV`
to force the dev profile from a release build (e.g. to test migrations
against throwaway data).

## Code style

- **Rust**: `cargo fmt` (settings in `rustfmt.toml`); `cargo clippy -D warnings` enforced by CI.
- **TS / Svelte**: `prettier` for formatting; `eslint` (svelte plugin + custom rules) for behavior.
- **Bash**: `shfmt -i 4 -ci -d` across `tests/bash/` and `tests/arch/`.

## Frequently asked

**Why doesn't an `mpv` window pop up when I play something?**
The GUI plays inside the window using hls.js + a local stream proxy. `mpv` is only launched if you click "Open in external player".

**Where does the GUI keep watch history?**
In its own state directory, as `history` — the CLI keeps its own file separately. The two used to share one, but the provider switch re-keyed both sides onto different show ids, so a shared file no longer carried anything either could read.

**Why does the dev server work in a browser tab too?**
By design — opening `http://localhost:5173` in any browser shows the same UI the Electron renderer loads. Useful for fast iteration. Production builds always run inside Electron because the streaming proxy + native window matter for the shipped product; standalone-browser dev only works against a separately-running backend (set `VITE_ANI_GUI_API_BASE` to its loopback URL).
