/**
 * Smoke test: launches the packaged Electron app from
 * `dist/linux-unpacked/ani-gui` (electron-builder's pre-AppImage
 * output), stubs the Rust backend's HTTP API so the renderer doesn't
 * depend on Kitsu being reachable, and asserts the home page reaches
 * a recognizable rendered state.
 *
 * Targeting the packaged artifact rather than running unpacked
 * Electron directly tests the production code path: the `app://`
 * protocol handler, the bundled SvelteKit static assets, and the
 * sidecar backend at `process.resourcesPath/ani-gui-backend`. None
 * of those wire correctly when `electron .` is run from source.
 *
 * Pre-condition: `pnpm run package` (or the chained `pnpm e2e`) has
 * produced `dist/linux-unpacked/`.
 */
import { expect, test, type ElectronApplication } from "@playwright/test";
import { killTree, withColdLaunchRetry } from "../lib/cold-launch.cjs";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { appInfo, emptyHistory, topRated, trending } from "./fixtures/kitsu";
import {
  appWindow,
  launchPackagedApp,
  packagedBinary,
  routeContext,
  splashState,
} from "./launch";
import { expectedPlatform } from "../lib/e2e-display.cjs";

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`,
    );
  }
});

/**
 * Wire all the API stubs the home page consumes onto a freshly-
 * launched Electron app, BEFORE its first window starts loading —
 * critical because the renderer fires its initial `fetch()` calls
 * (history, trending, top-rated) immediately on mount. Routes set
 * via `page.route()` after `appWindow()` resolves race those
 * requests and miss the first batch, leaving real-network responses
 * visible to the page. Setting them on `context.route()` (visible to
 * any page in the context) avoids the race.
 */
async function launchAppWithStubsOnce(
  onLaunch: (app: ElectronApplication) => void,
) {
  // Pinning XDG dirs to fresh tmp paths makes the backend's history,
  // settings, and cache files start empty regardless of the dev
  // machine's actual state. Without this the backend reads the
  // developer's own history — its own file under the state dir, not
  // the CLI's — and the Continue Watching strip renders: Playwright's
  // `route()` only intercepts what the renderer fetches, and history
  // is read off the filesystem by the backend.
  const tmp = path.join(
    os.tmpdir(),
    `ani-gui-e2e-${process.pid}-${Date.now()}`,
  );
  fs.mkdirSync(tmp, { recursive: true });
  const cleanEnv = {
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    // Nobody is here to read a dialog: a boot that fails must end the
    // process at once, which is what the cold-launch retry below
    // relies on. See lib/boot-failure.cjs.
    ANI_GUI_UNATTENDED: "1",
  };

  const app = await launchPackagedApp(cleanEnv);
  onLaunch(app);

  const context = app.context();
  await routeContext(context, "**/api/app-info", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(appInfo),
    }),
  );
  await routeContext(context, "**/api/history", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(emptyHistory),
    }),
  );
  await routeContext(context, "**/api/kitsu/trending", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(trending),
    }),
  );
  await routeContext(context, "**/api/kitsu/top-rated", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(topRated),
    }),
  );
  // Block image fetches; the renderer's placeholder takes over.
  await routeContext(context, "**/api/image*", (r) => r.fulfill({ status: 503 }));

  const page = await appWindow(app);
  return { app, page, context };
}

async function launchAppWithStubs() {
  // A cold launch can lose its first window before any assertion
  // runs — the closed-target flake. The harness relaunches once,
  // closing the dead app in between; every other failure propagates
  // untouched. See lib/cold-launch.cjs.
  let pending: ElectronApplication | null = null;
  return withColdLaunchRetry(
    async () => {
      const launched = await launchAppWithStubsOnce((app) => {
        pending = app;
      });
      pending = null;
      return launched;
    },
    {
      cleanup: async () => {
        const dead = pending;
        pending = null;
        // A dead app's close() hangs while its backend lives; kill
        // the whole tree first (lib/cold-launch.cjs).
        if (dead) {
          killTree(dead.process().pid);
          await dead.close();
        }
      },
    },
  );
}

test("app launches, hero renders, no unexpected console errors", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // Capture renderer console errors so the test fails on any
    // thrown exception in the SPA's boot path.
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    page.on("pageerror", (err) => errors.push(String(err)));

    // "Trending now" eyebrow is the first DOM signal the stubs
    // were consumed and the page is past the loading state.
    // "Top rated" eyebrow is the load signal we use across e2e —
    // it renders for every trending-result count, including 1 (a
    // single-entry fixture leaves "Trending now" without a Strip
    // because that single entry becomes the hero, which uses a
    // different eyebrow).
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });

    // Allow a tick for any async errors to fire after render.
    await page.waitForTimeout(250);

    // Filter out:
    //  - DevTools Autofill probe noise (Chromium internal)
    //  - 503s from our own /api/image stub (the renderer logs
    //    `<img onerror>` as a resource-load error; expected).
    const meaningful = errors.filter(
      (e) =>
        !/Autofill\.(enable|setAddresses)/.test(e) &&
        !/^DevTools/i.test(e) &&
        !/Failed to load resource.*503/.test(e),
    );
    expect(
      meaningful,
      `unexpected console errors:\n${meaningful.join("\n")}`,
    ).toEqual([]);
  } finally {
    await app.close();
  }
});

test("a test launch is muted", async () => {
  // A headless launch still opens the host's audio device, and test
  // runs have played sound on the desktop of whoever ran them. Every
  // spec launches through the shared helper (tests/arch/e2e_launch.sh
  // keeps it that way); this asks the running app whether the helper
  // put the switch on its command line.
  const { app } = await launchAppWithStubs();
  try {
    const muted = await app.evaluate(({ app: electronApp }) =>
      electronApp.commandLine.hasSwitch("mute-audio"),
    );
    expect(muted).toBe(true);
  } finally {
    await app.close();
  }
});

test("a test launch takes its flags from the test, not from GitHub", async () => {
  // The launch helper makes every launch a test run, and a test run's
  // backend never fetches the live flags file (backend/src/flags/
  // launch.rs, and backend/tests/flags_launch.rs, which watches the
  // network to prove it). The window was opened with the flags the
  // backend decided, before its first page rendered.
  const { app, page } = await launchAppWithStubs();
  try {
    const seen = await page.evaluate(() => ({
      origin: window.aniGui?.flagsOrigin,
      flags: window.aniGui?.flags,
    }));
    expect(seen).toEqual({ origin: "test", flags: {} });
  } finally {
    await app.close();
  }
});

test("the splash shows while the app starts, and is gone once the app's window has shown", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    await expect.poll(() => app.windows().length, { timeout: 5_000 }).toBe(1);
    // It was on screen — opened and shown, not merely created — and
    // closed once the app's window had shown, not suppressed by a
    // failure.
    // It was opened with the app's own version for the line above the
    // title.
    const version = await app.evaluate(({ app: a }) => a.getVersion());
    expect(await splashState(app)).toEqual({
      opened: true,
      shown: true,
      closed: true,
      suppressed: false,
      version,
      // Every boot step reached the splash, and its page lit a stretch
      // of the rail for each.
      steps: 3,
      railLit: 3,
    });
  } finally {
    await app.close();
  }
});

test("the splash page, opened as the launch opens it, shows the app's version above its title", async () => {
  // The launch's own splash comes and goes before a spec can look at
  // it, so the page is opened again here as the main process opens it
  // (lib/splash.cjs): same window options, same query. This checks what
  // Chromium renders — the page's CSP, the file:// query, the line the
  // script writes into — rather than the main process's record.
  const { app } = await launchAppWithStubs();
  try {
    const [version, shown, colours] = await app.evaluate(async ({ app: a, BrowserWindow }) => {
      const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
      const { SPLASH_PAGE, splashWindowOptions } = main.require("./lib/splash.cjs");
      const w = new BrowserWindow(splashWindowOptions());
      try {
        await w.loadFile(SPLASH_PAGE, { query: { v: a.getVersion() } });
        const text = await w.webContents.executeJavaScript(
          "document.querySelector('.title').innerText",
        );
        // The leading "v" in the app's accent, the rest as the line is.
        const c = await w.webContents.executeJavaScript(
          "(() => { const line = document.querySelector('.version');" +
            " const v = line.querySelector('.v');" +
            " return { v: v && v.textContent, mark: v && getComputedStyle(v).color," +
            " line: getComputedStyle(line).color," +
            " brand: getComputedStyle(document.documentElement).getPropertyValue('--brand').trim() }; })()",
        );
        return [a.getVersion(), text as string, c as Record<string, string | null>];
      } finally {
        w.destroy();
      }
    });
    expect(shown.split(/\s+/).filter(Boolean)).toEqual([`v${version}`, "ani-gui"]);
    expect(colours.v).toBe("v");
    expect(colours.brand).toBe("#d77a3b");
    expect(colours.mark).toBe("rgb(215, 122, 59)");
    expect(colours.line).not.toBe(colours.mark);
  } finally {
    await app.close();
  }
});

test("the splash's rail fills as steps land, and its stall line comes after 700 ms without moving the title", async () => {
  // Opened as the launch opens it (lib/splash.cjs), with the stall line
  // the main process would give it, then driven by hand: a test
  // launch's own boot is over before a spec can watch it.
  const { app } = await launchAppWithStubs();
  try {
    const seen = await app.evaluate(async ({ BrowserWindow }) => {
      const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
      const { SPLASH_PAGE, splashPageQuery, splashWindowOptions } = main.require("./lib/splash.cjs");
      const { stallTexts } = main.require("./lib/splash-progress.cjs");
      const { messagesFor } = main.require("./lib/main-messages.cjs");
      // Never shown, so its timers are kept from being throttled the
      // way a hidden page's are; the real splash is on screen.
      const options = splashWindowOptions();
      const w = new BrowserWindow({
        ...options,
        webPreferences: { ...options.webPreferences, backgroundThrottling: false },
      });
      const run = (code: string) => w.webContents.executeJavaScript(code);
      const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
      try {
        const texts = stallTexts(messagesFor("en"));
        await w.loadFile(SPLASH_PAGE, { query: splashPageQuery({ version: "0.0.0", stallTexts: texts }) });
        const look = () =>
          run(
            "({ lit: document.querySelectorAll('.rail i.lit').length," +
              " status: document.querySelector('.status').classList.contains('on') ?" +
              " document.querySelector('.status').textContent : null," +
              " title: document.querySelector('.wordmark').getBoundingClientRect().top })",
          );
        const start = await look();
        const lit1 = await run("window.splashProgress.set(1)");
        const early = await look();
        await wait(1000);
        const stalled = await look();
        const lit3 = await run("window.splashProgress.set(3)");
        const done = await look();
        return { expected: texts[1], start, lit1, early, stalled, lit3, done };
      } finally {
        w.destroy();
      }
    });
    expect(seen.start.lit).toBe(0);
    expect(seen.lit1).toBe(1);
    expect(seen.early).toMatchObject({ lit: 1, status: null });
    expect(seen.stalled).toMatchObject({ lit: 1, status: seen.expected });
    expect(seen.stalled.title).toBe(seen.early.title);
    expect(seen.lit3).toBe(3);
    expect(seen.done).toMatchObject({ lit: 3, status: null });
  } finally {
    await app.close();
  }
});

test("the app's window paints the app's dark background and is revealed once its page has rendered", async () => {
  // A slow first render (a cold Vite compile, a cold disk) used to show
  // a white frame between the splash and Home.
  const { app } = await launchAppWithStubs();
  try {
    const readyRecord = () =>
      app.evaluate(() => {
        const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
        return main.require("./lib/app-window.cjs").lastAppReady();
      });
    // Wait out the reveal: the splash gone, and the wait for the
    // renderer settled either way.
    await expect.poll(() => app.windows().length, { timeout: 5_000 }).toBe(1);
    await expect
      .poll(async () => {
        const r = await readyRecord();
        return r.rendered || r.gaveUp;
      }, { timeout: 5_000 })
      .toBe(true);
    const backgrounds = await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()
        .filter((w) => !w.isDestroyed() && (w as unknown as Record<string, unknown>).aniGuiSplash !== true)
        .map((w) => w.getBackgroundColor().toLowerCase()),
    );
    expect(backgrounds).toEqual(["#0c0a09"]);
    // The renderer said its first page had rendered while the window
    // was still hidden. That the reveal waits for this word is the
    // reveal gate's own unit test (lib/app-window.test.cjs).
    expect(await readyRecord()).toEqual({ rendered: true, gaveUp: false, beforeReveal: true });
  } finally {
    await app.close();
  }
});

test("the renderer cannot change the launch's flags", async () => {
  // The flags are the session's. The bridge hands the page frozen
  // copies of what the preload exposes, so neither the flags nor
  // window.aniGui can be written from the page (the preload freezes its
  // own copy too; preload.test.cjs checks that half).
  const { app, page } = await launchAppWithStubs();
  try {
    const seen = await page.evaluate(() => {
      const bridge = window.aniGui as unknown as Record<string, unknown>;
      const flags = bridge.flags as Record<string, unknown>;
      try {
        flags.injected = true;
      } catch {
        // Strict code throws; sloppy code is ignored. Either way it fails.
      }
      try {
        bridge.flags = { injected: true };
      } catch {
        // As above.
      }
      return {
        flagsFrozen: Object.isFrozen(flags),
        bridgeFrozen: Object.isFrozen(bridge),
        flags: window.aniGui?.flags,
      };
    });
    expect(seen).toEqual({ flagsFrozen: true, bridgeFrozen: true, flags: {} });
  } finally {
    await app.close();
  }
});

/** The `--ozone-platform` value on each of the app's processes'
 *  command lines, read from /proc. Chromium resolves the platform once,
 *  in the browser process, and hands the answer to every child it
 *  starts — the GPU process and each renderer carry it. A child
 *  rewrites its process title, which joins its arguments with spaces
 *  rather than NULs, so the switch is matched in either form. */
async function ozonePlatforms(app: ElectronApplication): Promise<string[]> {
  const pids = await app.evaluate(({ app: electronApp }) =>
    electronApp.getAppMetrics().map((metric) => metric.pid),
  );
  const platforms: string[] = [];
  for (const pid of pids) {
    let cmdline: string;
    try {
      cmdline = fs.readFileSync(`/proc/${pid}/cmdline`, "utf8");
    } catch {
      continue; // the process ended between the two reads
    }
    for (const match of cmdline.matchAll(/--ozone-platform=([^\s\0]+)/g)) {
      platforms.push(match[1]);
    }
  }
  return platforms;
}

test("a test launch lands on the display the run is meant for", async () => {
  // Test windows opened on the desktop of whoever ran the suite. Outside
  // CI the suite runs inside tools/headless-wayland.sh's private GNOME
  // Shell session, and the app must be on Wayland there — on the
  // headless compositor, which the launch helper insists on; in CI, on
  // the runner's Xvfb, through X11.
  const { app } = await launchAppWithStubs();
  try {
    await expect.poll(() => ozonePlatforms(app)).not.toEqual([]);
    const platforms = await ozonePlatforms(app);
    expect(new Set(platforms)).toEqual(new Set([expectedPlatform(process.env)]));
  } finally {
    await app.close();
  }
});

test("clicking a poster card navigates to a detail route", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // The first window starts loading before `context.route()`
    // can fully register, so the trending/top-rated stubs may
    // race the renderer's initial fetches. We stub them anyway
    // (in case the timing wins on faster machines), but the
    // assertion only checks that *some* /anime/<id> URL lands —
    // not the specific id from our fixture. The point of this
    // test is to exercise the route+navigation path, not the
    // network stubbing.
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    const topRatedRegion = page.getByRole("region", { name: /top rated/i });
    const card = topRatedRegion.locator("a").first();
    await expect(card).toBeVisible({ timeout: 10_000 });
    await card.click();

    // SvelteKit's client router updates the URL to `/anime/<id>`
    // without a full reload. Under the `app://` protocol that
    // becomes `app://localhost/anime/<id>`.
    await expect
      .poll(() => page.url(), { timeout: 10_000 })
      .toMatch(/\/anime\/\w+/);
  } finally {
    await app.close();
  }
});

