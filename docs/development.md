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
nvm install 20
# pnpm needs no separate install or version choice: package.json's
# packageManager field pins it, and corepack fetches that version.
corepack enable
```

```sh
# Quality of life (optional)
sudo apt install -y mpv jq ripgrep
```

### Then in the repo

Clone and run the one workspace install, which covers `frontend/`
and `electron/`; the frontend's install also sets up Lefthook and
writes the `pre-commit` / `pre-push` git hooks (`LEFTHOOK=0` skips
them for a single command, and `LEFTHOOK_EXCLUDE=<name>` skips one
check by its name in `lefthook.yml`; AGENTS.md §12 says when a
commit may do either):

```sh
git clone git@github.com:JoaoPucci/ani-gui.git
cd ani-gui
pnpm install
```

The bash test toolchain (vendored bats + plugins at pinned tags) is
POSIX by design — run this from a POSIX shell (Git Bash on Windows),
and only if you work in the bash layer:

```sh
./tests/bash/helpers/install-bats.sh
```

Verify the Rust toolchain, ending back at the root, where the dev
loop below starts:

```sh
cd backend
cargo --version
cd ..
```

### Other distros

Mostly same packages, different package manager. PRs welcome to add Fedora / Arch / openSUSE recipes.

## Dev loop

In order, starting from the repository root. The snippets hold pure
commands, one per line, and paste unchanged into bash, PowerShell,
or cmd — cmd has no `#` comment syntax, so the notes live out here
instead. The loop is supported on Linux and Windows; the two
long-running processes each get their own terminal, opened at the
root.

Step 1 — build the Rust sidecar (one-shot per Rust change):

```sh
cd backend
cargo build --bin ani-gui-backend
```

Step 2 — once per checkout, on x86_64 Linux only, stage the bundled
deps (Windows skips this: the dev launcher in step 4 stages its
own):

```sh
cd ../electron
pnpm run fetch:linux-deps
```

Step 3 — the Vite dev server with HMR, at `http://localhost:5173`
(keep running, own terminal):

```sh
cd frontend
pnpm dev
```

Step 4 — the Electron shell; spawns the sidecar, points at Vite
(keep running, own terminal):

```sh
cd electron
pnpm dev
```

Step 3 has to be answering before step 4 starts. The shell loads its
first page from Vite, and when that load fails — nothing is
listening, or Vite answers the page with an error status — it prints
the reason on its terminal (`[main] startup failed: …`) and exits
with code 1. Once Vite is serving the page, a dev launch waits as
long as the first paint takes — a cold Vite re-optimizing its
dependencies can keep it waiting a while — where a packaged build
gives up after two minutes. The backend gets the same two minutes to
answer at startup, in dev and packaged alike: both deadlines are
there for something that will never happen, not for something slow.

Step 2's position is load-bearing on a fresh checkout: it must run
after the first build, because the fetcher mirrors into
`backend/target/{debug,release}/bin` only for profile directories
that already exist — and before Electron, because the backend
snapshots that directory once at startup. Playback needs the staged
impersonating transport (the resolver's plain-`curl` fallback is
rejected by the provider); if you stage while Electron is already
running, restart it.

Do not run step 2 anywhere else — not on macOS, and not on other
Linux architectures: the fetcher downloads x86_64 Linux builds (the
same architecture every package ships for), and the staged directory
outranks PATH, so on any other host the incompatible binaries would
shadow a usable transport the host actually has.

Without the staging step the app still launches and browses metadata;
only stream resolution and downloads need the staged tools.

The Electron main process resolves the backend binary (`backend/target/debug/ani-gui-backend`), spawns it, and parses its stdout handshake — one `ANI_GUI_LANE <name> <url>` line per lane, then `ANI_GUI_LISTENING <url>` — to discover the loopback ports. The renderer reads the API's URL from `window.aniGui.apiBase` and the lanes from `window.aniGui.lanes` (both set by the Electron preload script), and sends each request to its lane ([architecture](./architecture.md#one-loopback-origin-per-class-of-traffic)).

## Build for distribution

```sh
cd electron
pnpm package          # AppImage only — fast iteration
pnpm package:release  # AppImage + .deb
```

