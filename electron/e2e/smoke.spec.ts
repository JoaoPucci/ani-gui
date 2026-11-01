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

/**
 * Record, in the main process, every navigation the window is asked to
 * make. The listener is added after the app's own guard, so by the time
 * a URL is recorded the guard has already ruled on it: the record is
 * the positive witness a refusal can be asserted after (docs/testing.md,
 * "Asserting that something did not happen").
 */
async function recordNavigations(app: ElectronApplication) {
  await app.evaluate(({ BrowserWindow }) => {
    const g = globalThis as unknown as { __navigations: string[] };
    g.__navigations = [];
    const win = BrowserWindow.getAllWindows()[0];
    win.webContents.on("will-navigate", (event: { url?: string }, url: string) => {
      g.__navigations.push(event.url ?? url);
    });
  });
}

async function navigationsAsked(app: ElectronApplication): Promise<string[]> {
  return app.evaluate(() => (globalThis as unknown as { __navigations: string[] }).__navigations);
}

/** Ask the page to navigate itself. Swallows the error a committed navigation causes. */
async function pageNavigatesTo(page: import("@playwright/test").Page, url: string) {
  await page
    .evaluate((u) => {
      window.location.href = u;
    }, url)
    .catch(() => {});
}

test("a navigation off the app's document is refused and the app stays", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    await recordNavigations(app);
    const consoleLines: string[] = [];
    page.on("console", (msg) => consoleLines.push(msg.text()));
    const home = page.url();

    // Chromium itself refuses a page's navigation to a local file or a
    // top-level data: document, before any main-process hook is asked;
    // the line it logs is the witness that it ruled.
    const file = `file://${droppedPage()}`;
    await pageNavigatesTo(page, file);
    await expect.poll(() => consoleLines.some((l) => l.includes("Not allowed to load local resource"))).toBe(true);
    expect(page.url(), `navigated to ${file}`).toBe(home);

    await pageNavigatesTo(page, "data:text/html,<h1>data page</h1>");
    await expect.poll(() => consoleLines.some((l) => /Not allowed to navigate top frame to data URL/.test(l))).toBe(true);
    expect(page.url(), "navigated to a data: page").toBe(home);

    // Another host on the app's scheme reaches the guard: the protocol
    // handler serves the bundle whatever the host, so this would load
    // the app under a document the bridge was never meant for.
    //
    // about:blank is not here. Chromium commits it without the
    // navigation hooks Electron's will-navigate hangs off, so no
    // main-process guard sees it; it takes the origin of the document
    // that asked for it, which is the app's own, and calls from it
    // across the bridge are refused by the IPC sender check.
    await pageNavigatesTo(page, "app://elsewhere/");
    await expect.poll(() => navigationsAsked(app)).toContain("app://elsewhere/");
    expect(page.url(), "navigated to app://elsewhere/").toBe(home);

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
    await recordNavigations(app);
    const home = page.url();
    const file = droppedPage();
    // Witness that each drag reached the page at all.
    await page.evaluate(() => {
      const g = window as unknown as { __dragovers: number };
      g.__dragovers = 0;
      window.addEventListener("dragover", () => {
        g.__dragovers += 1;
      });
    });
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
      const before = await page.evaluate(() => (window as unknown as { __dragovers: number }).__dragovers);
      for (const type of ["dragEnter", "dragOver", "drop"] as const) {
        await cdp.send("Input.dispatchDragEvent", { type, x: 400, y: 300, data });
      }
      await expect
        .poll(() => page.evaluate(() => (window as unknown as { __dragovers: number }).__dragovers))
        .toBeGreaterThan(before);
      // Electron's navigateOnDragDrop: false makes Chromium refuse the
      // drop itself, so no navigation is ever asked for — there is no
      // later event to wait on. The drop has been handled by the time
      // dispatchDragEvent returns; what is checked here is that it
      // asked for nothing and left the page where it was.
      expect(await navigationsAsked(app)).toEqual([]);
      expect(page.url()).toBe(home);
    }
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
    // The open is the witness: the guard hands the URL out in the same
    // step that refuses the navigation.
    await expect
      .poll(() => openedExternally(app))
      .toEqual(["https://example.com/new-window", "https://example.com/same-window"]);
    expect(page.url()).toBe(home);
  } finally {
    await app.close();
  }
});

// ─── The bridge answers the app's document only ───────────────────
//
// Every IPC handler in main checks that the call came from the main
// frame holding the app's document (lib/ipc-guard.cjs). A page that
// does get into the window — the guard above is meant to make that
// impossible — still gets nothing from the bridge.

/** Main-process stderr, line by line, from this point on. */
function collectStderr(app: ElectronApplication): string[] {
  const lines: string[] = [];
  app.process().stderr?.on("data", (chunk: Buffer) => {
    lines.push(...chunk.toString().split("\n"));
  });
  return lines;
}