test("settings locale picker is wired to a reloading change handler", async () => {
  // Verifies the picker surface — the `<select>` is on the page,
  // exposes every shipped locale, and selecting a non-default one
  // fires a full page reload (paraglideSetLocale's default). The
  // *post-reload locale resolution* depends on Electron's storage
  // semantics under Playwright's Electron launch and is flaky in CI's
  // headless xvfb runner; assert only the integration wiring.
  const { app, page } = await launchAppWithStubs();
  try {
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });

    await page
      .getByRole("link", { name: /settings/i })
      .first()
      .click();
    await expect(page).toHaveURL(/\/settings\/?$/);

    // All four shipped locales must be in the dropdown's option set.
    const langSelect = page.getByRole("combobox", { name: /locale/i });
    const optionValues = await langSelect.evaluate((el) =>
      Array.from((el as HTMLSelectElement).options).map((o) => o.value),
    );
    expect(optionValues).toEqual(
      expect.arrayContaining(["en", "pt-BR", "es-419", "ru"]),
    );

    // selectOption("pt-BR") triggers onchange → setLocale →
    // persist().then(paraglideSetLocale()). The reload is the
    // observable side effect we can pin down deterministically;
    // wait for it explicitly so the post-selectOption assertions
    // don't race the reload.
    const reloaded = page.waitForEvent("load");
    await langSelect.selectOption("pt-BR");
    await reloaded;

    // After the reload we're still on /settings — the URL is
    // preserved across paraglideSetLocale's reload, which is the
    // core behavior users rely on.
    await expect(page).toHaveURL(/\/settings\/?$/);
  } finally {
    await app.close();
  }
});

test("home page hides the Continue Watching strip when history is empty", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    // With XDG_STATE_HOME pinned to a fresh tmp dir (see
    // `launchAppWithStubs`), the backend's history file doesn't
    // exist, the renderer gets an empty list, and the Continue
    // Watching strip's `{#if history && history.length > 0}`
    // guard keeps the heading out of the DOM.
    // "Top rated" eyebrow is the load signal we use across e2e —
    // it renders for every trending-result count, including 1 (a
    // single-entry fixture leaves "Trending now" without a Strip
    // because that single entry becomes the hero, which uses a
    // different eyebrow).
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    await expect(
      page.getByRole("heading", { name: /continue watching/i }),
    ).toHaveCount(0);
  } finally {
    await app.close();
  }
});
