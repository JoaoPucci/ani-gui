# Proposal: an in-app updater

**Status**: research for the 0.20.0 release, alongside the
[notification center](notification-center.md) proposal, which is
reviewed separately; links to it resolve once it is merged. Nothing
below is built.
The library and platform facts were checked on 2026-11-02 against the
pages and source files cited next to them, and will drift; the
library versions named are the ones current on that day.

Labels used below: **[code]** is something read in this repository;
**[tried]** is something run on the Linux build machine for this
proposal; **[release]** is something read from the repository's published
GitHub releases; **[source]** is a fact taken from the external page
or source file cited next to it; **[unverified]** is a claim no source
was checked for; **[inference]** is a conclusion drawn from the
labelled facts around it; **[proposal]** is a design choice.

## The problem in plain terms

When a new version is out, the app says so and stops there:

1. At launch the app asks GitHub for the newest release. If it is
   newer than the running version, a badge with a glowing dot appears
   in the top bar. [code: `frontend/src/routes/+layout.svelte`,
   `frontend/src/lib/components/UpdateBadge.svelte`]
2. Clicking the badge opens a dialog with the two version numbers and
   the release notes. Its one action, "Open releases page", opens the
   release on GitHub in the system browser. [code:
   `frontend/src/lib/components/UpdateDialog.svelte`]
3. From there the user is on their own: pick the right file out of
   three, download it, and install it the way their package needs —
   replace the AppImage and make it executable again, install the
   `.deb` with root, or run the Windows installer.

The maintainer asked for a study of the alternative: the app downloads
the update itself and installs it, or gets as close to that as each
package allows.

The short answer, argued below: **yes for the AppImage and for
Windows, no for the `.deb`.** The AppImage and a per-user Windows
install belong to the user, so the app can replace them with nothing
more than a restart. The `.deb` belongs to the system, and the honest
place for its updates is the system's package manager.

## 1. What the app does today

- **The check.** The backend's `/api/update-check` asks GitHub's REST
  API for the newest release once per launch and returns it, or 204 on
  any failure; the renderer compares versions and fills the badge.
  The renderer never calls GitHub itself — the backend is the app's
  outbound HTTP boundary. [code: `backend/src/meta/github.rs`,
  `backend/src/api/update.rs`, `frontend/src/lib/update/check.ts`]
- **Pre-releases.** Every release so far is a pre-release, and
  GitHub's "latest release" lookup skips pre-releases, so the setting
  "Include pre-releases in update check" defaults to on; with it off
  the notifier finds nothing. There is no setting that turns the check
  off. [code: `backend/src/config/mod.rs`; `docs/PRIVACY.md`, "Update
  check"] [release: `GET /repos/JoaoPucci/ani-gui/releases/latest`
  answers 404]
- **Platform.** A change for 0.18.0, not yet merged, makes the
  notifier offer only a release that carries a package for the
  running platform: on Linux an asset ending `-x86_64.AppImage` or
  `-amd64.deb`, on Windows one ending `-x64-setup.exe`. It exists
  because releases do not always carry all three: 0.12.1 shipped a
  Windows installer alone, and 0.1.0 shipped only the two Linux
  packages. [release] An updater has the same need, one level
  finer: it must pick the *package*, not just the platform, because an
  AppImage must never be offered a `.deb`.
- **What a release carries.** Three assets, uploaded by hand: for
  0.17.0, `ani-gui-0.17.0-amd64.deb` (124 MB),
  `ani-gui-0.17.0-x86_64.AppImage` (151 MB) and
  `ani-gui-0.17.0-x64-setup.exe` (105 MB). No `latest.yml`,
  `latest-linux.yml`, `.blockmap`, `.zsync` or signature file. GitHub
  reports a SHA-256 `digest` for each asset. [release]
- **How packages are built.** By hand, on two machines: `pnpm
  package:release` on x86_64 Linux builds the AppImage and the `.deb`;
  `pnpm package:win` on x64 Windows builds the NSIS installer. No CI
  publishes anything. [code: `docs/development.md`, "Build for
  distribution"] The electron-builder config has no `publish` section,
  and none is inferred: electron-builder falls back to a GitHub
  provider when it finds repository information — a `repository`
  field, a CI variable, or `.git/config` in the project directory —
  and `electron/` offers none. (A `GH_TOKEN` or `GITHUB_TOKEN` in the
  build environment also triggers it.) So it writes no update metadata and
  puts no `app-update.yml` in the package. [source: electron-builder
  25.1.8, `src/publish/PublishManager.ts`,
  `src/util/repositoryInfo.ts`] [code: a built
  `linux-unpacked/resources/` holds no such file]
- **The AppImage is rebuilt after electron-builder.**
  `electron/scripts/repack-appimage.mjs` extracts electron-builder's
  AppImage, patches its `AppRun` to pass `--no-sandbox`, re-packs the
  tree with `mksquashfs`, and prepends the type2 runtime. The output is
  exactly runtime + squashfs; anything electron-builder had appended
  to its file is gone. [code]
- **The Windows installer** is assisted (`oneClick: false`), lets the
  user change the install directory, and has `perMachine: false`. Its
  `installer.nsh` downloads ffmpeg (about 80 MB) during install unless
  one is on `PATH`, and its uninstall step deletes that ffmpeg —
  including during the silent uninstall electron-builder runs before
  an upgrade. [code: `electron/package.json`,
  `electron/build-resources/installer.nsh`] It is not code-signed: the
  0.17.0 installer's PE security directory is empty. [tried: read
  from the first 4 KB of the published file]

## 2. Per-package mechanics

### AppImage

An AppImage is one file the user put somewhere and runs. Updating it
means putting a newer file in its place. Three ways exist.

**electron-updater's AppImage updater.** [source: electron-updater
6.8.11, `out/AppImageUpdater.js`, read from the npm package]

- It works only when the `APPIMAGE` environment variable is set, which
  the type2 runtime sets to the file's path. [source:
  `AppImage/type2-runtime`, `src/runtime/runtime.c`, `setenv("APPIMAGE",
  …)`]
