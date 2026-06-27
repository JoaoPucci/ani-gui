// Electron main process for ani-gui.
//
// Responsibilities:
//   1. Spawn the Rust sidecar (`ani-gui-backend`) and parse its stdout
//      to learn the localhost port it bound to.
//   2. Create a BrowserWindow whose preload script injects that URL
//      into `window.aniGui.apiBase` so the SvelteKit renderer can
//      `fetch()` against it.
//   3. Forward app lifecycle events to the sidecar — kill the
//      backend when the window closes so we don't leak a process.
//
// In dev (ELECTRON_DEV=1), points the BrowserWindow at the Vite dev
// server (default http://localhost:5173). In packaged builds, loads
// the static SvelteKit bundle from disk. M-E4 wires the packaged
// path; for M-E3 the dev path is enough to verify the wiring.

"use strict";

const {
  app,
  BrowserWindow,
  Menu,
  dialog,
  ipcMain,
  net,
  protocol,
  safeStorage,
  screen,
  shell,
} = require("electron");
const { spawn } = require("node:child_process");
const path = require("node:path");
const os = require("node:os");
const fs = require("node:fs");
const { pathToFileURL } = require("node:url");
const { extractLocaleFromToml } = require("./lib/extract-locale-from-toml.cjs");
const { isDevProfile } = require("./lib/dev-profile.cjs");
const { startOAuthServer } = require("./oauth-server");

const IS_DEV = process.env.ELECTRON_DEV === "1";
const VITE_DEV_URL = process.env.VITE_DEV_URL || "http://localhost:5173";

// Dev builds run against a separate data profile so they never read or
// migrate the installed app's data. The backend resolves its config /
// cache / state (incl. metadata.sqlite) under `ani-gui-dev` when
// ANI_GUI_DEV is set; spawnBackend inherits process.env, so exporting
// it here is enough. The Electron-owned userData (tokens, Chromium
// profile) is relocated below via app.setName.
if (IS_DEV) {
  process.env.ANI_GUI_DEV = "1";
}

// Native Wayland is selected by `ELECTRON_OZONE_PLATFORM_HINT=auto`, exported
// by the launcher (the .deb/AppImage wrapper written in afterPack, and the dev
// script) BEFORE Electron's early Ozone init — there is no in-process knob for
// it (app.commandLine.appendSwitch lands too late). We used to app.relaunch()
// into `--ozone-platform-hint=auto` when the flag was missing, but on a Wayland
// session that re-exec (the parent inits on XWayland, then re-execs into
// Wayland) crashed the child with a fatal Ozone CHECK — SIGTRAP, 100% on
// GNOME/ibus. Selecting the platform once, pre-init via the env hint, is the
// fix and keeps a single clean process.

// Pin the X11 WM_CLASS / Wayland app_id so GNOME matches the running
// window to our `.desktop` entry's `StartupWMClass=ani-gui`. Without
// this, the dock falls back to a generic icon — the .desktop's
// `Icon=` line is only used in the app grid, not for live-window
// matching. Must be set before app.whenReady().
//
// Under the dev profile we use a distinct name so Electron's userData
// (tokens + Chromium profile) and config reads land in `…/ani-gui-dev`,
// keeping dev runs off the installed app's profile — the counterpart to
// the backend's ANI_GUI_DEV data-dir switch above. Derived from BOTH
// signals (see isDevProfile): a packaged build launched with
// ANI_GUI_DEV=1 has ELECTRON_DEV unset but must still align with the
// backend, or locale + OAuth token paths leak across the boundary.
const APP_NAME = isDevProfile(process.env) ? "ani-gui-dev" : "ani-gui";
app.setName(APP_NAME);
process.title = APP_NAME;

// Custom scheme used in packaged builds to serve the SvelteKit
// static bundle. Loading the index.html via plain `file://` works
// for the root document but breaks SvelteKit's chunk graph — the
// runtime does dynamic `import('/_app/foo.js')` against the page's
// origin, which under `file://` resolves to filesystem-root nonsense.
// A custom scheme gives us a real origin we control, so we can
// resolve `/_app/...` against the bundle dir and SPA-fallback any
// other path to index.html.
const APP_SCHEME = "app";
const APP_ORIGIN = `${APP_SCHEME}://localhost`;

