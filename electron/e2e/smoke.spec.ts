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
import {
  _electron as electron,
  expect,
  test,
  type ElectronApplication,
} from "@playwright/test";
import { killTree, withColdLaunchRetry } from "../lib/cold-launch.cjs";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { appInfo, emptyHistory, topRated, trending } from "./fixtures/kitsu";

const electronDir = path.resolve(__dirname, "..");
const packagedBinary = path.join(electronDir, "dist/linux-unpacked/ani-gui");

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
 * via `page.route()` after `firstWindow()` resolves race those
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

  const app = await electron.launch({
    executablePath: packagedBinary,
    // `--no-sandbox` because chrome-sandbox in the unpackaged
    // linux-unpacked dir isn't SUID. The .deb's postinst fixes
    // that for installed copies; this test runs against the
    // pre-install artifact. `--mute-audio`: a headless launch still
    // opens the host's audio device, and test runs must not play sound
    // on the desktop of whoever runs them.
    args: ["--no-sandbox", "--mute-audio"],
    env: cleanEnv,
  });
  onLaunch(app);

  const context = app.context();
  await context.route("**/api/app-info", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(appInfo),
    }),
  );
  await context.route("**/api/history", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(emptyHistory),
    }),
  );
  await context.route("**/api/kitsu/trending", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(trending),
    }),
  );
  await context.route("**/api/kitsu/top-rated", (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(topRated),
    }),
  );
  // Block image fetches; the renderer's placeholder takes over.
  await context.route("**/api/image*", (r) => r.fulfill({ status: 503 }));

  const page = await app.firstWindow();
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
  // semantics under `_electron.launch()` and is flaky in CI's
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

// ─── The window stays on the app's own document ───────────────────
//
// The main process refuses every navigation that would leave the app's
// document (lib/window-guard.cjs). Any document the window loads gets
// the preload bridge, so a web page or about:blank the window was sent
// to ran with it. These cases drive renderer-initiated navigations,
// and a drop delivered through Chromium's own drag handling (CDP
// Input.dispatchDragEvent) rather than a synthetic DOM event. Electron
// keeps a drop from navigating by default (`navigateOnDragDrop`); the
// drop case holds that in place.

async function waitForHome(page: import("@playwright/test").Page) {
  await expect(page.getByText(/top rated/i).first()).toBeVisible({
    timeout: 15_000,
  });
}

/** A harmless page on disk, the kind of file a user might drop. */
function droppedPage(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-e2e-drop-"));
  const file = path.join(dir, "dropped.html");
  fs.writeFileSync(file, "<!doctype html><title>dropped</title><h1>dropped page</h1>");
  return file;
}

/** Record shell.openExternal in the main process instead of launching a browser. */
async function stubOpenExternal(app: ElectronApplication) {
  await app.evaluate(({ shell }) => {
    const g = globalThis as unknown as { __opened: string[] };
    g.__opened = [];
    shell.openExternal = async (url: string) => {
      g.__opened.push(url);
    };
  });
}

async function openedExternally(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(() => (globalThis as unknown as { __opened: string[] }).__opened);
}

/** Ask the page to navigate itself; the context may die if it succeeds. */
async function pageNavigatesTo(page: import("@playwright/test").Page, url: string) {
  await page
    .evaluate((u) => {
      window.location.href = u;
    }, url)
    .catch(() => {});
  // A refused navigation leaves no event to wait for; give an allowed
  // one the time it would take to commit.
  await page.waitForTimeout(1_000);
}

test("a navigation off the app's document is refused and the app stays", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    const home = page.url();
    for (const url of [
      `file://${droppedPage()}`,
      "data:text/html,<h1>data page</h1>",
      // Another host on the app's scheme: the protocol handler serves
      // the bundle whatever the host, so this would load the app under
      // a document the bridge was never meant for.
      "app://elsewhere/",
      // about:blank is not here. Chromium commits it without the
      // navigation hooks Electron's will-navigate hangs off, so no
      // main-process guard sees it; it takes the origin of the document
      // that asked for it, which is the app's own, and calls from it
      // across the bridge are refused by the IPC sender check.
    ]) {
      await pageNavigatesTo(page, url);
      expect(page.url(), `navigated to ${url}`).toBe(home);
    }
    // Read the live document rather than through a locator: Playwright
    // counts a refused navigation as one still in flight, and a locator
    // would wait on it until it timed out.
    await expect
      .poll(() => page.evaluate(() => /top rated/i.test(document.body.innerText)))
      .toBe(true);
  } finally {
    await app.close();
  }
});

test("a file or link dropped on the window does not replace the app", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    const home = page.url();
    const file = droppedPage();
    const cdp = await page.context().newCDPSession(page);
    for (const data of [
      {
        items: [{ mimeType: "text/uri-list", data: `file://${file}` }],
        files: [file],
        dragOperationsMask: 1 | 2 | 16,
      },
      {
        items: [{ mimeType: "text/uri-list", data: "https://example.com/dropped" }],
        dragOperationsMask: 1 | 2 | 16,
      },
    ]) {
      for (const type of ["dragEnter", "dragOver", "drop"] as const) {
        await cdp.send("Input.dispatchDragEvent", { type, x: 400, y: 300, data });
      }
      await page.waitForTimeout(1_500);
      expect(page.url()).toBe(home);
    }
    await expect(page.getByText(/top rated/i).first()).toBeVisible();
    await expect(page.getByText("dropped page")).toHaveCount(0);
  } finally {
    await app.close();
  }
});

test("the app's own routes still load in full, and reload", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    // A full-document navigation inside the app — SvelteKit falls back
    // to one on an unknown route or a failed chunk load.
    const loaded = page.waitForEvent("load");
    await pageNavigatesTo(page, "/settings");
    await loaded;
    await expect(page).toHaveURL(/^app:\/\/localhost\/settings\/?$/);
    await expect(page.getByRole("combobox", { name: /locale/i })).toBeVisible();
    await page.reload();
    await expect(page).toHaveURL(/^app:\/\/localhost\/settings\/?$/);
    await expect(page.getByRole("combobox", { name: /locale/i })).toBeVisible();
  } finally {
    await app.close();
  }
});

test("a web link opens in the system browser and the window stays", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    await stubOpenExternal(app);
    const home = page.url();
    // A link that asks for a new window, like the app's own external links.
    await page.evaluate(() => {
      const a = document.createElement("a");
      a.href = "https://example.com/new-window";
      a.target = "_blank";
      a.rel = "noreferrer";
      a.textContent = "external";
      a.id = "e2e-external";
      document.body.append(a);
    });
    await page.locator("#e2e-external").click();
    // And a navigation of the window itself to a web page.
    await pageNavigatesTo(page, "https://example.com/same-window");
    await expect
      .poll(() => openedExternally(app))
      .toEqual(["https://example.com/new-window", "https://example.com/same-window"]);
    expect(page.url()).toBe(home);
  } finally {
    await app.close();
  }
});
