# Proposal: a mini-player window in place of picture-in-picture

**Status**: a study for the 0.21.0 release, where the implementation
is queued. Nothing below is built. The product direction is decided
(an app-owned window, not a canvas, not Document Picture-in-Picture);
the open questions are listed in section 9, each with a recommended
answer. The measurements were taken on 2026-11-02 against Electron
33.4.11, hls.js 1.6.19 and the installed 0.17.0 release; the live
samples describe the provider on that day.

## The problem in plain terms

The player's "Picture-in-picture" menu entry pops the video out into a
small floating window. On anime that is close to useless, because the
subtitles do not come with it: they stay behind in the app's window,
drawn over an empty black box, while the floating window shows a
picture nobody can follow.

That is not a bug the app can fix inside picture-in-picture.
Chromium's picture-in-picture window draws the video element's
picture and nothing else, and the app's subtitles are text tracks on
that element, which Chromium renders in the page, not in the
picture. The newer API that would let an app put its own page in a
floating window, Document Picture-in-Picture, resolves in Electron
but never opens a window.

The maintainer has chosen the way out: an Electron-owned window that
is small, frameless and kept above other windows, carrying the
player itself, with its subtitles and the app's own controls. Two
alternatives are ruled out: drawing the subtitles into the picture
through a canvas ("too error prone"), and waiting for Document
Picture-in-Picture.

This study answers what that window takes: how playback gets into it
and back out, how it behaves as a window on each platform the app
ships for, what it looks like, how it is reached, how it is tested,
and in which pull requests it lands.

Labels used below: **[data]** is something observed in code, a log, a
measurement or a cited document; **[inference]** is a conclusion drawn
from data; **[proposal]** is a design choice; **[unverified]** is a
claim nobody has checked yet, with the check that would settle it.

## 1. What picture-in-picture does today

### The code paths

- **[data]** The custom controls' "more" menu has a
  "Picture-in-picture" item (`frontend/src/routes/play/[id]/+page.svelte:3255`)
  that calls `onPip` (`:650`): `requestPictureInPicture()` on the
  page's video, or `exitPictureInPicture()` when something is already
  in it. Failures are swallowed.
- **[data]** The custom controls are the default
  (`backend/src/config/mod.rs:125`, `use_custom_player_controls: true`).
  The comment at `+page.svelte:281` still says native is the default;
  it predates that change. With the setting off, Chromium's own
  controls show and offer their own picture-in-picture button.
- **[data]** The page owns its video element and releases it when the
  page goes; releasing closes picture-in-picture first when the
  element is in it (`frontend/src/lib/play/player-video.ts:47`). So
  picture-in-picture ends the moment the viewer leaves the player, as
  `docs/architecture.md` ("Leaving the player") says.
- **[data]** Subtitles are text tracks on that element: the tracks
  inside an HLS playlist, which hls.js exposes, and sidecar `.vtt`
  files the provider lists beside the stream, one `<track>` each
  (`frontend/src/lib/play/sidecar-tracks.ts:73`), served by the proxy
  on the stream lane. A pick in the captions menu sets one track to
  `showing` and the rest to `hidden` (`+page.svelte:509`).

### What was there before 0.16.0