// Register the scheme as standard + secure BEFORE app.whenReady, so
// fetch + service-worker-style guarantees apply to assets loaded
// through it.
protocol.registerSchemesAsPrivileged([
  {
    scheme: APP_SCHEME,
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

/**
 * Locate the compiled Rust backend binary.
 *
 * In dev we look in the cargo target dir (release first, then debug).
 * In packaged builds electron-builder copies the binary into
 * `process.resourcesPath/ani-gui-backend` via `extraResources` in
 * `package.json:build`. Throws with a clear message if missing.
 */
function resolveBackendBinary() {
  // Windows binaries carry the .exe suffix; the rest of the platforms
  // ship the bare binary name. Cargo writes the suffix automatically
  // when targeting a Windows triple, and electron-builder copies
  // whichever shape it finds.
  const exeSuffix = process.platform === "win32" ? ".exe" : "";
  const binaryName = `ani-gui-backend${exeSuffix}`;
  if (IS_DEV) {
    const repoRoot = path.resolve(__dirname, "..", "..");
    // Debug first in dev: cargo's debug profile is what `cargo
    // build` and `cargo test` produce by default, so it stays fresh
    // as the user iterates. The release binary at
    // `target/release/` only updates when something explicitly
    // runs `cargo build --release` and was a recurring footgun:
    // `pnpm dev` would silently pick up a stale release binary
    // from a packaging run hours earlier.
    const candidates = [
      path.join(repoRoot, "gui", "backend", "target", "debug", binaryName),
      path.join(repoRoot, "gui", "backend", "target", "release", binaryName),
    ];
    for (const p of candidates) {
      if (fs.existsSync(p)) return p;
    }
    throw new Error(
      `${binaryName} not found. Build it first:\n  ` +
        `cd gui/backend && cargo build --bin ani-gui-backend`,
    );
  }
  const packaged = path.join(process.resourcesPath, binaryName);
  if (!fs.existsSync(packaged)) {
    throw new Error(
      `${binaryName} not found in packaged resources at ${packaged}. ` +
        `The electron-builder \`extraResources\` rule may be misconfigured.`,
    );
  }
  return packaged;
}

/**
 * Locate the user's config.toml using the same path the Rust backend
 * writes to. Mirrors `directories-next`'s ProjectDirs resolution
 * (`net.thirdmovement.<app>`) so we read the same file the user's
 * Settings page persists to.
 *
 * `<app>` is `APP_NAME` — `ani-gui-dev` in dev so we read the dev
 * profile's config (the backend writes there under `ANI_GUI_DEV`),
 * `ani-gui` otherwise. Without this, a dev session would seed the
 * renderer locale from the installed app's config.
 *
 *   Linux:   $XDG_CONFIG_HOME (or ~/.config) /<app>/config.toml
 *   macOS:   ~/Library/Application Support/net.thirdmovement.<app>/config.toml
 *   Windows: %APPDATA% (or ~/AppData/Roaming) /thirdmovement/<app>/config/config.toml
 *
 * Returns null when the home dir can't be resolved — the only
 * platforms that lack one are headless CI containers, which don't
 * launch Electron anyway.
 */
function resolveUserConfigPath() {
  const home = process.env.HOME || process.env.USERPROFILE || os.homedir();
  if (!home) return null;
  if (process.platform === "linux" || process.platform === "freebsd") {
    const base = process.env.XDG_CONFIG_HOME || path.join(home, ".config");
    return path.join(base, APP_NAME, "config.toml");
  }
  if (process.platform === "darwin") {
    return path.join(
      home,
      "Library",
      "Application Support",
      `net.thirdmovement.${APP_NAME}`,
      "config.toml",
    );
  }
  // win32
  const base = process.env.APPDATA || path.join(home, "AppData", "Roaming");
  return path.join(base, "thirdmovement", APP_NAME, "config", "config.toml");
}

/**
 * Read the user's configured UI locale from config.toml synchronously
 * at app start, so the preload can seed Paraglide's localStorage key
 * before any renderer scripts run. Returns null on any failure
 * (missing file, unreadable, missing key) — the preload falls through
 * to Paraglide's preferredLanguage / baseLocale strategies, matching
 * pre-change behaviour.
 *
 * Synchronous fs is intentional: we need the value in
 * `additionalArguments` before the BrowserWindow is created, and the
 * file is a handful of KB at most.
 */
function readConfigLocale() {
  const p = resolveUserConfigPath();
  if (!p) return null;
  let text;
  try {
    text = fs.readFileSync(p, "utf8");
  } catch (e) {
    // ENOENT on a fresh install is normal — Settings hasn't run yet.
    // Other errors (permissions, decode) get logged but don't fail
    // the launch; the page just boots in English.
    if (e && e.code !== "ENOENT") {
      console.warn("[main] readConfigLocale:", e.message);
    }
    return null;
  }
  return extractLocaleFromToml(text);
}

/**
 * Spawn the backend and resolve once it prints its listening URL.
 * Rejects if the process exits before the URL is observed (so the
 * Electron main process doesn't sit indefinitely on a broken sidecar).
 */
function spawnBackend() {
  return new Promise((resolve, reject) => {
    const bin = resolveBackendBinary();
    // `detached: true` puts the backend in its own process group on
    // POSIX so we can kill the entire group (backend + ani-cli +
    // aria2c + ffmpeg) at quit time via `process.kill(-pid, …)`.
    // Without it, only the Rust process gets the signal and the
    // download grandchildren get reparented to init and keep
    // running. Windows has no process groups; the tree-kill path
    // shells out to taskkill /T instead — see killBackendTree().
    const child = spawn(bin, [], {
      stdio: ["ignore", "pipe", "pipe"],
      detached: process.platform !== "win32",
    });
    let buf = "";
    let resolved = false;
    // Set when the backend emits the ANI_GUI_FATAL bash_missing
    // signal — typically a Windows machine with no Git for Windows
    // install. Promoted to a class of error rather than a generic
    // "backend exited" message so app.whenReady can show a native
    // dialog with a download link instead of just logging.
    let fatalReason = null;

    // Backend prints the renderer-only secret on a separate handshake
    // line right after ANI_GUI_LISTENING. We may see either order on
    // the stdout buffer, so cache one while waiting for the other and
    // only resolve once both are in hand. Used to gate the disconnect-
    // after-expiry cache wipe (Codex P2 #3370011855).
    let pendingApiBase = null;
    let pendingInternalSecret = null;
    const maybeResolve = () => {
      if (resolved) return;
      if (pendingApiBase && pendingInternalSecret) {
        resolved = true;
        resolve({
          child,
          apiBase: pendingApiBase,
          internalSecret: pendingInternalSecret,
        });
      }
    };

    const onLine = (line) => {
      if (resolved) {
        // After handshake, downstream stdout becomes log output;
        // just echo it through so we can see it in dev.
        process.stdout.write(`[backend] ${line}\n`);
        return;
      }
      const apiMatch = line.match(/^ANI_GUI_LISTENING\s+(\S+)/);
      if (apiMatch) {
        pendingApiBase = apiMatch[1];
        maybeResolve();
        return;
      }
      const secretMatch = line.match(/^ANI_GUI_INTERNAL_SECRET\s+(\S+)/);
      if (secretMatch) {
        pendingInternalSecret = secretMatch[1];
        maybeResolve();
      }
    };

    child.stdout.on("data", (chunk) => {
      buf += chunk.toString("utf-8");
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        onLine(line);
      }
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf-8");
      process.stderr.write(`[backend] ${text}`);
      if (text.includes("ANI_GUI_FATAL bash_missing")) {
        fatalReason = "bash_missing";
      }
    });
    child.on("exit", (code, signal) => {
      if (!resolved) {
        const err = new Error(
          `backend exited before handshake (code=${code}, signal=${signal})`,
        );
        err.fatalReason = fatalReason;
        reject(err);
      } else {
        console.error(`[backend] exited (code=${code}, signal=${signal})`);
      }
    });
  });
}

// Surface a Windows-friendly "Install Git for Windows" dialog when
// the backend bails out at startup with the `bash_missing` signal.
// Blocks until the user picks a button; clicking the install option
// opens gitforwindows.org in the default browser. Either way the app
// then quits — there's no recovery without bash.
async function showBashMissingDialog() {
  const result = await dialog.showMessageBox({
    type: "error",
    title: "Git for Windows required",
    message: "ani-gui needs Git for Windows to run.",
    detail:
      "ani-gui drives the upstream ani-cli script via bash, which on Windows is " +
      "shipped as part of Git for Windows. Install it from gitforwindows.org and " +
      "relaunch ani-gui — it'll find bash automatically.\n\n" +
      "(If Git for Windows is already installed, make sure bash.exe is on PATH or " +
      "reachable at the standard install path.)",
    buttons: ["Open download page", "Quit"],
    defaultId: 0,
    cancelId: 1,
    noLink: true,
  });
  if (result.response === 0) {
    await shell.openExternal("https://gitforwindows.org/");
  }
}

let backendChild = null;

/**
 * Kill the backend AND every download grandchild it spawned
 * (ani-cli, aria2c, ffmpeg, yt-dlp). On POSIX we negate the pid to
 * signal the backend's process group — spawnBackend uses
 * `detached: true` so the cascade works. On Windows there are no
 * process groups, so we shell out to taskkill with /T (kill tree).
 *
 * Idempotent — safe to call when the backend has already exited.
 */
function killBackendTree() {
  if (!backendChild || backendChild.killed) return;
  if (process.platform === "win32") {
    // /F = force, /T = include child processes. Fire-and-forget;
    // we don't await it because the close path is already winding
    // down and a stuck taskkill shouldn't block the quit.
    try {
      spawn("taskkill", ["/F", "/T", "/PID", String(backendChild.pid)], {
        stdio: "ignore",
        windowsHide: true,
      });
    } catch (e) {
      console.error("[main] taskkill failed:", e);
    }
    return;
  }
  try {
    // Negative pid = process group. SIGTERM gives the children a
    // chance to clean up; if any survives, the OS reaper will
    // eventually SIGKILL on app shutdown.
    process.kill(-backendChild.pid, "SIGTERM");
  } catch (e) {
    // ESRCH: group already gone (backend exited first). Anything
    // else is unexpected and worth logging.
    if (e && e.code !== "ESRCH") console.error("[main] killBackendTree:", e);
  }
}

async function createWindow(apiBase, internalSecret) {
  // Pre-compute the work area so the window opens at the maximized
  // size in one shot. Setting the constructor width/height to the
  // work-area size avoids the "open at 1280×800, then animate to
  // maximized" flash some WMs (mutter, kwin) animate when you call
  // `win.maximize()` against a smaller initial size. After the
  // window is up we still call `maximize()` so the WM tracks the
  // maximized state — that lets the user un-maximize back to a
  // sensible default and lets a snap shortcut work.
  const { width: workW, height: workH } = screen.getPrimaryDisplay().workAreaSize;
  const win = new BrowserWindow({
    width: workW,
    height: workH,
    minWidth: 960,
    minHeight: 600,
    show: false,
    // Window icon for the dev session — packaged builds get it
    // from the .desktop file electron-builder generates from
    // build-resources/icon.png. Without this, GNOME's window list
    // shows the generic Electron logo while running unpackaged.
    icon: path.join(__dirname, "build-resources", "icon.png"),
    // Frameless: we draw our own titlebar + window controls in the
    // renderer. Under native Wayland/Ozone (the relaunch above), GNOME
    // gives Chromium no server-side decorations, so Chromium drew its
    // own CSD with the window buttons on the LEFT, ignoring GNOME's
    // button-layout (electron/electron#48422). Owning the chrome puts
    // minimize/maximize/close back where Linux users expect them and
    // keeps them consistent across platforms. `resizable` stays true so
    // the OS resize edges still work on a frameless window.
    frame: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      // Pass the resolved apiBase + the user's configured locale
      // into the preload via additional arguments. The preload reads
      // them off process.argv. The locale flag is omitted entirely
      // when config.toml is missing or has no locale key, so the
      // preload falls through to Paraglide's preferredLanguage /
      // baseLocale strategies instead of seeding garbage.
      additionalArguments: (() => {
        const args = [`--ani-gui-api-base=${apiBase}`];
        const locale = readConfigLocale();
        if (locale) args.push(`--ani-gui-locale=${locale}`);
        if (internalSecret) {
          args.push(`--ani-gui-internal-secret=${internalSecret}`);
        }
        return args;
      })(),
    },
  });

  // New-window requests come from three places:
  //   1. `window.open(...)` or `<a target="_blank">` to an external
  //      site (e.g. settings help links, ffmpeg.org from the error
  //      overlay) — route through shell.openExternal so the user's
  //      default browser opens it.
  //   2. Middle-click / ctrl-click / shift-click on any in-app
  //      `<a href="/route">` — Chromium would normally open a new
  //      tab. Electron has no tabs and our renderer is a single-
  //      window app, so allowing this either leaks the dev URL to
  //      the system browser (dev) or spawns a preload-less, broken
  //      BrowserWindow (packaged). Deny.
  //   3. Anything we don't recognise — deny, never spawn windows
  //      we didn't ask for.
  win.webContents.setWindowOpenHandler(({ url }) => {
    let target;
    try {
      target = new URL(url);
    } catch {
      return { action: "deny" };
    }
    if (target.protocol === "http:" || target.protocol === "https:") {
      const currentUrl = win.webContents.getURL();
      let currentOrigin = null;
      try {
        currentOrigin = new URL(currentUrl).origin;
      } catch {
        currentOrigin = null;
      }
      // Same-origin http(s) is the dev server hosting our own
      // routes (http://localhost:5173/...). Treat it as internal —
      // a middle-click on a SvelteKit `<a href="/route">` link
      // shouldn't bounce the URL into the user's system browser.
      if (currentOrigin && target.origin === currentOrigin) {
        return { action: "deny" };
      }
      shell.openExternal(url);
    }
    return { action: "deny" };
  });

  // Renderer-side diagnostics surface in the main process log so we
  // can tell whether the page actually loaded and whether scripts
  // are throwing. Without this, a blank window looks identical to a
  // successful load from the outside.
  win.webContents.on("did-fail-load", (_e, code, desc, url) => {
    console.error(`[renderer] did-fail-load ${url}: ${code} ${desc}`);
  });
  win.webContents.on("console-message", (_e, level, msg, line, source) => {
    const tag = ["log", "warning", "error"][level] || "log";
    console.log(`[renderer:${tag}] ${msg} (${source}:${line})`);
  });

  // "Open maximized" is a two-step belt-and-suspenders pattern.
  //
  // Step 1 — pre-show hint (electron/electron #45815, #834):
  // call `maximize()` synchronously while the window is still
  // hidden, so the underlying surface is created with the maximize
  // hint already set. Most launches end here — the WM honors the
  // hint at map time and the window appears maximized in one shot.
  //
  // Step 2 — post-show fallback: ~1 launch in 5 mutter drops the
  // pre-show hint under load and the window comes up at workArea
  // size but un-flagged. The `show` event fires deterministically
  // *after* the surface is mapped, so re-firing `maximize()` there
  // acts on a fully tracked window and is reliable. We only re-fire
  // if the WM didn't already pick up the hint, so there's no
  // animation on the happy path. Event-driven (NOT setTimeout) —
  // late timers race the WM and were the cause of five earlier
  // failed iterations.
  //
  // The post-show maximize triggers a brief WM-rendered animation
  // on the bad path. We attempted to hide it behind setOpacity(0)
  // → setOpacity(1), but the maximize animation is on the window
  // frame (compositor-rendered), which Electron cannot suppress.
  win.maximize();
  win.once("ready-to-show", () => {
    win.show();
  });
  win.once("show", () => {
    if (!win.isMaximized()) win.maximize();
  });

  // Catch the X-button / window-close path. The before-quit hook
  // below covers Cmd+Q / dock-quit / OS shutdown; both reuse the
  // same guard so wording stays consistent across exit paths.
  win.on("close", (e) => maybePromptOnClose(win, e));

  // Keep the renderer's custom maximize/restore button in sync with the
  // real window state — the user can also maximize via the WM (super+up,
  // double-click the drag strip, tiling), so the button can't rely on
  // its own clicks alone. Frameless windows still emit these.
  const sendMaxState = () => {
    if (!win.isDestroyed()) {
      win.webContents.send("ani-gui:window:maximize-changed", win.isMaximized());
    }
  };
  win.on("maximize", sendMaxState);
  win.on("unmaximize", sendMaxState);

  if (IS_DEV) {
    win.webContents.openDevTools({ mode: "detach" });
    await win.loadURL(VITE_DEV_URL);
  } else {
    // Packaged static SvelteKit bundle, served via the custom
    // `app://` scheme registered above. The bundle's chunks do
    // dynamic imports off the page's origin, so we need a real
    // origin (not `file://`) for them to resolve.
    //
    // Load the root path, not `/index.html` — SvelteKit's client
    // router reads `location.pathname` and treats `/index.html`
    // as a non-route (the app has no `routes/index.html` page).
    // The protocol handler maps `/` to the index.html file.
    await win.loadURL(`${APP_ORIGIN}/`);
  }
  return win;
}

/**
 * Wire `app://localhost/...` to the packaged SvelteKit bundle.
 *
 * Resolution rules:
 *   - `/_app/...` and other extensioned paths → file in the bundle.
 *   - any extensionless path → `index.html` (SvelteKit SPA fallback).
 *   - 404 if the resolved path escapes the bundle dir (defence in
 *     depth — the URL parser shouldn't permit `..` traversal, but
 *     it's cheap to check).
 */
function registerAppProtocol() {
  const bundleDir = path.join(process.resourcesPath, "frontend", "build");
  protocol.handle(APP_SCHEME, async (request) => {
    const url = new URL(request.url);
    let pathname = decodeURIComponent(url.pathname);
    if (!pathname || pathname === "/") pathname = "/index.html";
    const target = path.normalize(path.join(bundleDir, pathname));
    if (!target.startsWith(bundleDir)) {
      return new Response("forbidden", { status: 403 });
    }
    try {
      await fs.promises.access(target);
      return net.fetch(pathToFileURL(target).toString());
    } catch {
      // SPA fallback — anything routerly (no file extension)
      // hands back the index so SvelteKit's client router takes
      // over. A real missing asset (image, css) returns 404.
      if (!path.extname(pathname)) {
        return net.fetch(
          pathToFileURL(path.join(bundleDir, "index.html")).toString(),
        );
      }
      return new Response("not found", { status: 404 });
    }
  });
}

// Window controls for the renderer's custom (frameless) titlebar.
// Each resolves the sender's own window, so they're correct even if a
// second window ever exists. `close` goes through win.close() so the
// existing close-prompt guard (win.on("close", …)) still fires.
ipcMain.on("ani-gui:window:minimize", (event) => {
  BrowserWindow.fromWebContents(event.sender)?.minimize();
});
ipcMain.on("ani-gui:window:toggle-maximize", (event) => {
  const w = BrowserWindow.fromWebContents(event.sender);
  if (!w) return;
  if (w.isMaximized()) w.unmaximize();
  else w.maximize();
});
ipcMain.on("ani-gui:window:close", (event) => {
  BrowserWindow.fromWebContents(event.sender)?.close();
});
// Synchronous so the titlebar can paint the correct maximize/restore
// icon on first render without a flash; the maximize-changed event keeps
// it live afterwards.
ipcMain.on("ani-gui:window:is-maximized", (event) => {
  event.returnValue = Boolean(BrowserWindow.fromWebContents(event.sender)?.isMaximized());
});

// IPC handlers for the renderer's preload bridge. Exposed as
// window.aniGui.pickDirectory() / .pickFile() / .revealInFolder(path).
ipcMain.handle("ani-gui:pick-directory", async (_event, options) => {
  const result = await dialog.showOpenDialog({
    properties: ["openDirectory", "createDirectory"],
    title: options?.title || "Choose download folder",
    defaultPath: options?.defaultPath || undefined,
  });
  if (result.canceled || !result.filePaths?.[0]) return null;
  return result.filePaths[0];
});

// Native file picker — used by the settings page to let the user
// point at an external-player executable that isn't on PATH (typical
// on Windows where mpv.exe is often installed under
// `C:\Program Files\mpv\` outside the system PATH). Mirrors
// pick-directory: returns the absolute path or null on cancel.
ipcMain.handle("ani-gui:pick-file", async (_event, options) => {
  const filters = Array.isArray(options?.filters) ? options.filters : undefined;
  const result = await dialog.showOpenDialog({
    properties: ["openFile"],
    title: options?.title || "Choose file",
    defaultPath: options?.defaultPath || undefined,
    filters,
  });
  if (result.canceled || !result.filePaths?.[0]) return null;
  return result.filePaths[0];
});

ipcMain.handle("ani-gui:reveal-in-folder", async (_event, dirPath) => {
  if (typeof dirPath !== "string" || !dirPath) return false;
  // Use openPath for directories (showItemInFolder targets a file
  // and selects it; for our case the renderer always passes a
  // directory). Returns '' on success per Electron docs.
  const err = await shell.openPath(dirPath);
  return err === "";
});

// Latest active-download count pushed by the renderer (see preload's
// notifyActiveDownloads). Main reads this synchronously at close
// time to decide whether to prompt the user before quitting.
let activeDownloadCount = 0;
// Set true once the user has confirmed "Quit anyway" so the
// follow-up close (which we synthesise via win.close()) skips the
// prompt. Without this we'd recurse.
let confirmedQuit = false;

ipcMain.on("ani-gui:active-downloads", (_event, count) => {
  if (typeof count !== "number" || !Number.isFinite(count) || count < 0) return;
  activeDownloadCount = Math.floor(count);
});

// Synchronous read of the current locale from config.toml. The renderer
// needs this on EVERY load (not just at first window creation) because
// changing the language in Settings triggers a renderer reload but
// leaves main.js running — the locale value baked into
// `webPreferences.additionalArguments` at window-creation time stays
// frozen at whatever config.toml held when Electron first started.
// Re-reading the file here is cheap (config is a handful of KB) and
// happens once per renderer load.
ipcMain.on("ani-gui:read-config-locale", (event) => {
  event.returnValue = readConfigLocale();
});

// ─── Account integration IPC handlers ──────────────────────────────
//
// See .planning/account-integration.md §3.3 (OAuth flow) and §3.4
// (token storage). The Rust backend never touches safeStorage — every
// token operation is mediated by these handlers.

/**
 * Path where encrypted tokens for the given provider live. One file
 * per provider so disconnect can rm a single file cleanly.
 * `app.getPath("userData")` is the OS-canonical per-app data dir:
 *   - Linux: $XDG_CONFIG_HOME/ani-gui (defaults to ~/.config/ani-gui)
 *   - macOS: ~/Library/Application Support/ani-gui
 *   - Windows: %APPDATA%\ani-gui
 */
function tokenPathFor(provider) {
  const dir = path.join(app.getPath("userData"), "tokens");
  // The directory may not exist on first install; mkdir -p.
  fs.mkdirSync(dir, { recursive: true });
  // Allowlist provider slugs to prevent path traversal via a malicious
  // renderer payload (defensive — preload exposes a closed enum, but
  // belt-and-braces).
  if (!/^[a-z]+$/.test(provider)) {
    throw new Error("invalid provider slug");
  }
  return path.join(dir, `${provider}.bin`);
}

// Track the active OAuth attempt so a cancel can stop the server.
let activeOAuth = null;

/**
 * Begin the OAuth flow for the given provider. Opens the consent URL
 * in the OS browser and listens on 127.0.0.1:53682 for the callback.
 * Renderer gets `{ code, state }` back when the user approves, or an
 * error with one of the documented `kind`s.
 */
ipcMain.handle("ani-gui:account:open-oauth", async (_event, { authUrl }) => {
  if (typeof authUrl !== "string" || !authUrl.startsWith("https://")) {
    return { ok: false, kind: "bad_request" };
  }
  // Cancel any previous in-flight attempt — a fresh click should
  // start fresh, not stack on top.
  if (activeOAuth) {
    try {
      activeOAuth.stop();
    } catch {
      /* ignore */
    }
    activeOAuth = null;
  }
  let server;
  try {
    server = startOAuthServer();
  } catch (err) {
    return { ok: false, kind: "port_busy", message: String(err.message || err) };
  }
  activeOAuth = server;
  // Wait for the bind to complete before opening the consent URL.
  // server.listen() is async — the `listening` event fires after
  // server.ready resolves, and only then is the socket accepting.
  // An already-authorised browser profile that redirects immediately
  // would otherwise race the bind and hit ECONNREFUSED. Codex P2
  // #3370057919.
  try {
    await server.ready;
  } catch (err) {
    // Same drain rationale as the openExternal branch below
    // (Codex P2 #3371719725): when server.ready rejects the bind
    // helper also synchronously rejects server.promise; attach a
    // no-op handler so Node doesn't surface an unhandledRejection
    // after we return.
    server.promise.catch(() => {});
    activeOAuth = null;
    const msg = String(err.message || err);
    let kind = "error";
    if (msg.startsWith("port_busy")) kind = "port_busy";
    return { ok: false, kind, message: msg };
  }
  // Codex P2 #3371658225: shell.openExternal returns a Promise — on
  // hosts with no default browser (or where xdg-open / the desktop
  // portal can't dispatch the URL) it rejects, and the prior fire-
  // and-forget call left the OAuth server running while the renderer
  // hung in `connecting` until the 5-minute timeout. Await + handle
  // the rejection so we shut down the listener and surface a launch
  // error immediately.
  try {
    await shell.openExternal(authUrl);
  } catch (err) {
    // Codex P2 #3371719725: server.stop() rejects server.promise with
    // "cancelled". We're about to bail without awaiting it, so attach
    // a no-op drain first — otherwise Node surfaces an
    // unhandledRejection on hosts where openExternal fails (no
    // default browser / xdg-open / portal), which Electron 28+ may
    // upgrade to a process exit. The drain has to land BEFORE stop()
    // so the rejection is never observed without a handler.
    server.promise.catch(() => {});
    server.stop();
    activeOAuth = null;
    return {
      ok: false,
      kind: "browser_launch_failed",
      message: String((err && err.message) || err),
    };
  }
  try {
    const result = await server.promise;
    activeOAuth = null;
    return { ok: true, code: result.code, state: result.state };
  } catch (err) {
    activeOAuth = null;
    const msg = String(err.message || err);
    let kind = "error";
    if (msg.startsWith("port_busy")) kind = "port_busy";
    else if (msg.startsWith("timeout")) kind = "timeout";
    else if (msg.startsWith("cancelled")) kind = "cancelled";
    else if (msg.startsWith("oauth_error")) kind = "oauth_error";
    return { ok: false, kind, message: msg };
  }
});

/** Cancel the active OAuth attempt (renderer pressed cancel). */
ipcMain.handle("ani-gui:account:cancel-oauth", () => {
  if (activeOAuth) {
    try {
      activeOAuth.stop();
    } catch {
      /* ignore */
    }
    activeOAuth = null;
    return true;
  }
  return false;
});

/**
 * Encrypt + persist tokens for the given provider. The JSON body
 * (`{access_token, refresh_token, expires_at_epoch_s, user_id}`) is
 * encrypted via safeStorage and written to a per-provider file.
 *
 * Returns true on success. The renderer should treat any false /
 * thrown result as "fall back to disconnected and tell the user".
 */
/**
 * Codex P2 #3370070913: `safeStorage.isEncryptionAvailable()` returns
 * true even when Electron falls back to its hardcoded `basic_text`
 * backend on Linux installs without libsecret/kwallet/gnome-keyring
 * (or in headless test environments). That backend XORs the payload
 * with a fixed string — recoverable by anyone with read access to the
 * token file — and contradicts the privacy promise that OAuth tokens
 * are encrypted by the OS keychain. Reject persistence in that mode
 * so the renderer surfaces `keychain_unavailable` and the user can
 * install the missing keyring before connecting.
 */
function isRealEncryptionBackend() {
  if (!safeStorage.isEncryptionAvailable()) return false;
  // getSelectedStorageBackend exists on Linux only — on macOS/Windows
  // there's no plaintext fallback to worry about. When the method is
  // absent (other platforms, older Electron), treat encryption as
  // legit because isEncryptionAvailable() already gates it.
  if (typeof safeStorage.getSelectedStorageBackend !== "function") return true;
  const backend = safeStorage.getSelectedStorageBackend();
  return backend !== "basic_text" && backend !== "unknown";
}

ipcMain.handle("ani-gui:account:set-token", async (_event, { provider, payload }) => {
  if (!isRealEncryptionBackend()) {
    return { ok: false, kind: "encryption_unavailable" };
  }
  if (typeof payload !== "object" || payload == null) {
    return { ok: false, kind: "bad_request" };
  }
  try {
    const encrypted = safeStorage.encryptString(JSON.stringify(payload));
    fs.writeFileSync(tokenPathFor(provider), encrypted);
    return { ok: true };
  } catch (err) {
    return { ok: false, kind: "io_error", message: String(err.message || err) };
  }
});

/**
 * Decrypt + return the persisted token payload for the given
 * provider. Returns `{ ok: true, payload }` on success, or an
 * `{ ok: false, kind }` for absent file / decryption failure / etc.
 *
 * Synchronous IPC because the renderer calls this on EVERY backend
 * request that needs a bearer; the round-trip cost matters.
 */
ipcMain.on("ani-gui:account:get-token", (event, provider) => {
  try {
    const p = tokenPathFor(provider);
    if (!fs.existsSync(p)) {
      event.returnValue = { ok: false, kind: "not_found" };
      return;
    }
    if (!isRealEncryptionBackend()) {
      // The token file exists but the OS keychain that wrote it is no
      // longer reachable (or never was). Refuse to decrypt rather than
      // surfacing a plaintext-fallback token — Codex P2 #3370070913.
      event.returnValue = { ok: false, kind: "encryption_unavailable" };
      return;
    }
    const encrypted = fs.readFileSync(p);
    const plain = safeStorage.decryptString(encrypted);
    event.returnValue = { ok: true, payload: JSON.parse(plain) };
  } catch (err) {
    event.returnValue = { ok: false, kind: "decrypt_error", message: String(err.message || err) };
  }
});

/** Delete the persisted token file for the given provider. */
ipcMain.handle("ani-gui:account:clear-token", async (_event, provider) => {
  try {
    const p = tokenPathFor(provider);
    if (fs.existsSync(p)) {
      fs.unlinkSync(p);
    }
    return { ok: true };
  } catch (err) {
    return { ok: false, kind: "io_error", message: String(err.message || err) };
  }
});

/**
 * Open an https URL in the OS browser. Used by the /account page's
 * Privacy Policy link. Only allows https / http schemes — a renderer
 * compromise can't redirect this to a file:// path.
 */
ipcMain.handle("ani-gui:open-external", async (_event, url) => {
  if (typeof url !== "string") return false;
  if (!/^https?:\/\//.test(url)) return false;
  await shell.openExternal(url);
  return true;
});

/**
 * Close-path guard. Called from window 'close' and app 'before-quit';
 * intercepts both X-button and Cmd+Q / dock-quit / OS shutdown. Uses
 * showMessageBoxSync so the close-handler's preventDefault sticks —
 * the async variant returns after the close has already been
 * committed.
 */
function maybePromptOnClose(win, event) {
  if (confirmedQuit) return;
  if (activeDownloadCount <= 0) return;
  event.preventDefault();
  const plural = activeDownloadCount === 1 ? "" : "s";
  const choice = dialog.showMessageBoxSync(win, {
    type: "question",
    buttons: ["Cancel", "Quit anyway"],
    defaultId: 0,
    cancelId: 0,
    title: "Active downloads",
    message: `${activeDownloadCount} download${plural} in progress.`,
    detail: "They will be cancelled if you quit. Continue?",
  });
  if (choice === 1) {
    confirmedQuit = true;
    // Re-trigger the close path. The flag above makes this no-op
    // through the guard so the window actually closes this time.
    if (win && !win.isDestroyed()) win.close();
    else app.quit();
  }
}

app.whenReady().then(async () => {
  try {
    // Drop Electron's default app menu (File / Edit / View / Window /
    // Help) — the in-window topbar + rail are the navigation surface;
    // the platform menu was just adding a strip of system chrome the
    // app doesn't use.
    Menu.setApplicationMenu(null);
    if (!IS_DEV) registerAppProtocol();
    const { child, apiBase, internalSecret } = await spawnBackend();
    backendChild = child;
    await createWindow(apiBase, internalSecret);
  } catch (err) {
    console.error("[main] startup failed:", err);
    // Surface a friendly install dialog when the backend bailed out
    // because Git for Windows wasn't reachable. The bash_missing
    // signal flows up via spawnBackend's rejected error.
    if (err && err.fatalReason === "bash_missing") {
      try {
        await showBashMissingDialog();
      } catch (dialogErr) {
        console.error("[main] bash-missing dialog failed:", dialogErr);
      }
    }
    app.exit(1);
  }
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", (e) => {
  // Prompt on Cmd+Q / dock-quit / OS-shutdown if downloads are
  // active. The window-level `close` handler above covers the
  // X-button path; both reuse the same guard.
  const focused = BrowserWindow.getFocusedWindow();
  const win = focused || BrowserWindow.getAllWindows()[0];
  if (win) maybePromptOnClose(win, e);
  // Tree-kill so ani-cli + aria2c + ffmpeg actually stop. A bare
  // backendChild.kill() only signals the Rust process and orphans
  // the download grandchildren to init.
  killBackendTree();
});

// Re-create a window if the user clicks the dock icon on macOS while
// the app is still running.
app.on("activate", async () => {
  if (BrowserWindow.getAllWindows().length === 0 && backendChild) {
    // Re-derive apiBase from the running backend's known origin.
    // In practice we'd persist this from spawnBackend(); for now,
    // rely on the user to relaunch.
  }
});