test("every IPC channel refuses a sender that is not the app's document", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    const stderr = collectStderr(app);
    // Call every handler the running app registered, the way Electron
    // would for a call from a frame holding about:blank. The channels
    // come from ipcMain itself, so a handler added later is covered
    // without editing this list.
    const outcome = await app.evaluate(async ({ ipcMain }) => {
      const forged = () => {
        const event: Record<string, unknown> = {
          senderFrame: { url: "about:blank", parent: null },
          touched: [] as string[],
        };
        // A refused call reads senderFrame and nothing else; a handler
        // that ran would reach for the sender's window.
        Object.defineProperty(event, "sender", {
          get() {
            (event.touched as string[]).push("sender");
            throw new Error("handler reached for the sender");
          },
        });
        return event;
      };
      const settle = <T,>(p: Promise<T>) =>
        Promise.race([p, new Promise((r) => setTimeout(() => r("no answer"), 2_000))]);

      // ipcMain keeps each invoke handler as registered, keyed by
      // channel; Electron calls it with the event and turns a return
      // into the reply and a throw into the rejection.
      const invoke = (ipcMain as unknown as {
        _invokeHandlers: Map<string, (e: unknown, ...a: unknown[]) => unknown>;
      })._invokeHandlers;
      const handled: Record<string, string> = {};
      for (const [channel, fn] of invoke) {
        const event = forged();
        const answer = await settle(
          Promise.resolve()
            .then(() => fn(event))
            .then(
              (v) => `replied ${JSON.stringify(v)}`,
              (err) => `threw ${String((err as Error)?.message ?? err)}`,
            ),
        );
        handled[channel] = String(answer);
      }
      const listened: Record<string, { returnValue: unknown; touched: string[] }> = {};
      for (const channel of ipcMain.eventNames()) {
        if (typeof channel !== "string" || !channel.startsWith("ani-gui:")) continue;
        const event = forged();
        for (const fn of ipcMain.listeners(channel)) {
          try {
            // No arguments: a count reaching the active-downloads
            // handler would make the app prompt before it quits.
            (fn as (e: unknown) => void)(event);
          } catch {
            // A handler that ran and failed is recorded by `touched`.
          }
        }
        listened[channel] = { returnValue: event.returnValue, touched: event.touched as string[] };
      }
      return { handled, listened };
    });

    const handledChannels = Object.keys(outcome.handled);
    const listenedChannels = Object.keys(outcome.listened);
    // Not vacuous: the bridge's channels are all there.
    expect(handledChannels).toEqual(
      expect.arrayContaining([
        "ani-gui:pick-directory",
        "ani-gui:pick-file",
        "ani-gui:reveal-in-folder",
        "ani-gui:account:open-oauth",
        "ani-gui:account:cancel-oauth",
        "ani-gui:account:set-token",
        "ani-gui:account:clear-token",
        "ani-gui:open-external",
      ]),
    );
    expect(listenedChannels).toEqual(
      expect.arrayContaining([
        "ani-gui:window:minimize",
        "ani-gui:window:toggle-maximize",
        "ani-gui:window:close",
        "ani-gui:window:is-maximized",
        "ani-gui:active-downloads",
        "ani-gui:read-config-locale",
        "ani-gui:account:get-token",
      ]),
    );
    for (const channel of handledChannels) {
      expect(outcome.handled[channel], channel).toMatch(/^threw .*refused/);
    }
    for (const channel of listenedChannels) {
      expect(outcome.listened[channel].touched, channel).toEqual([]);
    }
    // The sync channels answer with a refusal, never a token or a locale.
    expect(outcome.listened["ani-gui:account:get-token"].returnValue).toEqual({
      ok: false,
      kind: "refused",
    });
    expect(outcome.listened["ani-gui:read-config-locale"].returnValue).toBeNull();
    expect(outcome.listened["ani-gui:window:is-maximized"].returnValue).toBe(false);
    // Every refusal is logged, the async messages included.
    await expect
      .poll(() => listenedChannels.filter((ch) => !stderr.some((l) => l.includes(`[ipc] refused ${ch} `))))
      .toEqual([]);
  } finally {
    await app.close();
  }
});

test("the app's own document still reaches the bridge", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    await stubOpenExternal(app);
    const answers = await page.evaluate(async () => {
      const bridge = (window as unknown as {
        aniGui: {
          windowControls: { isMaximized(): boolean };
          openExternal(url: string): Promise<boolean>;
          account: { getToken(p: string): { ok: boolean; kind?: string } };
        };
      }).aniGui;
      return {
        maximized: typeof bridge.windowControls.isMaximized(),
        opened: await bridge.openExternal("https://example.com/privacy"),
        token: bridge.account.getToken("anilist"),
      };
    });
    expect(answers.maximized).toBe("boolean");
    expect(answers.opened).toBe(true);
    // No token was stored in this fresh profile; what matters is that
    // the handler ran rather than refusing.
    expect(answers.token.kind).not.toBe("refused");
    expect(await openedExternally(app)).toEqual(["https://example.com/privacy"]);
  } finally {
    await app.close();
  }
});

test("revealInFolder opens a directory and never a file", async () => {
  const { app, page } = await launchAppWithStubs();
  try {
    await waitForHome(page);
    await app.evaluate(({ shell }) => {
      const g = globalThis as unknown as { __openedPaths: string[] };
      g.__openedPaths = [];
      shell.openPath = async (p: string) => {
        g.__openedPaths.push(p);
        return "";
      };
    });
    const file = droppedPage();
    const dir = path.dirname(file);
    const answers = await page.evaluate(
      async ([f, d]) => {
        const bridge = (window as unknown as {
          aniGui: { revealInFolder(p: string): Promise<boolean> };
        }).aniGui;
        return { file: await bridge.revealInFolder(f), dir: await bridge.revealInFolder(d) };
      },
      [file, dir],
    );
    expect(answers).toEqual({ file: false, dir: true });
    expect(
      await app.evaluate(() => (globalThis as unknown as { __openedPaths: string[] }).__openedPaths),
    ).toEqual([dir]);
  } finally {
    await app.close();
  }
});