On Linux the packages carry a backend built for
`x86_64-unknown-linux-musl`, which links its C library into the
binary instead of borrowing the system's glibc. A backend linked
against the build machine's glibc refuses to start on any system with
an older one, and the build machine is usually newer than the
systems the packages run on. Building it needs the musl target and a
C compiler for it, once per machine. Run the `rustup` line from inside
the repository: `rust-toolchain.toml` pins the toolchain cargo uses
here, and a target added from anywhere else goes on rustup's default
toolchain instead.

```sh
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools   # musl-gcc, for the C the backend compiles in (SQLite, ring)
```

`musl-tools` is the Debian and Ubuntu package; Fedora calls it
`musl-gcc` and Arch `musl`.

`pnpm package`, `pnpm package:release` and the `dist` scripts build
that backend themselves (`pnpm run build:backend:linux`). The dev loop
keeps using the ordinary debug build.

Artifacts land in `electron/dist/`. There is no release-packaging CI —
no workflow triggers on a tag and nothing publishes installers. (The
e2e workflow does run `pnpm run dist` on Linux to produce the
`linux-unpacked/` build it tests against, so electron-builder itself is
exercised in CI; publishing is not.) Release artifacts are built on a
matching host and uploaded to the GitHub release by hand:

| Target | Host | Command |
|---|---|---|
| `.AppImage` + `.deb` | x86_64 Linux | `pnpm package:release` |
| NSIS installer (`.exe`) | x64 Windows | `pnpm package:win` |

The host architectures are load-bearing, not a formality: the dep
fetchers stage hard-coded x86_64 assets and the AppImage repack
prepends an x86_64 runtime, so a build on any other architecture
produces a package with incompatible executables or fails outright.

The `electron-builder` config declares no macOS target; nothing
produces a `.dmg`. The dev loop (Vite + Electron from source) runs
on Linux, where `fetch:linux-deps` stages the transport playback
needs, and on Windows, where the `dev` launcher stages the bundled
tools itself before starting Electron. The launcher is a Node
script, so pnpm's script shell — `cmd.exe` on Windows, regardless
of the invoking terminal — has no shell-dialect syntax to trip
over. On macOS the app launches and browses metadata, but no
fetcher stages a transport there, so stream resolution falls back
to a plain `curl` the provider rejects. No macOS artifact is built
or shipped.

## Logging and debugging