Up to 0.15.x the app parked one shared video element outside the page
tree so that it survived navigation, entered picture-in-picture
automatically when the viewer left a playing episode (the
`disable_auto_pip_on_leave` setting turned that off), and told the
floating window's two close buttons apart: its × paused and left the
viewer where they were, its "back to tab" returned to the player
**[data]** (`git show 748d1cdd9 -- docs/architecture.md`, the 0.16.0
change "leaving the player unloads it", #204). That change removed the
shared element and the automatic entry, because the playback that
outlived its page kept error handling and retries wired to a page that
had gone. Its commit message states the rule that replaced it: "there
is no off-route playback". This proposal keeps that rule (section 3).

### The baseline on the current source

**[data]** Measured with the installed 0.17.0 under Xvfb (X11, no
window manager), muted, with the XDG directories in a scratch folder,
driving the app with Playwright's Electron driver. Two shows,
episode 1 of each, streamed from the current provider through the
app's proxy:

| | *Sousou no Frieren* (Kitsu 46474) | *One Piece* (Kitsu 12) |
|---|---|---|
| Stream | HLS, 1920×1080 | HLS, 1440×1080 |
| Subtitle tracks | 9 sidecar, English `showing` (270 cues) | 1 sidecar, English `showing` (285 cues) |
| `requestPictureInPicture()` | resolves; a 384×216 window opens | resolves; a 288×216 window opens |
| Picture in the floating window | yes | yes |
| Subtitles in the floating window | **no** | **no** |
| Where the active cue is drawn | in the main window, over Chromium's "Playing in picture-in-picture" placeholder | the same |
| Frames presented (`requestVideoFrameCallback`, ~3 s) | 77 | 73 |
| Leaving the play page | the floating window closes; no video element is left | the same |

The floating window is an X11 window titled "Picture in picture" with
`_NET_WM_STATE_ABOVE` and `_NET_WM_STATE_STICKY` set. Screenshots of
the screen were taken while a cue was active ("Beyond Journey's End";
"Wealth, fame, power..."): the floating window shows the frame alone,
and the cue sits on the empty player in the main window. So
picture-in-picture works on the current source, and it shows exactly
what the maintainer described: the picture without the subtitles.

### Document Picture-in-Picture

**[data]** electron/electron#39633 ("support new documentPictureInPicture
M116 Feature") is open, last updated 2026-08-07. In an earlier attempt
on Electron 33.4.11, `documentPictureInPicture.requestWindow()`
resolved and no OS window was created. `docs/deferred-work.md` held an
entry to adopt the API once that issue lands; this proposal supersedes
it, and the entry now points here.

## 2. Options considered

- **(a) Composite the subtitles into the picture** (a canvas stream
  with the cues drawn in). Ruled out by the maintainer.
- **(b) Wait for Document Picture-in-Picture.** Ruled out by the
  maintainer; nothing indicates when #39633 lands.
- **(c) Shrink the main window into a mini player.** The one window
  turns small, frameless and on top, showing only the player, and
  grows back. **[inference]** No handover at all, no second renderer,
  no state to carry. But the viewer cannot browse the app while
  watching, and restoring a maximized window is where this app has
  fought the window manager before (`electron/main.js`, the "open
  maximized" notes around `win.maximize()`). Kept as the fallback if
  (d) proves too costly; not recommended.
- **(d) A second window, and playback moves into it.** One video
  element and one hls.js engine at a time; the main window releases
  its player and the mini window opens the same stream at the same
  point. **Recommended**; sections 3–8 describe it.
- **(e) A second window that mirrors the main window's playback**
  (frames streamed across, the main window still decoding). Rejected:
  it decodes twice, the subtitles would have to be carried across
  separately (which is (a) again), and the main window would keep a
  playing player the viewer cannot see.

## 3. Architecture

### Playback moves; it is never mirrored

**[proposal]** When the viewer opens the mini player, the main
window's play page hands over its state and releases its video exactly
as it does when the viewer leaves the page (engine destroyed, element
unloaded). The mini window opens the same stream session at the handed
position. Coming back reverses it. At no moment do two engines play.

This keeps the 0.16.0 rule. Playback still never outlives the page
that owns it: the mini window's page owns it, with its own error
handling, stall recovery and retries, and when that page closes, its
playback ends.

### The mini window is a route of the same app

**[proposal]** The mini window loads `/mini/[id]?session=…&episode=…&kind=…`
from the same SvelteKit build: `app://localhost/mini/…` in a packaged
build, the Vite server in development. **[data]** Nothing new is
needed to serve it: the `app://` handler hands `index.html` to any
extensionless path (`electron/main.js:558`), and SvelteKit's client
router takes the route from there.

The window gets the same preload and the same `additionalArguments`
as the main one (`electron/main.js:401`, `windowArguments(backend, …)`),
so it sees the same backend handshake: `window.aniGui.apiBase`, the
secret and the six loopback lanes. **[data]** The main process already
keeps that handshake for a later window: `keeper.booted(backend)`
records it for the single-instance reopen path
(`electron/lib/single-instance.cjs:70`). Both windows share Electron's
default session, so they share `localStorage` and with it the kept
watch positions (`ani-gui.watch-positions`) and, once #330 lands, the
per-show caption choice (`ani-gui.caption-choices`). They also share
Chromium's six-connection pool per lane: one profile, one network
context.

**[data]** The stream session needs no new resolve. The play page
builds its media URL from the session id and the stream lane
(`+page.svelte:904`), and a proxy session lives four hours
(`backend/src/proxy/token.rs:49`, `DEFAULT_SESSION_TTL`). The mini page
builds the same URL from the same id.

**[proposal]** The mini route must not mount the root layout's app
chrome. `frontend/src/routes/+layout.svelte` holds the top bar, the
rail, the download dock, toasts and the update badge, and on mount it
hydrates accounts and checks for updates (`:231`, `:245`). Two of its
effects would be wrong in a second window:

- **[data]** It pushes the active-download count to the main process on
  every change (`+layout.svelte:77–80`), and the main process keeps
  only the last value it received, from any window
  (`electron/main.js:641–644`). A mini window mounting that layout
  would report 0 and switch off the quit prompt while a download runs
  in the main window. The fix is in two places: the mini route
  renders without that layout (the chrome moves into a route group the
  mini route is outside of), and the main process keeps the count per
  sender and prompts on the sum, so no future window can clobber it.
- **[inference]** An update check and an account renewal per mini
  window would duplicate the main window's.

### One engine per window, owned by the player page

AGENTS.md §6 says hls.js is a singleton inside `Player.svelte`.
**[data]** No `Player.svelte` exists. The engine is constructed in the
play page (`+page.svelte:1798`) and, separately, in the `/play`
diagnostic page that plays a pasted URL
(`frontend/src/routes/play/+page.svelte:35`). The rule's intent holds:
one engine per player surface, never one built ad hoc.

**[proposal]** The play page's attach path (engine, sidecar tracks,
quality control, resume hold, stall recovery, position writing) moves
into a module both routes call. The play page is 5,644 lines; the
pieces already sit in `$lib/play/` (`arm-quality.ts`, `resume-hold.ts`,
`stall-machine.ts`, `sidecar-tracks.ts`, `watch-position.ts`, …), so
the extraction gathers wiring that lives in the page today. The §6
text is amended in the same change to name that module and the
two-window rule: at most one engine plays across the app, except for
the moment of a handover.

### What is handed over

| State | How it travels | Notes |
|---|---|---|
| Position | explicit, in the handover message | **[data]** The kept-position rules would lose it: a point under 15 s reopens at the start, one in the last 90 s is forgotten (`docs/architecture.md`, "Leaving the player"). The handover carries the point the way a stale-stream recovery's pending point does, which already outranks the kept one. |
| Show, episode, session, media kind | the mini URL | The same parameters the play URL carries, including the guess and Continue-row flags (`+page.svelte:193–197`). |
| Subtitle track | explicit: the track's language and label, or "off" | Not the index: the mini page attaches its own `<track>` elements, and their order depends on when the listing lands. #330's per-show memory picks the same track on its own when the viewer chose one, but not for a guessed session, whose pick is not remembered. |
| Volume and mute | explicit | **[data]** Carried today in module state inside one renderer (`player-video.ts:10–11`), which a second window does not share. |
| Playback rate | explicit | Page state today (`+page.svelte:562`). |
| Quality pin | explicit | The level the quality control holds (`$lib/play/quality-pin.ts`), so the mini window does not start a ladder from its setting again. |
| Playing or paused | explicit | |
| Audio | nothing | Sub or dub is a property of the session, chosen at resolve; the app does not switch HLS audio renditions. |

**[proposal]** The main process brokers the handover: the play page
sends `mini:open` with that state; the main process opens or shows the
mini window and forwards it; `mini:return` runs the other way, focuses
the main window, and the main window navigates to the play URL with
the state applied. Positions keep being written to `localStorage` as
they are today, so a crash in the middle of a handover still resumes
from the kept point.

### What a handover costs

**[data]** A throwaway spike (not in the repository) played a
generated 720p HLS stream with a WebVTT track in a main window, handed
it to a 480×270 frameless always-on-top window and back, three times
each way, with a window created at the handover ("cold") and with one
created beforehand and kept hidden ("warm"). The stream was served
locally, without the provider's pacing:

| Where | Main → mini, cold | Main → mini, warm | Mini → main |
|---|---|---|---|
| Xvfb, X11 | 252–268 ms | 82–200 ms | 85–133 ms |
| GNOME Shell 50 (headless), native Wayland | 256–283 ms | 69–194 ms | 89–289 ms |
| GNOME Shell 50 (headless), XWayland | 244–309 ms | 87–205 ms | 79–112 ms |

Times run from the handover request to the first frame the receiving
window presents (`requestVideoFrameCallback`). The first frame landed
within 70 ms of the handed position, with one exception: a point
inside the stream's first fragment first showed the fragment's start
(0.08 s for a point at 3.98 s), the flash the app's resume hold already
hides. A cold window takes about 70–140 ms to load the page. The
receiving window's renderer holds about 220–250 MB of working set
while it plays, and 133 MB while hidden and idle.

On the live source the network dominates. **[data]** In the installed
0.17.0, playing *Frieren* episode 1, a fresh hls.js engine attached in
the same page at 60, 300 and 600 seconds past the playhead, through
the app's proxy, presented its first frame after 1.3, 3.8 and 3.1
seconds (manifest parsed after 0.3–0.7 s). The page's own engine was
paused but still buffering, so this is an upper bound: in a real
handover the main engine is gone first, and the proxy's paced budget
(a burst of twenty, then one request every 1.5 s, `docs/architecture.md`)
is the mini window's alone.

**[proposal]** So a handover will show a gap of about one to four
seconds on the live source. It is made invisible rather than removed:

- The main window captures the current frame before it releases the
  element (10–60 ms and about 45 KB as a JPEG at 720p in the spike),
  and the mini window shows it as a still under the player's loading
  treatment until its own first frame lands at the point. The resume
  hold (`$lib/play/resume-hold.ts`) is that treatment; the engine starts
  at the point as #305 makes it do, so no earlier frame is fetched.
- The mini window is kept warm: created hidden once the play page
  opens, so the page is loaded when the viewer clicks.

Two ways to shorten the gap further, not in the first version: the
mini window could pre-roll at a point a few seconds ahead while the
main window keeps playing, and switch when the main window reaches it
(two engines for a few seconds, against the same host budget); or the
proxy could keep the last fragments it served for a session, so the
mini window's first fragment never reaches the host. Both are open
question 5.

**[data]** One spike finding the implementation must respect: on the
reused (warm) window, a `<track>` kept across `video.load()` showed no
cues on the second and third handover, while one created per attach
always did. The app already attaches sidecar tracks per stream and
removes them when the stream retires (`sidecar-tracks.ts:73–89`); the
mini page keeps doing that.

## 4. The window on each platform

### Electron options

**[proposal]** `frame: false`, `alwaysOnTop: true` (level `floating`),
`resizable: true`, `minWidth: 320`, `minHeight: 180`,
`setAspectRatio(16 / 9)` re-set to the stream's own ratio once its
metadata is in, `skipTaskbar: true` where the platform has it,
`backgroundColor: '#000'`, and **no `parent`**. The page draws its own
drag strip with `-webkit-app-region: drag`, as the main window's title
bar does (`+layout.svelte:1157`).

**[data]** A drag region takes every pointer event
(`+layout.svelte:405` cites electron/electron#1354), so the whole
picture cannot be both draggable and clickable. The drag strip is the
top edge of the chrome; a click on the picture toggles play.

### Linux on X11, and XWayland

**[data]** Under GNOME Shell 50 with the window on XWayland
(`--ozone-platform=x11`), mutter honoured always-on-top: the mini
window carried `_NET_WM_STATE_ABOVE`, and after an ordinary window
opened focused on top of it, the stacking order read main < other <
mini. Minimizing the main window left the mini window visible; a mini
window created with `parent: main` was hidden with it
(`_NET_WM_STATE_HIDDEN`) and marked skip-taskbar, which is why the
proposal sets no parent. `setBounds` placed the window where asked
(clamped below GNOME's top bar). `setAspectRatio` writes the aspect
into `WM_NORMAL_HINTS` (177/100 for 16:9), which window managers that
read size hints honour.

**[data]** Without a window manager (plain Xvfb), Electron's
`alwaysOnTop` does not set `_NET_WM_STATE` at all, while Chromium's own
picture-in-picture window sets it before mapping. So CI under Xvfb
cannot observe always-on-top (section 7).

**[unverified]** Plain X11 sessions (Xfce, KDE on X11, Cinnamon). Any
EWMH window manager should honour the same state; check with the
script in the appendix.

### Linux on Wayland: the critical risk

**[data]** The app runs native Wayland under a Wayland session: the
launcher exports `ELECTRON_OZONE_PLATFORM_HINT=auto` before Electron
starts (`electron/build-resources/after-pack.cjs:45`, comment at
`electron/main.js:73`), and the maintainer's logs show
`--ozone-platform=wayland`.

**[data]** Wayland has no always-on-top for application windows:

- Electron's documentation, after electron/electron#50403 was closed
  by the documentation change #50560 (merged 2026-06-08), says of both
  `win.setAlwaysOnTop` and `win.isAlwaysOnTop`: "Not supported on
  Wayland (Linux)." On that issue an Electron maintainer wrote: "there
  is no concept of 'always on top' in the Wayland protocol. Apps cannot
  see or control their z-order. KDE and some other environments let
  *users* control this value with window rules, but it's at the
  compositor level."
- The same documentation says `getBounds()` returns `x: 0, y: 0` on
  Wayland because "introspecting or programmatically changing the
  global window coordinates is prohibited", and that "minimized" is not
  a supported state there. Electron's Wayland write-up (2026-03-17)
  adds that the compositor, not the app, decides where a window opens.
- **[data]** In the headless GNOME Shell 50, natively on Wayland,
  `isAlwaysOnTop()` returned `true` (the flag, not the compositor's
  state, which is the misleading value #50403 reports), and the mini
  window's bounds read `x: 0, y: 0`. Stacking cannot be read from a
  client, and the headless shell refuses screenshots to unlisted
  callers, so the z-order itself was not observed there.

**[inference]** Chromium's own picture-in-picture is no better on
GNOME Wayland: it is an ordinary Wayland toplevel too. Keeping it buys
nothing there.

**[data]** The Ozone platform is chosen once for the whole Electron
process before any window exists (`after-pack.cjs` explains why it
has to be the launcher's environment: `appendSwitch` runs too late, and
relaunching into the flag crashed). **[inference]** So one window
cannot run on XWayland while the other runs natively. The choices are:

1. **Accept it.** On Wayland the mini window is a small frameless
   window that other windows can cover. GNOME's own window menu
   (Alt+Space) has "Always on Top", applied by the compositor; KDE has
   window rules. **[unverified]** whether GNOME's menu opens on, and
   applies to, Electron's frameless Wayland window: appendix, step 4.
2. **Run the whole app on XWayland**, by a setting that relaunches it
   with the X11 platform. Always-on-top then works, as measured above.
   Costs: a change to how the app launches, which the maintainer
   decides; **[inference]** text that may render blurred under
   fractional scaling, as XWayland clients can; and input methods no
   longer on the Wayland path the launcher enables with
   `--enable-wayland-ime`. Electron 38 removed
   `ELECTRON_OZONE_PLATFORM_HINT` (per the Electron write-up), so the
   launcher's mechanism changes with the next Electron major in any
   case. A viewer can already do this today by exporting
   `ELECTRON_OZONE_PLATFORM_HINT=x11` before launching; the launcher
   honours a value already set (`after-pack.cjs:45`).
3. **A second Electron process for the mini window on XWayland.**
   Rejected: it would need its own profile under the single-instance
   lock (`electron/main.js:116`), so it would not share `localStorage`
   (positions, caption choices), and the handover would cross
   processes.

**[proposal]** Option 1, with a one-time hint in the mini window on
Wayland that names the window menu, if step 4 confirms it. Open
question 1.

### Windows

**[data]** Electron documents `setAlwaysOnTop` levels for Windows; the
default `floating` level stays below the taskbar. `setAspectRatio` and
`-webkit-app-region` are supported. **[unverified]** Everything else:
nobody in the loop runs Windows, and nothing builds the Windows
installer in CI (AGENTS.md §15). Before the window phase merges, the
appendix steps run on a Windows install: on top of other windows,
dragging, resizing at the ratio, Aero Snap, a second monitor at another
scale, minimizing the main window, and the taskbar entry.

**[data]** §15: the mini window uses Electron APIs only. It bundles,
spawns and probes nothing, so neither fetcher changes and
`tests/arch/linux_deps.sh` / `windows_deps.sh` are unaffected.

### Placement memory and monitors

**[proposal]** Remember the mini window's size everywhere and its
position where the platform allows it (X11, XWayland, Windows): the
bounds and the display they were on, kept beside the app's other window
state. On open, a remembered position on a display that is no longer
attached, or one that falls outside every work area, falls back to the
bottom-right corner of the main window's display, 24 px in from the
work area's edges. On Wayland only the size applies; the compositor
places the window. The clamping is a pure function over the displays
`screen.getAllDisplays()` reports, and is unit-tested.

## 5. Controls and design

### What the window shows

**[proposal]** At rest: the picture and the subtitles, nothing else.
The chrome appears on pointer movement over the window or keyboard
focus, and leaves after 2.5 s without either. **[data]** That is the
main player's rule in fullscreen, where the player fills the screen
and hovering cannot tell the chrome to leave
(`$lib/play/fullscreen-idle.ts`, `FULLSCREEN_IDLE_HIDE_MS = 2500`); the
mini window is in the same position. Subtitles are the same native
text-track rendering as in the main player, so whatever the cues look
like there, they look like here.

```
 at rest (480 × 270)
 ┌────────────────────────────────────────────────┐
 │                                                │
 │                                                │
 │                  (the picture)                 │
 │                                                │
 │                                                │
 │        I'll go wherever the wind takes me.     │
 └────────────────────────────────────────────────┘

 pointer over the window
 ┌────────────────────────────────────────────────┐
 │ ░ Frieren · EP 12 ░░░░░░░░░░░░░░░░░░░  ⤢   ✕  │ ← drag strip; back to app; close
 │                                                │
 │                      ❚❚                        │ ← click anywhere on the picture
 │                                                │
 │        I'll go wherever the wind takes me.     │ ← cues lift above the bar
 │ ━━━━━━━━━━━━━●──────────────────────  14:02    │ ← seek, per-show accent
 │ ⏮  ❚❚  ⏭                          CC ▾         │ ← prev / play / next; captions
 └────────────────────────────────────────────────┘

 captions menu (opens upward, clipped to the window)
                                    ┌─────────────┐
                                    │ ✓ English   │
                                    │   Español   │
                                    │   Off       │
                                    └─────────────┘
```

- **[proposal]** The look is the main player's custom controls, cut
  down: the per-show accent on the timeline (`accentFor(id)`,
  `+page.svelte:214`), tabular numerals for the time and the episode
  number, the same icon set. A solid scrim gradient under the controls,
  not frosted glass (AGENTS.md §7). The window has no rounded corners
  or shadow of its own beyond what the platform draws.
- **[proposal]** At the minimum size (320×180) the bottom row keeps
  play, the timeline and CC; previous/next move into the timeline's
  hover.
- **[unverified]** Whether cues stay legible at 320×180. The app does
  not style cues (`docs/deferred-work.md`, "Subtitle presentation"), so
  their size is Chromium's, relative to the video's height. Phase 0
  checks it on real episodes; if they are too small, a `::cue` size for
  the mini window is the smallest fix.
- **[proposal]** The "Skip opening" button appears in the mini window
  during an interval, as in the main player, and auto-skip applies.

### Keyboard

**[proposal]** The main player's map (`$lib/play/keyboard.ts`): Space
plays and pauses, ←/→ seek, ↑/↓ change volume, N/P switch episode,
M mutes. F, which is fullscreen in the main player, returns to the
app. Escape closes an open menu, then nothing: a stray Escape must not
end playback. Focus is visible on every control.

### Reduced motion

**[proposal]** The chrome fades in and out over the shortest duration
(140 ms); under `prefers-reduced-motion` it appears and disappears at
once, through `$lib/motion` from #339. The window's own opening is the
compositor's animation, which the app cannot change on any platform.

### Closing, returning, and the main window

| Action | What happens | Compared with picture-in-picture |
|---|---|---|
| ⤢ "Back to app", or F | Playback moves back into the main window's player, at the point, with the handed state; the main window comes forward (restored if minimized). | The old "back to tab". |
| ✕ Close | Playback stops; the position is kept as today. The main window stays where it is and is not brought forward. | The old ×, which paused and left the viewer in place. |
| Playing an episode in the main window while the mini window plays | The mini window closes first; its position is kept. One engine. | — |
| Closing the main window | As today: the quit prompt if a download runs, then the app quits and the mini window with it. | — |
| Minimizing the main window | The mini window stays. | — |

**[data]** The close prompt is attached to the main window only
(`electron/main.js:495`), so closing the mini window never prompts.
**[proposal]** The quit path's prompt is parented to the focused window
today (`electron/main.js:1001`); it is parented to the main window
instead, so a quit while the mini window has focus does not show the
dialog over a 480-pixel window.

### Episodes from the mini window

**[proposal]** Previous and next, and auto-play at the end of an
episode, work in the mini window through the same switch the play page
uses (a new session resolved on the resolve lane, the same recording
of the watch). When the series ends, the mini window shows the end
state and stays; it does not return to the app by itself.

### The download dock

**[proposal]** The dock stays in the main window. The mini window shows
no download state; a download that finishes or fails shows its toast in
the main window as it does now. Downloads beside playback already yield
to it through the proxy (`docs/architecture.md`), and nothing about that
depends on which window plays.

## 6. Entry points

- **[proposal]** The player's "Picture-in-picture" menu item becomes
  "Mini player" and opens the mini window. A button for it joins the
  control bar's right-hand group, beside fullscreen.
- **[data]** Entering on leaving the player is what 0.15.x did with
  `disable_auto_pip_on_leave`, removed in 0.16.0 with the off-route
  playback (section 1). **[proposal]** Not in the first version:
  leaving the player stops playback, as now. With the mini window the
  question can be asked again without reviving off-route playback,
  since the playback would move to a page that owns it. Open question 7.
- **[proposal]** Chromium's picture-in-picture goes: the menu item is
  replaced, and the element gets `disablePictureInPicture` so the
  native controls stop offering it when "Custom player controls" is
  off. **[inference]** It has no remaining advantage: it shows no
  subtitles, and on Wayland it is no more on top than the mini window.

## 7. Testing

**What tests can prove:**

- **Unit (vitest, with property tests for the pure parts):** the
  handover state (encode, decode, and the track matched by language and
  label, or off), the placement clamp over a set of displays, the
  chrome's auto-hide state machine, the mini key map.
- **Main-process modules (`electron/lib/*.cjs` with their `*.test.cjs`,
  the existing pattern):** a mini-window keeper (one at a time, warm
  reuse, close and return), the active-download count kept per sender,
  and the quit prompt's parent.
- **Acceptance (vitest + MSW):** the mini page attaches the session at
  the handed point with the handed track, volume, rate and level, and
  writes positions; the play page, on "Mini player", sends its state
  and releases its video; returning applies the state.
- **End to end, on the packaged app** (`electron/e2e/`, extending the
  pattern of `lanes.spec.ts`, which already serves a fake upstream
  playlist through the real proxy): with a generated HLS fixture and a
  WebVTT track, open the mini player and assert that a second window
  loads `/mini/…`, the main window has no video element, the mini
  window's video plays within a second of the handed point with a
  `showing` track and an active cue, the window was created frameless
  and on top (its flags, read through `app.evaluate`), "Back to app"
  restores playback in the main window, ✕ closes it and leaves the main
  window where it was, and the quit prompt still sees a running
  download.

**What needs a person:** whether the window actually stays on top,
dragging, resizing at the ratio, monitors, placement memory, and every
Wayland and Windows behaviour. **[data]** Xvfb has no window manager,
under which Electron does not even set the always-on-top state
(section 4). A pull request in the window phase carries the appendix
steps as its manual test section, run on GNOME Wayland, XWayland, and
Windows.

## 8. Plan for 0.21.0

Each phase is one pull request, TDD as AGENTS.md §2 sets it, shippable
behind the previous one. Effort is rough, in working days.

0. **Design (1–2 days).** Mockups of the window at rest, with chrome,
   at the minimum size, with the captions menu, and the Wayland hint,
   built in the real app per AGENTS.md §8 (designer and critic, §7
   constraints verbatim). Reviewed by the maintainer before phase 2's
   interface work.
1. **Extract the player's attach path (3–5 days).** Engine, sidecar
   tracks, quality control, resume hold, stall recovery and positions
   behind one module; the play page becomes its adapter. No behaviour
   change; the existing acceptance cases are the guard. The §6 text is
   amended here.
2. **The window (2–3 days).** The main-process keeper, the `mini:*`
   IPC, warm creation, placement memory, the per-sender download count,
   the quit prompt's parent, and the mini route outside the app chrome.
   Manual steps on Wayland, XWayland and Windows in the PR.
3. **Handover and controls (4–6 days).** The mini page, both
   directions of the handover with the frame still and the resume hold,
   the controls and keyboard, reduced motion, captions carried, the
   menu item and button, Chromium's picture-in-picture removed. The end
   to end case.
4. **Episodes in the mini window (2–3 days).** Previous, next,
   auto-play, the end state, skip buttons.
5. **Platform answers and docs (1–2 days).** The Wayland hint (or the
   setting, per question 1), `docs/architecture.md` ("Leaving the
   player", "Embedded playback") and the README's feature table.

About 13–21 working days in all.

## 9. Open questions for the maintainer

1. **Wayland.** Accept a mini window that is not kept on top under
   native Wayland, with a hint pointing at GNOME's window menu, or add a
   setting that relaunches the app on XWayland? **Recommended:** accept
   it and show the hint (once step 4 of the appendix confirms the menu
   works); no launch change.
2. **What ✕ does.** **Recommended:** stop playback, keep the position,
   leave the main window as it is, as the old × did.
3. **Playing in the main window while the mini window plays.**
   **Recommended:** the mini window closes first; one engine.
4. **Closing the main window while the mini window plays.**
   **Recommended:** the app quits as today, after the download prompt.
   The alternative, the main window hiding while the mini window plays
   on, leaves an app with no visible way back to its library once the
   mini window closes.
5. **The handover gap.** **Recommended:** ship with the frame still
   over a gap of about one to four seconds. Pre-rolling ahead or a
   proxy-side fragment cache can follow if the gap reads badly in
   use.
6. **Chromium's picture-in-picture.** **Recommended:** remove it,
   including from the native controls.
7. **Entering the mini player on leaving the player.**
   **Recommended:** not in the first version; revisit after it ships.
8. **The mini window in the taskbar and Alt+Tab.** **Recommended:**
   skip the taskbar where the platform allows it (X11, Windows); on
   Wayland it is listed like any window.
9. **Native controls.** **Recommended:** the mini window always uses
   the app's own controls, whatever "Custom player controls" says.

## Appendix: manual checks

Save this as `mini-check.cjs` in the repository root. It opens an
ordinary window and a frameless, always-on-top 16:9 window with a red
drag strip; close both to end it.

```js
// Mini-player window check: one ordinary window and one small,
// frameless, always-on-top 16:9 window. Close both to quit.
const { app, BrowserWindow } = require("electron");

const page = (title, body, bg) =>
  "data:text/html," +
  encodeURIComponent(
    `<title>${title}</title><body style="margin:0;height:100vh;background:${bg};` +
      `color:#fff;font:16px sans-serif">${body}</body>`,
  );

app.whenReady().then(() => {
  const main = new BrowserWindow({ width: 900, height: 600, title: "check: main" });
  main.loadURL(page("check: main", "<p style='padding:16px'>Main window</p>", "#203040"));

  const mini = new BrowserWindow({
    width: 480,
    height: 270,
    minWidth: 320,
    minHeight: 180,
    frame: false,
    alwaysOnTop: true,
    title: "check: mini",
  });
  mini.setAspectRatio(16 / 9);
  mini.loadURL(
    page(
      "check: mini",
      "<div style='-webkit-app-region:drag;app-region:drag;height:32px;" +
        "background:#a33;padding:6px'>drag here</div>" +
        "<p style='padding:8px'>Mini window: resize from a corner</p>",
      "#602020",
    ),
  );
  mini.once("ready-to-show", () => {
    console.log(
      `platform=${process.platform} ozone=${app.commandLine.getSwitchValue("ozone-platform") || "default"}` +
        ` electron=${process.versions.electron} isAlwaysOnTop=${mini.isAlwaysOnTop()}`,
    );
  });
  mini.on("resize", () => {
    const [w, h] = mini.getSize();
    console.log(`mini size ${w}x${h} ratio ${(w / h).toFixed(3)}`);
  });
});

app.on("window-all-closed", () => app.quit());
```

Run it from the repository root, after `pnpm install`. The path is
relative to `electron/`, where `pnpm --dir` runs the command.

- GNOME on Wayland, natively: `pnpm --dir electron exec electron --ozone-platform=wayland ../mini-check.cjs`
- The same session, on XWayland: `pnpm --dir electron exec electron --ozone-platform=x11 ../mini-check.cjs`
- Windows (PowerShell or `cmd.exe`): `pnpm --dir electron exec electron ../mini-check.cjs`

Then, for each run:

1. Click another application's window so it overlaps the red window.
   Does the red window stay in front?
2. Drag the red window by its top strip. Does it move? Resize it from a
   corner: does it keep 16:9 (the terminal prints the ratio)?
3. Minimize the blue window. Does the red one stay?
4. Wayland only: focus the red window and press Alt+Space. Is there an
   "Always on Top" entry, and with it ticked, does step 1 now keep the
   red window in front?
5. Windows only: drag the red window to a second monitor with another
   scale, snap it with Win+arrow keys, and note whether it shows in the
   taskbar.

## Experiments and live requests made for this study

All on 2026-11-02, one Electron run at a time:

- **Baseline, live:** two plays through the installed 0.17.0 (*Frieren*
  and *One Piece*, episode 1 each), on a fresh profile: each opened the
  home page and the show page (their metadata and availability
  requests), resolved one play, and streamed under a minute of the
  episode through the app's proxy.
- **Cold-attach cost, live:** one more play of *Frieren* episode 1 and
  three extra engine attaches against its session, at 60, 300 and 600
  seconds past the playhead.
- **Spike, local:** a generated two-minute 720p HLS stream with a
  one-cue-per-second WebVTT track, served on loopback; under Xvfb, and
  under a headless GNOME Shell 50 in a private D-Bus session with its
  own scratch home (native Wayland and XWayland). No live requests.