- It downloads the new AppImage, preferring a *differential* download:
  it reads a block map embedded at the end of the running file (its
  last four bytes give the map's size) and at the end of the new one
  (whose size the metadata file gives as `blockMapSize`), and fetches
  only the changed ranges. If that fails it falls back to the whole
  file. [source: `out/differentialDownloader/FileWithEmbeddedBlockMapDifferentialDownloader.js`]
- It installs by `unlink`ing the running file and `mv`ing the new one
  into the same directory. If the old file name contains a version
  number, the new file keeps *its own* name (`ani-gui-0.18.0-…`) and an
  `appimage-filename-updated` event fires, so any launcher entry that
  pointed at the old path now points at nothing. [source:
  `AppImageUpdater.js`, `doInstall`]

What breaks here: the repack. electron-updater checks the download
against the SHA-512 in `latest-linux.yml`, which electron-builder
computes over *its* AppImage, and reads a block map electron-builder
appended to that file. The repack replaces the file with one whose
hash differs and whose tail carries no block map. [code] [source]
Both would have to be rebuilt by the repack: hash and size rewritten
in `latest-linux.yml`, a block map regenerated and appended.
[inference] That is doable, but it puts electron-builder's internal
file formats — the embedded block map and the metadata file — inside
a script this repository maintains, and keeps them in step with every
electron-builder upgrade. [inference]

**AppImageUpdate and zsync.** The AppImage format's own mechanism: an
update-information string embedded in the runtime's `.upd_info` ELF
section, and a `.zsync` file published beside the AppImage, which
tools such as AppImageUpdate read to fetch only changed blocks.
[source: <https://github.com/AppImage/AppImageSpec/blob/master/draft.md>,
"Update information"] The type2 runtime reserves the section and
prints it on `--appimage-updateinformation`. [source:
`type2-runtime/src/runtime/runtime.c`] The deferred-work entry
"Update information in the AppImage" already lists what this would
take; two things are new:

- The pre-release question that entry left open has an answer. The
  spec defines `latest-pre` and `latest-all` beside `latest`, and
  upstream AppImageUpdate implements both by reading the
  `/releases` list from GitHub's REST API. [source:
  AppImageCommunity/AppImageUpdate,
  `src/updateinformation/GithubReleasesZsyncUpdateInformation.cpp`]
  So `gh-releases-zsync|JoaoPucci|ani-gui|latest-all|ani-gui-*-x86_64.AppImage.zsync`
  would find a pre-release. [inference]
- That code takes the newest release and fails if none of its assets
  matches the pattern. A Windows-only release like 0.12.1 would make
  every AppImage's update fail until the next Linux release. [source:
  same file] [inference]

This route helps people who update AppImages with external tools —
AppImageUpdate, and AppImage managers that read the same information
[unverified: which ones do]; the app itself would
not use it to update. [proposal] It is independent of everything
else here.

