"use strict";

// Chromium/Ozone command-line flags that make Electron run natively on
// Wayland instead of through the XWayland bridge. Under a Wayland session
// Electron defaults to XWayland (the X11 compatibility layer), whose
// input/compositor path accumulates UI latency — the picker/search
// sluggishness that vanished on a native X11 session.
//
// CRITICAL: `ozone-platform-hint` is consumed during Electron's *early* Ozone
// init, before main.js executes. `app.commandLine.appendSwitch("ozone-...")`
// therefore lands too late and is silently ignored — the app stays on
// XWayland. The flag has to be present in the process's real argv at launch.
// We can't control argv uniformly across the dev script, the AppImage AppRun,
// and the .deb launcher, so main.js guarantees it with a one-time relaunch:
// when the hint is missing on Linux, re-exec ourselves with it appended.
//
// `ozone-platform-hint=auto` is session-aware (Wayland under a Wayland
// session, X11 otherwise) so X11 users are unaffected; `enable-wayland-ime`
// routes text input through the Wayland input method. Linux-only; an empty
// list elsewhere so the caller is a no-op on macOS/Windows.
const OZONE_HINT_FLAG = "--ozone-platform-hint=auto";
const WAYLAND_IME_FLAG = "--enable-wayland-ime";

// The Wayland launch flags as real command-line arguments. Empty off Linux.
function waylandLaunchArgs(platform) {
  if (platform !== "linux") return [];
  return [OZONE_HINT_FLAG, WAYLAND_IME_FLAG];
}

// True when the env indicates a Wayland session — WAYLAND_DISPLAY is the
// canonical signal (set by the compositor), with XDG_SESSION_TYPE as a
// fallback. We gate the relaunch on this so X11 sessions are entirely
// unaffected: the session-aware hint would resolve back to X11 there anyway,
// so re-execing buys nothing but a wasted restart.
function isWaylandSession(env) {
  if (!env) return false;
  return Boolean(env.WAYLAND_DISPLAY) || env.XDG_SESSION_TYPE === "wayland";
}

// Decide whether the current process must relaunch to apply the Wayland flags,
// returning the args to append (empty = no relaunch needed). Returns [] when:
//   - not on Linux (the flags are a no-op there);
//   - not a Wayland session (X11 stays X11 at zero cost);
//   - the ozone hint is already in argv — this is the relaunch-loop guard, so
//     the re-exec'd process sees its own appended flag and stops;
//   - ELECTRON_OZONE_PLATFORM_HINT is exported — Electron honors it natively,
//     so we're already on the right backend.
function waylandRelaunchArgs(platform, argv, env) {
  const args = waylandLaunchArgs(platform);
  if (args.length === 0) return [];
  if (!isWaylandSession(env)) return [];
  if (argv.some((a) => a.startsWith("--ozone-platform-hint"))) return [];
  if (env && env.ELECTRON_OZONE_PLATFORM_HINT) return [];
  return args;
}

module.exports = { waylandLaunchArgs, waylandRelaunchArgs };