The backend uses [`tracing`](https://docs.rs/tracing). Adjust verbosity by setting `RUST_LOG` before launching the backend (or the Electron shell that spawns it). bash:

```sh
RUST_LOG=ani_gui=debug,axum=info pnpm --dir electron dev
```

PowerShell:

```powershell
$env:RUST_LOG = 'ani_gui=debug,axum=info'; pnpm --dir electron dev
```

cmd:

```bat
set RUST_LOG=ani_gui=debug,axum=info
pnpm --dir electron dev
```

There is no log file: the backend writes to its own stderr/stdout, and the Electron shell relays both to the terminal it was launched from (prefixed `[backend]`, alongside `[renderer:*]` console lines, tagged with the level Electron reports for each: `verbose` for `console.debug`, `info` for `console.log` and `console.info`, then `warning` and `error`). A packaged build launched from the desktop has no terminal, so to capture its logs start the executable from a shell with output redirected to a file.

A packaged build whose startup fails — the backend does not start, or the window never gets its first page — says so in an error dialog before it exits with code 1: in the app's language, with the failure's own message as the detail (the same text the `[main] startup failed:` log line carries). The dialog closes by itself after a minute. A dev launch shows no dialog; the reason is on its terminal.

The player logs which level of an HLS stream it loads and plays with `console.debug`, which the shell relays as `[renderer:verbose]` lines:

```
[renderer:verbose] [play] quality: start: 1080p (level 2, 5.0 Mbps) …
[renderer:verbose] [play] quality: playing 1080p (level 2, 5.0 Mbps) …
[renderer:verbose] [play] quality: stall: 1080p (level 2, 5.0 Mbps) → 720p (level 1, 2.8 Mbps) …
```

`start` is the level the stream began at, `playing` each change of the level on screen, and every other change names its reason (`stall`, `fragments-failed`, `level-failed`, `followed`, `healthy`, `pick`, `auto`). A level whose segments turn out to be encoded at another height than its playlist lists says so once, as `level 0 encodes 480p; its playlist says 360p`. From then on the log names that level by both heights — `480p (level 0, listed 360p, 0.8 Mbps)` — because the player moves through the levels in the order their playlists declare.

The streaming proxy port is logged at startup:

```
INFO ani_gui::proxy: stream proxy listening on 127.0.0.1:42337
```

Use it to inspect proxied requests with `curl`. In Windows
PowerShell spell it `curl.exe` — bare `curl` there is an alias for
`Invoke-WebRequest`, which rejects `-sI`:

```sh
curl -sI http://127.0.0.1:42337/healthz
```

## Useful environment variables

| Variable | Purpose |
|---|---|
| `RUST_LOG` | tracing filter |
| `ANI_GUI_UPSTREAM_BASE` | dev/test only; redirects `meta_http` to a wiremock instance |
| `VITE_ANI_GUI_API_BASE` | browser-only dev: point the Vite renderer at a separately-running backend |
| `ANI_GUI_UNATTENDED` | set to `1` by a harness with nobody at the screen (the e2e launch helper and the backend's integration tests always do): a packaged build whose startup fails then exits at once instead of showing its error dialog, and the backend never fetches the live remote flags file, taking its flags from `ANI_GUI_TEST_FLAGS_FILE` or the defaults ([Remote flags](#remote-flags)) |
| `ANI_GUI_TEST_FLAGS_FILE` | only with `ANI_GUI_UNATTENDED=1`: a flags file, in the remote file's format, that the test run's flags come from. Ignored on any other launch |
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

Only one instance of the app runs per profile; a second launch on the
same profile hands over to the running one and exits. The Electron shell
puts its own profile (and with it that lock) under `ani-gui-dev` when the
dev launcher's `ELECTRON_DEV=1` or `ANI_GUI_DEV` is set, so `pnpm dev`
runs beside an installed copy. A bare `electron .` with neither set uses
the installed app's shell profile, and hands over to it if it is running.

## Remote flags

The maintainer can switch a declared flag without a release by editing
one file on the repository's `flags` branch. What a flag may do, and
why its flags need no signature, is in
[`architecture.md`](./architecture.md#remote-flags). Only a flag
declared in `backend/src/flags/mod.rs` (`DECLARED`) has any effect. A
build ignores names it does not declare and values its declaration
does not list.

**Creating the branch (once).** The branch holds `flags.json` and,
once the file is signed, its signature `flags.json.minisig` and a
`.gitattributes` that keeps both byte for byte (see "Signing the flags
file" below). It is not the default branch, so the `master` ruleset
does not cover it. These commands create it in a separate worktree
next to the checkout. Run them in bash from the repository root:

```sh
git worktree add --detach ../ani-gui-flags
cd ../ani-gui-flags
git switch --orphan flags
printf '{ "flags": {} }\n' > flags.json
git add flags.json
git commit -m "chore: start the remote flags file"
git push -u origin flags
```

Until the branch exists, every launch gets a 404 and uses the
defaults, which is also what an empty `flags` object does, and each
session retries the file after 3, 10 and 30 minutes and then hourly.
Create the branch before the first release that carries this.

**The format.** One JSON object with a `flags` object inside. A kill
switch takes `false`; a fallback takes the name of one of the earlier
behaviours its declaration lists:

```json
{
  "flags": {
    "some_kill_switch": false,
    "some_fallback": "previous"
  }
}
```

**Flipping a flag.** Edit `flags.json` in the worktree above, not in
GitHub's web editor, and publish it. Once `flags.json.minisig` is on the
branch, publish every edit, a flag change included, with step 3 of
[Signing the flags file](#signing-the-flags-file): the signature covers
the whole file, and an edit pushed without a fresh one drops every
notice the file carries (step 3 says how). Until then there is nothing
to drop, and the file is published unsigned. In bash:

```sh
cd ../ani-gui-flags &&
  git pull &&
  python3 -m json.tool flags.json &&
  git commit -am "chore: switch some_kill_switch off" &&
  git push
```

In PowerShell (on `py -3`, see step 3):

```powershell
& {
  Set-Location -ErrorAction Stop ..\ani-gui-flags
  git pull
  if ($LASTEXITCODE -ne 0) { throw 'git pull failed' }
  py -3 -m json.tool flags.json
  if ($LASTEXITCODE -ne 0) { throw 'flags.json is not valid JSON' }
  git commit -am "chore: switch some_kill_switch off"
  if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
  git push
  if ($LASTEXITCODE -ne 0) { throw 'git push failed' }
}
```

Each command runs only if the one before it succeeded, so a file that
is not JSON is never committed or pushed.

`python3 -m json.tool` checks only that the file is JSON. Two other
mistakes get past it and fail differently:

- **The wrong shape** (no `flags` object at the top, for example) is
  treated like malformed JSON. Each launch counts it as a failed fetch
  and keeps the copy it cached, so the change does not apply, and an
  earlier retreat stays in force for up to seven days.
- **A wrong name or value** (a misspelt flag, `"false"` as a string, a
  behaviour the declaration does not list) puts that flag back at its
  default. That lifts any retreat already in force for it.

So after pushing, wait out the five-minute CDN cache, launch the app,
and confirm the values with `GET /api/flags` (below).

**Lifting a retreat.** Set the flag back to its default (`true`, or
the current behaviour's name), or remove its entry, and publish the
file as above. Removing
the whole file, or the branch, also works in the end. But installs then
get a 404, which counts as a failed fetch, so each one keeps the copy it
cached for up to seven days.

**How long until installs see it.** `raw.githubusercontent.com` serves
the file with `Cache-Control: max-age=300`, so the CDN can return the
old copy for up to five minutes after a push. After that, each install
picks up the change the next time it launches, when the file arrives
within that launch's two-second cap. A launch on a slow connection that
misses the cap keeps fetching in the background and caches the file,
so it applies from the launch after. Worst case: one launch delay;
never stuck unless the file can't be downloaded at all during an entire
session. An app already running keeps the flags it decided at launch
until it is restarted. An install that cannot reach GitHub at all keeps
its cached copy for up to seven days, then uses the defaults.

To see what a running backend decided, read
`GET <api base>/api/flags`. It returns every declared flag's value and
where they came from (`origin`: `file`, `cache` or `defaults`, or
`test` in a test run). The
backend logs the decision at `info` as `remote flags settled`, with the
same origin. The app's window gets the same values from the preload, as
`window.aniGui.flags`.

**Development and test runs.** `pnpm dev` is a normal launch: it reads
the live file, so a flag flipped on the branch reaches a dev run too.
Test runs never do. The end-to-end launch helper and the backend's
integration tests set `ANI_GUI_UNATTENDED=1`, and their flags come from
`ANI_GUI_TEST_FLAGS_FILE` or the defaults (see the table of
environment variables above).

**Declaring a flag** (for the change that adds one). Declare it as a
`pub const FlagDecl`, list it in `DECLARED`, read it through that
constant, and register a test case for its default and for each retreat
in `flags::matrix::CASES`. A test fails until every value has one.

## Signing the flags file

The flags themselves need no signature (see
[`architecture.md`](./architecture.md#remote-flags)). The notices the
file may carry for the notification center, and its donation-ask
number, are used only when `flags.json` is signed with minisign and the
signature verifies against a public key the build trusts
(`backend/src/flags/signature.rs`, `TRUSTED_KEYS`). Each launch fetches
`flags.json.minisig` from beside `flags.json` on the `flags` branch. An
unsigned file, a signature that does not verify, or a build that trusts
no key keeps the file's flags and drops its notices. The shipped build
trusts no key yet: the first one goes in with the release that starts
showing remote notices.

The private key never goes into this repository, a pull request, an
issue, CI or any shared drive. Keep it, and its password, offline with
the maintainer. Anyone holding it can put text and a link in front of
every user.

These steps need [minisign](https://jedisct1.github.io/minisign/)
0.11 or later. Each step is given for bash on Linux and for
PowerShell 7.1 or later on Windows.

**0. Keep the file's bytes as they are pushed (once).** The signature
covers the exact bytes, and the `flags` branch does not carry the main
branch's `.gitattributes`. A checkout that converts line endings (Git
for Windows with `core.autocrlf=true`) would sign bytes that are not the
ones GitHub serves. In the branch's worktree:

```sh
printf 'flags.json -text\n*.minisig -text\n' > .gitattributes
git add .gitattributes
git commit -m "chore: keep the flags file's bytes as signed"
git push
```

```powershell
Set-Content -NoNewline -Path .gitattributes -Value "flags.json -text`n*.minisig -text`n"
git add .gitattributes
git commit -m "chore: keep the flags file's bytes as signed"
git push
```

**1. Make the key pair (once per key).**

```sh
mkdir -p ~/.minisign
minisign -G -p ~/.minisign/ani-gui-flags.pub -s ~/.minisign/ani-gui-flags.key
```

```powershell
New-Item -ItemType Directory -Force "$HOME\.minisign" | Out-Null
minisign -G -p "$HOME\.minisign\ani-gui-flags.pub" -s "$HOME\.minisign\ani-gui-flags.key"
```

minisign asks for a password for the secret key. `ani-gui-flags.pub`
holds two lines; the second is the public key, a base64 string starting
`RW`.

**2. Trust the key in a release.** Add that second line to
`TRUSTED_KEYS` in `backend/src/flags/signature.rs`, update the two
tests that pin the shipped build as trusting no key
(`the_shipped_build_trusts_no_key_until_the_maintainer_adds_one` in
`signature_test.rs` and `the_shipped_build_trusts_no_signature_yet` in
`remote_signed_test.rs`), and ship the release. Installs on older releases ignore
signed content; that is the safe direction.

**3. Sign and publish.** After every edit of `flags.json`, sign the
exact bytes you push, then commit the file and its signature together:

```sh
cd ../ani-gui-flags &&
  git pull &&
  python3 -m json.tool flags.json &&
  minisign -S -s ~/.minisign/ani-gui-flags.key -m flags.json \
    -t "flags.json $(date -u +%Y-%m-%dT%H:%M:%SZ)" &&
  minisign -V -p ~/.minisign/ani-gui-flags.pub -m flags.json &&
  git add flags.json flags.json.minisig &&
  git commit -m "chore: say what changed" &&
  git push
```

`-S` writes `flags.json.minisig` beside the file with minisign's default
prehashed signature, the only kind the app accepts; `-V` checks it
before it is pushed. Each command runs only if the one before it
succeeded, so a file that is not JSON, or a signature that does not
verify, is never committed or pushed. Put what changed in the commit
message. In PowerShell (Windows' `python3` may be the Store's
placeholder, so the check uses `py -3`; PowerShell's own
`ConvertFrom-Json` accepts comments the app does not, so it is no
substitute):

```powershell
& {
  Set-Location -ErrorAction Stop ..\ani-gui-flags
  git pull
  if ($LASTEXITCODE -ne 0) { throw 'git pull failed' }
  py -3 -m json.tool flags.json
  if ($LASTEXITCODE -ne 0) { throw 'flags.json is not valid JSON' }
  minisign -S -s "$HOME\.minisign\ani-gui-flags.key" -m flags.json -t "flags.json $(Get-Date -AsUTC -UFormat '%Y-%m-%dT%H:%M:%SZ')"
  if ($LASTEXITCODE -ne 0) { throw 'signing failed' }
  minisign -V -p "$HOME\.minisign\ani-gui-flags.pub" -m flags.json
  if ($LASTEXITCODE -ne 0) { throw 'the signature does not verify' }
  git add flags.json flags.json.minisig
  if ($LASTEXITCODE -ne 0) { throw 'git add failed' }
  git commit -m "chore: say what changed"
  if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
  git push
  if ($LASTEXITCODE -ne 0) { throw 'git push failed' }
}
```

After pushing, wait out the CDN's five-minute cache: until both files
are fresh it can serve the new file with the old signature, and a
launch in that window keeps the flags and drops the notices. An edit
pushed without a fresh signature leaves the old signature beside new
bytes: installs keep the flags and drop the notices until a matching
signature is pushed. GitHub's web editor cannot sign, so make every
edit in the worktree; an edit already made there needs this step at
once.

**4. Rotating the key.** Make the new pair (step 1), add its public key
to `TRUSTED_KEYS` beside the old one, and ship that release. Once it is
out, sign with the new key only. A release later, remove the old key
from the list. If a secret key is lost or leaked, delete
`flags.json.minisig` from the branch at once, and ship a release that
removes that key. An install drops the notices once a launch has
downloaded the unsigned file: on that launch if the file arrives within
its two-second wait, otherwise on the next (a deleted signature answers
404 at once, so it adds no wait). One that cannot reach the
file keeps its cached copy, and the notices in it, for up to seven
days.

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