**Hand-rolled replacement.** The backend downloads the new AppImage
into a temporary file *in the same directory* as the running one,
verifies it and marks it executable; at install time the Electron main
process renames it over the old path.
`rename` within one filesystem is atomic: a crash leaves either the
old file or the new one, never half of each. [inference, POSIX
`rename(2)`] The running copy keeps working, since its mounted image
holds the old file open. [inference] [unverified: not tested against
the type2 runtime's FUSE mount] The path stays the same, so launcher
entries keep working; the file name may then carry the old version
number, which is cosmetic. Relaunch runs the path in `APPIMAGE`
again — not Electron's own `execPath`, which points inside the old
mount. [inference]

`APPIMAGE` alone does not prove the app is an AppImage: child
processes inherit it, so a `.deb` install or an unpacked build started
from a launcher or terminal that is itself an AppImage sees *that*
program's path, and an updater trusting the variable would rename
ani-gui over someone else's file. The check has to be positive: the
type2 runtime also sets `APPDIR` to its mount point [source:
`runtime.c`, `setenv("APPDIR", …)`], and Electron's `execPath` must
lie under that mount, and the file at `APPIMAGE` must be the image
mounted there. [proposal] electron-updater trusts the variable alone.
[source: `AppImageUpdater.js`, `isUpdaterActive`]

**Where it cannot work.** All three routes write to the AppImage's
directory. When the user cannot — the file sits in `/opt` or another
root-owned place, on a read-only mount, or in a sandbox — the update
must fail before it downloads anything, and the dialog falls back to
today's "Open releases page". [proposal] A pre-flight check is a
write test in that directory (create and remove a temporary file),
not a permission-bit reading, because ACLs and read-only mounts do
not show in the bits. [inference]

**The AppImageHub catalog.** The catalog's entry for the app is the
repository's URL; the catalog reads the AppImage from the GitHub
releases and does not host it. [source: `data/ani-gui` in
AppImage/appimage.github.io] Its test launches
the AppImage under `firejail --net=none` and waits for a window, so an
update check there fails for lack of network; the check is already
silent on failure and must stay so. Its warning about missing update
information goes away only with the zsync route above.

Recommendation for the AppImage: **hand-rolled replacement**, in the
app, with the zsync route as an optional extra for other tools later.
[proposal]

### `.deb`

A `.deb` install lives in `/opt/ani-gui`, owned by root, with a
`postinst` that sets the Chromium sandbox helper setuid and links
`/usr/local/bin/ani-gui`. [code:
`electron/build-resources/deb-after-install.sh`] Updating it needs
root, every time.

**Download and run `dpkg` through `pkexec`.** This is what
electron-updater's `.deb` updater does: it takes the first of
`gksudo`, `kdesudo`, `pkexec` or `beesu` that is installed, else
`sudo`, and through it runs `/bin/bash -c` with `dpkg -i` on the
downloaded file; if that fails it runs `apt-get install -f -y`. (A
fallback to `apt install -y --allow-unauthenticated
--allow-downgrades --allow-change-held-packages <file>` exists for a
system without `dpkg`, which an apt system never is.) [source: electron-updater
6.8.11, `out/DebUpdater.js`, `out/LinuxUpdater.js`] The electron-builder
docs list `.deb` among the auto-updatable targets and note that it
"always elevate[s]". [source:
<https://www.electron.build/docs/features/auto-update>]

Against `AGENTS.md` §11 — whose spirit is that a change to the system
is surfaced and approved, never silent — the polkit prompt *is* an
approval, but of something the user cannot read: a root shell running
a command line the app built, followed, if `dpkg` fails, by `apt-get
install -f -y`, which may install or *remove* other packages to settle
dependencies without asking. A desktop with `pkexec` installed but no
polkit agent running gets `pkexec --disable-internal-agent`, which
fails. [source: `out/LinuxUpdater.js`, `determineSudoCommand`,
`sudoWithArgs`] [inference: what `-f -y` may remove]

**Download and hand it to the system installer.** The app downloads
the `.deb` to the Downloads folder and opens it with the desktop's
handler (`xdg-open`), which on most desktops is a software center or
a package installer that shows its own confirmation. [inference] Which
program that is varies by desktop and release, and some have none for
local `.deb` files. [unverified] The app does nothing privileged, and
the user sees a familiar installer. The saving over today is one
download step.

**An apt repository.** The way Linux users expect third-party
software to update: the user adds a repository once, and `apt
upgrade` and the desktop's update tool keep the app current. It needs
a signed repository — an `InRelease` or `Release` + `Release.gpg`
signed with a key the project holds, listing SHA-256 or SHA-512
hashes [source: <https://wiki.debian.org/DebianRepository/Format>] —
and a one-time setup the user runs as root: the key goes in
`/etc/apt/keyrings`, and an entry with `signed-by` in
`/etc/apt/sources.list.d`. [source:
<https://wiki.debian.org/DebianRepository/UseThirdParty>] Hosting is
the hard part:

- GitHub Pages blocks files over 100 MiB in the repository it
  publishes from, and the 0.17.0 `.deb` is 118 MiB; a site is also
  capped at 1 GB, about eight releases. [source:
  <https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github>,
  <https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>]
  Git LFS files are not served by Pages. [unverified: community
  reports only]
- A `Packages` file's `Filename` is relative to the repository's
  base, and apt follows HTTP redirects by default, so a small server
  could answer with a redirect to the GitHub release asset. [source:
  <https://wiki.debian.org/DebianRepository/Format>;
  <https://manpages.debian.org/bookworm/apt/apt-transport-http.1.en.html>]
  GitHub Pages cannot send such redirects [unverified], so this needs
  a real server, such as a Cloudflare Worker.
- Hosted services: Cloudsmith hosts open-source projects free (at
  least 50 GB stored and 200 GB delivered) with attribution [source:
  <https://docs.cloudsmith.com/resources/open-source-hosting-policy>];
  packagecloud's free plan is 2 GB stored and 10 GB of bandwidth,
  too small for 124 MB packages [source:
  <https://packagecloud.io/pricing>].

**Leave it to the package manager.** The notifier keeps saying a new
version exists and links to the release, as now.

Recommendation for `.deb`: **no in-app install.** Keep the notifier,
and let the dialog offer "Download the .deb" (into Downloads, then
"Show in folder") next to "Open releases page" so the user saves one
step and installs with the tool they trust. An apt repository is the
right long-term answer for `.deb` users and is worth its own proposal;
it is not an in-app feature. [proposal]

### Windows (NSIS)

The installer is assisted (`oneClick: false`) with `perMachine:
false`, which in electron-builder means the installer *asks* whether
to install for the current user or for all users. [source:
<https://www.electron.build/docs/nsis>, `perMachine`: "Whether to
show install mode installer page (choice per-machine or per-user) for
assisted installer"] So both kinds of install exist in the wild.

- **Per-user** (the default directory is
  `%LOCALAPPDATA%\Programs\ani-gui`): the user owns the files, and a
  newer installer replaces them without a UAC prompt. [source:
  electron-builder 25.1.8, `templates/nsis/multiUser.nsh`,
  `templates/nsis/installer.nsi`: `RequestExecutionLevel user` unless
  installing for all users]
- **Per-machine** (under Program Files): every update needs
  administrator rights. A silent upgrade of a per-machine install
  raises UAC itself: "If we're running a silent upgrade of a
  per-machine installation, elevate so extracting the new app will
  succeed." [source: `templates/nsis/installer.nsi` at
  electron-builder 25.1.8] There is no update that avoids the prompt;
  the app can only say it is coming.
- **"Only for me" into a folder only administrators can write**, such
  as one under Program Files picked on the directory page: the
  directory page has no elevation hook, so the installer runs without
  the rights to write there. [source: same templates] How that fails
  is [unverified]. An update of such an install would run into the
  same wall.

The app has to know which kind it is before it offers anything. The
installer records the install under the current user's registry hive
or the machine's, depending on the mode (`InstallLocation` under its
uninstall key) [source: electron-builder 25.1.8,
`templates/nsis/include/installer.nsh`, `registryAddInstallInfo`
writes to `SHELL_CONTEXT`], so: a machine-wide entry means a
per-machine install, which updates with a UAC prompt; a per-user entry
means a per-user install, which gets a write test in its directory
and, if that fails, falls back to "Open releases page". [proposal]
Electron has no registry API, so this is a `reg query` from the main
process or a read from the backend. [inference]

**How electron-updater installs on Windows.** It runs the downloaded
installer with `--updated`, adding `/S` for a silent install and
`--force-run` to start the app afterwards. `quitAndInstall` shows the
installer's window unless asked for silence (`isSilent` defaults to
false); the install it makes on quit (`autoInstallOnAppQuit`, default
on) is silent and does not relaunch.
Downloads are kept under `%LOCALAPPDATA%\ani-gui-updater\pending\`.
[source: electron-updater 6.3.9 (the line paired with
electron-builder 25.1.8), `src/NsisUpdater.ts`, `src/BaseUpdater.ts`,
`src/AppUpdater.ts`, `src/DownloadedUpdateHelper.ts`] For a
differential download it needs the `.blockmap` of both the old and
the new installer on the release, and a copy of the old installer,
which electron-builder's NSIS template keeps at
`%LOCALAPPDATA%\ani-gui-updater\installer.exe`; without them it falls
back to the whole file. [source: `src/AppUpdater.ts` at 6.3.9;
`templates/nsis/include/installer.nsh` at 25.1.8] [unverified:
whether today's installers already write that copy]

**What the app's own installer script does on an update.** The
update replaces the install directory, and with it the ffmpeg the
first install downloaded; the `customInstall` macro then runs again —
electron-builder does not skip it on updates — and downloads ffmpeg
again (about 80 MB) unless one is on `PATH`. In a silent install the
download shows no window: the inetc plug-in "recognizes Installer's
Silent mode and in this case hides any output". [source:
electron-builder 25.1.8, `templates/nsis/installSection.nsh`,
`templates/nsis/uninstaller.nsh`; <https://nsis.sourceforge.io/Inetc_plug-in>;
code: `electron/build-resources/installer.nsh`] The same happens today
when a user runs a newer installer by hand. An updater makes it
frequent and invisible: each update is the installer's 105 MB plus
ffmpeg's 80 MB, and an update made offline after the download ends
with no ffmpeg. electron-builder offers `${isUpdated}` to tell an
update from a fresh install [source:
<https://www.electron.build/docs/nsis>], which is set when the
installer runs with `--updated` [source: electron-builder 25.1.8,
`src/targets/nsis/nsisScriptGenerator.ts`], but the old copy is gone
by the time `customInstall` runs. The fix has to keep the old copy
through the update: set it aside before the old directory is removed
and put it back under `${isUpdated}`, or keep it outside the install
directory. The app's per-user data folder does not fit the second: a
per-machine install serves several users, and an installer elevated
with another account resolves that folder to the administrator's
profile. Either way the uninstall step's cleanup of ffmpeg moves with
it. [proposal; which hook runs early enough is unverified]

**SmartScreen.** See section 4: an installer the app downloads and
starts itself is not expected to meet the "Windows protected your PC"
screen, which a browser download of the same unsigned file does.

**The build machine.** Whatever the route, the installer is built on
the Windows machine and must reach the release before it is published
(section 6).

**Two ways to install, from simple to complete:**

1. **Run the installer with its own window.** The app downloads and
   verifies the installer, the user clicks "Restart to update", the
   app quits and starts the installer normally. The user sees the
   installer they already know, which keeps their choice of per-user
   or per-machine and their directory, asks for UAC itself when it
   needs to, and offers to start the app at the end. [proposal]
2. **Silent install** (`/S --updated --force-run`), which is
   electron-updater's install on quit and an option of its
   `quitAndInstall`: no installer window, but a UAC prompt that
   arrives with no window to explain it on per-machine installs, and a
   failure that has nowhere to show. [inference]

Recommendation for Windows: **in-app download, then the installer's
own window** (way 1) first; silent install later, once the first has
been used across a few releases.

## 3. Libraries and approaches

### electron-updater

electron-builder's companion library: npm's `latest` tag is 6.8.11
(electron-builder 26.18.0); 7.0 is in alpha with electron-builder 27.
[source: npm registry, `npm view electron-updater dist-tags` and
`electron-builder dist-tags`] The repository uses electron-builder
25.1.8 and Electron 33.4.11, so adopting it current would also mean
moving electron-builder a major version. [code: `pnpm-lock.yaml`]

It supports NSIS on Windows and AppImage, `.deb`, `.rpm` and pacman on
Linux. [source: <https://www.electron.build/docs/features/auto-update>;
`out/main.js` picks the `.deb` updater when the package's
`resources/package-type` file says `deb`] It needs a `publish`
configuration so electron-builder writes `app-update.yml` into the
package and `latest.yml` / `latest-linux.yml` beside the artifacts,
and those files uploaded to every release. [source: same page]

How its GitHub provider finds a release matters most here, because
every release is a pre-release. [source: electron-updater 6.8.11,
`out/providers/GitHubProvider.js`]

- With `allowPrerelease` off (the default for a version without a
  pre-release suffix like `0.17.0`) it asks for the latest *full*
  release, which this repository has never had — the same dead end
  the notifier's setting describes. [source: `getLatestTagName`;
  `AppUpdater.d.ts`]
- With it on, and no channel set, it reads
  `https://github.com/<owner>/<repo>/releases.atom` and takes **the
  first entry**, whatever it is, then fetches `latest-linux.yml` or
  `latest.yml` from that tag's downloads. [source: `getLatestVersion`]
- That feed lists tags, not only releases. On 2026-11-02 its entries
  were `v0.17.0`, `v0.16.0`, `v0.15.1`, `v0.15.0`, `v0.14.1` — and
  `v0.15.0` is a tag with no release. [release: the feed, and
  `GET /releases/tags/v0.15.0` answering 404] A tag pushed before its
  release is published, a release missing one platform's package, or
  a bare tag at the top of the feed each make the check fail with
  "Cannot find latest-linux.yml" rather than find the previous good
  release. [inference]
- That is the problem the 0.18.0 notifier change solves on the
  backend, solved again differently. The two would disagree: while
  0.12.1 (Windows only) was the newest release, the badge would have
  offered 0.12.0 to a Linux install on 0.11.0, and electron-updater
  would have reported a missing `latest-linux.yml` for 0.12.1.
  [inference]

It can be pointed elsewhere: `setFeedURL` takes a generic URL, so the
backend could pick the release and hand electron-updater that tag's
download directory. [source: `AppUpdater.d.ts`, `setFeedURL`] That
keeps one source of truth for *which* release, at the cost of two
components that each talk to GitHub.

Other things it brings: download progress events, `autoDownload` and
`autoInstallOnAppQuit` (both default on), staged rollouts by
percentage, and a SHA-512 check of every download against the
metadata file. [source: `AppUpdater.d.ts`, `AppUpdater.js`]

### Electron's built-in `autoUpdater`

"Currently, only macOS and Windows are supported", with "no built-in
support for auto-updater on Linux"; on Windows it drives
Squirrel.Windows installers, made with electron-winstaller or Forge's
Squirrel maker, and MSIX packages through a separate updater. [source:
<https://www.electronjs.org/docs/latest/api/auto-updater>] The app
builds an NSIS installer, which Squirrel cannot update, and
electron-builder states "Squirrel.Windows is not supported" for its
own updater. [source:
<https://www.electron.build/docs/features/auto-update>] Using it would
mean replacing the Windows installer and still having nothing for
Linux. Not an option.

### update-electron-app

Electron's small wrapper around the free `update.electronjs.org`
service supports macOS and Windows only, needs Squirrel.Windows
assets ("the update will silently fail" without them), and the service
collects only releases "_not_ marked as draft or pre-release" — which
excludes every ani-gui release. [source:
<https://github.com/electron/update-electron-app>] Not an option.

### Hand-rolled: the backend downloads, the shell installs

The backend already finds the release, already decides it fits the
platform (from 0.18.0), and already owns outbound HTTP. [code] It
would add:

1. **Pick the asset** for the running *package*, not just the
   platform: the AppImage only when the app is positively running
   from one (section 2), the `-x64-setup.exe` on Windows, and nothing
   for anything else — a `.deb` install, recognised by its own marker
   such as the `/opt/ani-gui` path, or an unpacked or development
   build. [proposal]
   Before anything downloads, the backend also checks the install can
   be updated: the write test beside the AppImage, the install mode
   and write test on Windows (section 2). [proposal]
2. **Download** it — the Windows installer into an update folder under
   the app's cache directory, the AppImage into a temporary file beside
   the running one, to keep the final rename atomic — streaming progress to the renderer the way
   downloads already stream theirs, resumable by HTTP range if the
   connection drops. [proposal]
3. **Verify** it before anything uses it (section 4).
4. **Hand over** to the Electron main process, which installs:
   rename-and-relaunch for the AppImage, run-the-installer-and-quit
   for Windows. The install step has to live in the shell, because
   the backend is the shell's child and dies with it. The main process
   accepts only a file the backend staged and verified — in the update
   folder, or the temporary file beside the AppImage — never a path
   the renderer chose. [proposal]

What it does not get for free: differential downloads (each update is
the full 105–151 MB file), staged rollouts, and the years of
edge-case handling in electron-updater's NSIS path. What it gets:
one place that decides which release, no new release assets beyond
a signature (section 6), no change to the repack, no dependency on
electron-builder's metadata formats, and the backend's existing test
layers (`wiremock`, `proptest`). [proposal]

### Comparison

| | electron-updater | Built-in `autoUpdater` | update-electron-app | Hand-rolled |
|---|---|---|---|---|
| AppImage | Yes, but the repack breaks its hash and block map | No | No | Yes |
| `.deb` | Yes, through `pkexec`/`sudo` `dpkg -i` | No | No | Not proposed |
| Windows NSIS | Yes | No (Squirrel or MSIX) | No (Squirrel only) | Yes, by running the installer |
| All-pre-release repository | Only with `allowPrerelease`, newest feed entry wins | — | No | Yes; same rule as the notifier |
| Picks a release that carries this package | No | — | — | Yes |
| New release assets | `latest*.yml`, block maps | — | — | One signature per package |
| Differential download | Yes | — | — | No |

## 4. Integrity and security

An updater is a program that replaces the app with whatever it
downloaded. The question is what it takes for that to be something
the maintainer did not publish.

**The threats, from most to least likely:**

1. A corrupted or truncated download.
2. A network attacker. Ruled out by HTTPS with certificate
   validation, which every request here uses (`reqwest` with rustls
   in the backend). [code: `AGENTS.md` §5] Plain-HTTP fallbacks and
   redirects to non-HTTPS URLs must be refused. [proposal]
3. Someone who gains the power to change the release: a stolen
   GitHub session or token of the maintainer, or a compromise of
   GitHub. Today, whoever can publish a release can replace an asset,
   and every user who clicks the link installs it.
4. A tampered file on the user's own disk between download and
   install. Out of scope: an attacker who can write to the user's
   files can already replace the app.

**What each check stops:**

| Check | Corruption | Network | Release changed by someone else |
|---|---|---|---|
| HTTPS only | — | Yes | No |
| SHA-256 `digest` from GitHub's API | Yes | Yes | No: whoever replaces the asset gets a new digest with it |
| electron-updater's SHA-512 in `latest*.yml` | Yes | Yes | No: the file sits in the same release |
| A signature by a key held only by the maintainer, checked against a public key built into the app | Yes | Yes | **Yes** |
| Windows Authenticode | Yes | Yes | Yes, if the publisher is checked; the installer is unsigned today (below) |

GitHub computes a SHA-256 for every asset on upload and returns it as
`digest` in the release API, which the backend already fetches; it
costs nothing to check, and it catches problem 1. [source:
<https://github.blog/changelog/2025-06-03-releases-now-expose-digests-for-release-assets/>,
<https://docs.github.com/en/rest/releases/assets>] [release: every
0.16.0 and 0.17.0 asset carries `sha256:…`]

GitHub's **immutable releases** narrow problem 3 without any change to
the app: once a release is published, "Git tags cannot be moved" and
"Release assets cannot be modified or deleted". It is a repository
setting, and it applies only to releases published after it is turned
on. [source:
<https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases>,
<https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/preventing-changes-to-your-releases>]
The docs say the pre-release flag can still be changed on an immutable
release, which suggests pre-releases are covered. [inference] 0.17.0
reports `immutable: false`. [release] It stops a published release
being *changed*; it does not stop someone with the same access from
publishing a *new* one.

It is not free. Every asset has to be attached while the release is
still a draft, and nothing can be added or replaced after publishing:
a forgotten package or signature means a new release. [source: the
"preventing changes" page above] Releases since 0.14.0 have had every
asset in place before publishing; earlier ones did not — 0.13.0's
installer arrived nine hours after it was published, and the Windows
installers of 0.9.1 to 0.12.0 were added on 2026-09-01, weeks later.
[release: asset `created_at` against `published_at`]

GitHub's **artifact attestations** prove which workflow built a file;
they are made by GitHub Actions, and nothing here builds in Actions.
[source:
<https://docs.github.com/en/actions/concepts/security/artifact-attestations>]
Not usable without release CI.

Only a signature checked against a key *outside* GitHub answers
problem 3. The proposal is **minisign**: Ed25519 over a BLAKE2b-512
hash of the file, a public key of one line, a signature file with a
*trusted comment* that is covered by the signature. [source:
<https://jedisct1.github.io/minisign/>] The trusted comment carries the
asset's file name and version, and the app checks them, so an old
signed release cannot be passed off as a new one. [proposal] The
notification-center proposal leans towards minisign for its remote
notices as well [source: `notification-center.md`, section 3], so one
verifier (the `minisign-verify` crate) would serve both. They should
not share a *key*: notices would be signed often, and every signing
would handle the key that can replace the app. Two keys, the release
key used once per release, keep a slip with the frequent one from
reaching the installer. [proposal]
The private key lives on the maintainer's machine, never on GitHub;
signing can be a repository script on Node's own Ed25519 and BLAKE2b,
so nothing is installed system-wide. [tried: Node 24's `crypto` signs
and verifies an Ed25519 signature over a `blake2b512` digest;
producing minisign's exact file format from it is untried]

Losing the key is the cost: a release signed with a new key cannot
update installs that carry the old one, and those installs fall back
to "Open releases page" for that one jump. The app should carry a
list of keys, not one, so a planned rotation ships the next key a
release ahead. [proposal]

**Who can publish.** Today, anyone with write access to the
repository can create or edit a release. [unverified: the
repository's roles were not read] With signatures, publishing a
release that updates users also takes the signing key.

**Windows Authenticode and SmartScreen.** The installer is not
signed (section 1). What that means today and with an updater:

- Downloaded in a browser, an unsigned installer meets SmartScreen's
  "Windows protected your PC" and needs "More info" → "Run anyway";
  without a signature, reputation starts from zero for every new
  version. [source:
  <https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation>]
- Downloaded by the app's own HTTP client and started directly, the
  file carries no Mark of the Web, which browsers write and plain HTTP
  clients do not "by-design", so SmartScreen's download check is not
  expected to run. [source: Eric Lawrence,
  <https://textslashplain.com/2016/04/04/downloads-and-the-mark-of-the-web>,
  not a Microsoft document] [unverified on a Windows machine] The
  updater would therefore remove a warning that today protects users
  from unknown files; the minisign check is what takes its place.
- Windows 11's Smart App Control, when it is on, blocks unsigned
  executables without positive reputation wherever they came from.
  [source: the SmartScreen reputation page above] An in-app update
  cannot get around it; the installer fails to start and the app
  falls back to "Open releases page".
- electron-updater checks the installer's Authenticode signature
  against a `publisherName`; with none configured, as for an unsigned
  app, 6.x skips the check. [source: electron-updater 6.3.9 and
  6.8.11, `src/NsisUpdater.ts`] Its next major version, in alpha,
  rejects the update instead unless verification is switched off at
  build time. [source: electron-builder `master`,
  `packages/electron-updater/src/NsisUpdater.ts`; pull requests
  <https://github.com/electron-userland/electron-builder/pull/10416>
  and <https://github.com/electron-userland/electron-builder/pull/10380>]

Getting a signature:

| Option | Cost | Fit |
|---|---|---|
| Azure Artifact Signing (was Trusted Signing) | From $9.99 a month [source: the SmartScreen reputation page above] | Individual developers must be in the United States or Canada; needs a paid Azure subscription. [source: <https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart>, <https://learn.microsoft.com/en-us/azure/artifact-signing/faq>] |
| SignPath Foundation | Free for open source | Binaries "must be built from source code in a verifiable way" — in practice CI, which this project does not have — and the certificate names SignPath Foundation as publisher. [source: <https://signpath.org/terms>] |
| OV certificate from a CA | Paid yearly [unverified: prices not checked] | Since June 2023 the key must live in hardware (a token or HSM). [source: <https://cabforum.org/working-groups/code-signing/requirements/>] |
| EV certificate | More | No longer skips SmartScreen's reputation check. [source: SmartScreen reputation page above] |

Signing is worth doing for the browser download path whatever happens
to the updater, but it is not a precondition for the updater: the
minisign check covers the in-app path. It is an open question
(section 8).

## 5. The experience

The rules, then the screens. All of them are [proposal].

- **Never interrupt playback.** Nothing restarts the app by itself.
  The download waits while an episode plays and starts, or resumes,
  when playback stops; the backend already knows when a media fetch
  went through its proxy in the last thirty seconds, and that is the
  signal. [code: `docs/architecture.md`, "External processes"] An
  update fetches from GitHub, not the stream's host, so it spends
  none of the host's request budget; holding it back is about the
  user's bandwidth, not the host's limits.
- **Ask once, then stay out of the way.** The first click on "Update"
  is the consent for that version: download, verify, and get ready.
  Installing needs a second, explicit click ("Restart to update"), or
  happens when the user quits the app normally.
- **Install on quit is the default, restart now is offered.** On quit
  the app installs and does not start again; on "Restart to update"
  it installs and starts again. On Windows, "installs" means the
  installer's window opens as the app closes (section 2). The dialog's
  "ready" line is the notice; quitting adds no prompt of its own. The
  close guard, which asks only while episodes are downloading [code:
  `electron/main.js`, `maybePromptOnClose`], is left as it is, and an
  update download still running at quit is simply resumed next time.
- **No automatic download unless asked.** A setting "Download updates
  automatically", off by default, lets a user who wants it skip the
  first click. The check itself stays as it is today.
- **Failure leaves the old version untouched.** Download failures keep
  the partial file for a range-resume on the next try; a file that
  fails verification is deleted and never offered again in that
  session; a refused rename reports the reason and offers "Open releases page",
  which is always there as the way out. On Windows the app has quit by
  the time the installer runs, so the installer reports its own
  failures, as it does today.
- **Rollback is reinstalling the previous version.** No updater here
  keeps the old version around. The release notes of a bad release
  say which version to go back to, and the releases page has it.
  Keeping the previous AppImage beside the new one (`.old`) is cheap
  and is an open question (section 8).
- **Offline:** the check fails silently, as now; a download that
  loses the network pauses, shows "Waiting for the network", and
  resumes from where it stopped.
- **Opt-out:** "Install updates from inside the app" in Settings, on
  by default. Off, the dialog behaves as it does today. And "Check for
  updates at launch", also on by default: there is no switch for the
  check today, and an updater is a good moment to add one.

### Where it shows

The badge and its dialog stay the entry point until the notification
center lands; then "Update available" becomes a notice there whose
detail is this dialog, and the badge folds into the bell or stays,
which is the notification center's open question, not this one's.
[proposal; see [notification-center.md](notification-center.md),
"Surface design options"]

Progress does not join the download dock. The dock lists episodes,
with per-episode rows, speed and "Show in folder", and its close guard
counts them; an app update is one item of a different kind with a
different end ("Restart"), and mixing them would make "3 downloads"
ambiguous. The dialog shows the progress, and the badge carries a
small ring while it runs, so the dialog can be closed. [proposal]

### Wireframes

The dialog keeps today's shape — title, versions, release notes,
footer actions — and only the footer and one status line change. The
accent is the app's own, not a per-anime one.

Update found (AppImage or Windows):

```
┌──────────────────────────────────────────────────┐
│  Update available                                │
│  A newer version of ani-gui is available.        │
│                                                  │
│   CURRENT        LATEST                          │
│   0.19.0         0.20.0                          │
│                                                  │
│  ┌────────────────────────────────────────────┐  │
│  │ ## What's new                              │  │
│  │ - …                                        │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
│      Open releases page   [ Close ] [ Update ]   │
└──────────────────────────────────────────────────┘
```

Downloading (the dialog can be closed; the badge shows the ring):

```
│  ███████████████░░░░░░░░░░░  58% · 87 of 151 MB  │
│  4.2 MB/s · about 15 s left                      │
│                                                  │
│                         [ Close ] [ Cancel ]     │
```

Waiting behind playback:

```
│  ███████████████░░░░░░░░░░░  58% · paused        │
│  Waiting until playback stops.                   │
```

Ready, AppImage:

```
│  ✓ 0.20.0 is ready. It installs when you quit,   │
│    or now if you restart.                        │
│                                                  │
│                [ Later ] [ Restart to update ]   │
```

Ready, on Windows (the permission line only for a per-machine
install, section 2):

```
│  ✓ 0.20.0 is ready. The installer opens when you │
│    quit, or now if you restart.                  │
│    Windows will ask for permission to install it.│
│                                                  │
│                [ Later ] [ Restart to update ]   │
```

`.deb` install:

```
│  This copy was installed as a system package.    │
│  Install the update with your package manager,   │
│  or download the .deb and open it.               │
│                                                  │
│  Open releases page  [ Close ] [ Download .deb ] │
```

Failure:

```
│  ! The update couldn't be installed: the folder  │
│    holding ani-gui is read-only.                 │
│    Your current version is unchanged.            │
│                                                  │
│                [ Close ] [ Open releases page ]  │
```

Every string the renderer shows goes through Paraglide in all shipped
locales (nothing here adds a main-process dialog), and the reasons
come from the backend as stable keys (`error.update.*`), as
every other error does. [code: `docs/architecture.md`, error keys]
Motion follows the existing dialog (`fly` in, `fade` backdrop), and
the progress bar does not animate under reduced motion.

## 6. Release-process changes

Today a release is: build on Linux, build on Windows, create a
pre-release with `gh release create --prerelease`, upload three files
by hand. [code: `AGENTS.md` §13, `docs/development.md`] Since 0.14.0
the files have been uploaded to a draft that is published once all
three are there, though neither document says so. [release] What each
route adds:

**Hand-rolled with signatures (recommended):**

- One `.minisig` file per package the app updates:
  `ani-gui-<v>-x86_64.AppImage.minisig` and
  `ani-gui-<v>-x64-setup.exe.minisig`. The `.deb` gets one too, so
  users can check it by hand.
- A signing step, run where the private key is. Since the key should
  live in one place, the simplest order is: the Windows machine builds
  and uploads the installer to the draft release as it does now; the
  maintainer's Linux machine downloads it from the draft, checks its
  SHA-256 against the one the Windows machine printed, signs all three
  packages, and uploads the signatures. One script does it.
  [proposal] The alternative, a second copy of the key on the Windows
  machine, doubles the places it can leak.
- The release stays a **draft** until every package and its signature
  are attached. The notifier already ignores drafts, so publishing
  last keeps an install from being offered a release whose signature
  has not arrived. [code: `backend/src/meta/github.rs`, drafts
  filtered] An install that finds a package without its signature
  treats it as not updatable in the app and offers "Open releases
  page". [proposal]
- No change to electron-builder's config, to the AppImage repack, or
  to the catalog test.

**With electron-builder's metadata (electron-updater):**

- A `publish` section in `electron/package.json` (GitHub provider,
  owner and repo). electron-builder writes the metadata whenever a
  publish configuration exists, whether or not it uploads, so assets
  can still be uploaded by hand. [source: electron-builder 25.1.8,
  `src/publish/PublishManager.ts`]
- Per release: `latest-linux.yml` for the Linux packages, `latest.yml`
  for Windows, and the installer's `.blockmap`; the AppImage's block
  map is inside the file. [source:
  <https://www.electron.build/docs/features/auto-update>]
- The AppImage repack re-appends a block map and rewrites the hash
  and size in `latest-linux.yml` after it repacks (section 2).
- The Windows machine uploads `latest.yml` and the block map with the
  installer; the Linux machine uploads `latest-linux.yml`.
- A tag must never reach GitHub ahead of its release's metadata,
  because the newest feed entry wins (section 3).

**For third-party AppImage tools (zsync, optional):**

- The repack writes the update-information string into the runtime's
  `.upd_info` section, and the build produces
  `ani-gui-<v>-x86_64.AppImage.zsync` with `zsyncmake`, which the
  build would fetch into the repository the way the repack fetches
  the runtime, since installing it on the build machine is a system
  change. [code: `docs/deferred-work.md`, "Update information in the
  AppImage"]
- The catalog's warning about missing update information goes away.

**The catalog test** runs the AppImage offline under `firejail
--net=none`; the update check must fail quietly there, as it already
does. Nothing else in the test changes on any route.

## 7. Privacy

The app has no telemetry, and an updater does not change that, but it
does add requests, so `docs/PRIVACY.md` would say what they reveal.
[code: `docs/PRIVACY.md`, "Update check"]

- **The check** is unchanged: a request to GitHub's API at launch,
  revealing to GitHub the user's IP address, the time, and a
  `User-Agent: ani-gui` header. No account, no installation id, no
  version number is sent. [code: `backend/src/meta/github.rs`]
- **The download** reveals the same to GitHub and its download host,
  plus which package was fetched — that is, roughly which platform
  the user runs and that they chose to update. A resumed download is
  several requests for the same file.
- **Nothing is learned that a browser fetching the same file would
  not reveal**, apart from the `User-Agent: ani-gui` that says the
  request came from the app. The difference from today is who sends
  it: the app, after the user clicks Update, instead of the browser.
- **What electron-updater would add**, if chosen: a persistent
  per-install identifier. It creates a random UUID for staged
  rollouts, stores it in the app's data folder as `.updaterId`, and
  sets it as an `x-user-staging-id` header on its provider's requests
  — so every check would carry a stable id GitHub could use to
  recognise the same install across IP addresses. [source:
  electron-updater 6.8.11, `out/AppUpdater.js`,
  `getOrCreateStagingUserId` and `getUpdateInfoAndProvider`] That is
  the kind of identifier the privacy policy promises the app does not
  send, so adopting the library would mean overwriting the header —
  the app's own `requestHeaders` are merged over it, so a fixed value
  replaces the id [source: `computeFinalHeaders`], though the file is
  still created — or accepting and documenting it. Its GitHub check
  also reads the `releases.atom` feed from `github.com` and a metadata
  file from the release downloads, one more request than today.
  [source: `GitHubProvider.js`]
- **A signature check** happens on the machine; it sends nothing.

## 8. Recommendation and plan

**Per package:**

- **AppImage — update in the app.** Download, verify, rename over the
  running file, restart on the user's click or install on quit. The
  user owns the file, so nothing privileged happens. Where the folder
  cannot be written, say so and offer the releases page.
- **Windows — update in the app, through the installer's own
  window.** Download and verify in the app; on "Restart to update"
  the app quits and starts the installer, which handles per-user and
  per-machine installs, and UAC, as it does today. A silent install
  can follow once this has run across a few releases.
- **`.deb` — no in-app install; it stays with the system package
  manager.** Installing a system package from an app means running
  `dpkg` as root on the app's say-so, which is the wrong shape for a
  package the system owns. The dialog offers to download the `.deb`
  into Downloads and show it, beside the releases page. An apt
  repository is the real answer for `.deb` users, and needs its own
  proposal: it is hosting, signing and a one-time root setup, not an
  app feature.
- **Approach: hand-rolled, not electron-updater.** The backend
  already picks the release and, from 0.18.0, picks only one that
  carries a package for the platform; electron-updater would pick
  again by a different rule (the newest entry in the releases feed,
  bare tags included), break on the AppImage repack, need new
  metadata files on every release from both build machines, and send
  a per-install id on every check. Its strengths — differential
  downloads and years of NSIS edge cases handled — matter less than
  one source of truth and a privacy policy that stays true.
- **Integrity: minisign signatures** checked against keys built into
  the app, plus GitHub's SHA-256 digest, plus immutable releases.

The first in-app update a user can take is the one *after* the
release that ships the updater: installs of 0.19.0 and earlier keep
the link to the releases page, so the release notes of the first
updater release should say it has to be installed by hand once.

**Phases**, each a pull request on its own. Effort is rough: **S** is
a day or less, **M** a few days, **L** a week or more.

0. **Mockups in the real app** of the dialog states in section 5,
   reviewed by the maintainer before anything is built, together with
   the notification center's, since the update notice lands there. S.
1. **Release signing**: a key pair, a repository script that writes
   minisign signatures with Node's own crypto, the release steps in
   `docs/development.md` (draft until signed), and immutable releases
   turned on. The first release that *needs* signatures is the one
   after the updater ships — the first an updater install can take —
   so signing must be in place by then; doing it first, and signing
   the updater's own release too, rehearses the steps before anything
   depends on them. S–M.
2. **Which package, and is it updatable**: the backend works out the
   package kind with the positive checks of sections 2 and 3 (an
   AppImage only when `APPIMAGE`, `APPDIR` and the running executable
   agree; the installer on Windows; a `.deb` by its own marker;
   anything else not updatable) and runs the pre-flight checks — the
   write test beside the AppImage, the install mode and write test on
   Windows. It returns the asset, its signature and digest, or the
   reason this install cannot update (a `.deb`, an unwritable folder,
   a missing signature, an unrecognised install) as a stable key.
   Builds on the 0.18.0 notifier change; `wiremock` tests and a
   `proptest` for the pure selection. M.
3. **Download and verify** in the backend: the staging places of
   section 3 (the update folder, or the temporary file beside the
   AppImage), progress
   streamed to the renderer, resume by range after a dropped
   connection, a pause while playback is live, cancel, then the
   digest and the signature (with its trusted comment's file name and
   version) checked before the file is offered. M.
4. **Install, on both platforms at once** (`AGENTS.md` §15): the
   Electron main process accepts only a file the backend reports as
   staged and verified; on Linux it renames it over the AppImage and
   relaunches through `APPIMAGE`; on Windows it quits and starts the
   installer. Both happen on "Restart to update" or on a normal quit.
   Tests in
   `electron/lib/*.test.cjs`, an acceptance test for the flow, and a
   manual check on a Windows machine, since no CI builds the
   installer. M–L.
5. **The dialog and settings**: the states in section 5, the badge's
   ring, "Install updates from inside the app", "Download updates
   automatically" and "Check for updates at launch" (with its line in
   `docs/PRIVACY.md`), every string in all shipped locales, `vitest` for
   the state machine extracted from the component, and an acceptance
   test. M.
6. **ffmpeg survives a Windows update**: keep the ffmpeg the
   installer fetched through an update (section 2, for per-user and
   per-machine installs alike), with the uninstall step's cleanup to
   match. Lands with or before phase 4. S–M.
7. **`.deb`: "Download .deb"**, saving to Downloads and revealing it.
   S.

Not planned for 0.20.0: a silent Windows install; update information
and `.zsync` for third-party AppImage tools; an apt repository;
Authenticode signing. Each has an entry in `docs/deferred-work.md`
("Update information in the AppImage", which stays, with this
proposal's findings, and "What the in-app updater leaves out").

**Open questions for the maintainer**, each with a recommended answer:

1. **Hand-rolled, or electron-updater?** Recommended: hand-rolled, for
   the reasons above.
2. **One signing key for updates and remote notices, or two?**
   Recommended: two, with one verifier (section 4) — the release key
   kept on the maintainer's Linux machine only and used once per
   release, the Windows build signed there after upload.
3. **Turn on immutable releases now?** Recommended: yes. It applies
   only to releases published after it is on, and it fits how releases
   have been made since 0.14.0 (every asset on the draft before
   publishing); the cost is that a forgotten file then needs a new
   release, and that installers can no longer be added to old
   releases, as happened for 0.9.1 to 0.12.0.
4. **When an AppImage is replaced, keep its path or rename it to the
   new version's file name?** Recommended: keep the path, so launcher
   entries and integrations keep working; the old version number in
   the name is cosmetic.
5. **Keep the previous AppImage beside the new one for rollback?**
   Recommended: no; it doubles 150 MB on disk, and the releases page
   has every version.
6. **Download automatically?** Recommended: off by default, a setting
   for those who want it.
7. **Windows: the installer's window, or silent?** Recommended: the
   window first; revisit silent after a few releases.
8. **`.deb`: a "Download .deb" button, or only the link?** Recommended:
   the button. And an apt repository: a later proposal of its own,
   probably on a hosted service, since GitHub Pages cannot hold the
   packages.
9. **Authenticode signing?** Recommended: not now. The updater does
   not need it; the cheap options do not fit a hand-built installer by
   an individual outside the United States and Canada [inference: the
   maintainer's location was not checked]. Revisit if releases move to
   CI, which would also open SignPath and artifact attestations.
10. **A switch to turn the update check off?** There is none today.
    Recommended: add it in phase 5, on by default, and say so in
    `docs/PRIVACY.md`.

## Related work

- [A notification center](notification-center.md), proposed for the
  same release: "Update available" is one of its notices, and its
  signed remote notices would share this proposal's verifier, under a
  key of their own.
- The 0.18.0 notifier change, which offers only releases that carry a
  package for the running platform. Phase 2 builds on it.
- `docs/deferred-work.md`, "Update information in the AppImage": the
  zsync route, for third-party tools.
- `docs/PRIVACY.md`, "Update check": to be extended with the download
  (section 7) when phase 3 lands.

## Requests made for this proposal

On 2026-11-02, all read-only:

- GitHub REST API, unauthenticated or as the maintainer's `gh`
  session: the release list, `releases/latest` (404), the 0.1.0,
  0.15.0 (404), 0.16.0 and 0.17.0 releases with their assets and
  digests; the AppImageUpdate, type2-runtime and AppImage catalog
  sources named above.
- `github.com/JoaoPucci/ani-gui/releases.atom`, once.
- The first 4 KB of the 0.17.0 Windows installer, by HTTP range, to
  read its PE security directory.
- The npm registry: `electron-updater` 6.8.11 unpacked locally and
  read, never run; the version tags of `electron-updater` and
  `electron-builder`.
- The documentation pages cited inline.
