/**
 * The player starts on Auto at the master's highest level, whatever
 * the settings hold, and the More menu names that start on one line —
 * in the packaged app's own Chromium, where layout is real.
 *
 * The backend resolves the master for every play, and the only quality
 * the settings keep is the download's; neither it nor the quality an
 * older version wrote may cap the player. The stream is the local
 * two-level HLS fixture (36p and 72p) played through the backend's own
 * stream proxy, and the app runs in Brazilian Portuguese, whose word for
 * Auto ("Automática") once pushed the More menu's "Auto · 72p" value
 * onto a second line.
 */
import {
  expect,
  test,
  type ElectronApplication,
  type Page,
} from "@playwright/test";
import fs, { existsSync } from "node:fs";
import http from "node:http";
import type net from "node:net";
import os from "node:os";
import path from "node:path";
import {
  appInfo,
  continueKitsuEpisode6,
  continueKitsuMatch,
  defaultSettings,
} from "./fixtures/kitsu";
import { appWindow, launchPackagedApp, packagedBinary } from "./launch";

const mediaDir = path.join(__dirname, "fixtures", "captions");

/** Settings that would have capped the player before: a download
 *  quality below the stream's top, and the quality key 0.17.0 wrote. */
const settings = {
  ...defaultSettings,
  locale: "pt-BR",
  download_quality: "480",
  quality: "worst",
  use_custom_player_controls: true,
};

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`,
    );
  }
});

async function launch(): Promise<{ app: ElectronApplication; page: Page }> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-quality-"));
  const config = path.join(tmp, "config", "ani-gui");
  fs.mkdirSync(config, { recursive: true });
  fs.writeFileSync(
    path.join(config, "config.toml"),
    'locale = "pt-BR"\ndownload_quality = "480"\nquality = "worst"\n',
  );
  const app = await launchPackagedApp({
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    ANI_GUI_UNATTENDED: "1",
  }, { profile: tmp });
  const page = await appWindow(app);
  return { app, page };
}

/** A server for the media fixture, standing in for a provider's CDN. */
async function mediaServer(): Promise<{ origin: string; close: () => void }> {
  const types: Record<string, string> = {
    ".m3u8": "application/vnd.apple.mpegurl",
    ".ts": "video/mp2t",
  };
  const server = http.createServer((req, res) => {
    const name = path.basename(new URL(req.url ?? "/", "http://x").pathname);
    const file = path.join(mediaDir, name);
    if (!existsSync(file)) {
      res.writeHead(404).end();
      return;
    }
    const body = fs.readFileSync(file);
    res.writeHead(200, {
      "content-type": types[path.extname(name)] ?? "application/octet-stream",
      "content-length": body.length,
    });
    res.end(body);
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as net.AddressInfo;
  return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

async function apiBase(page: Page): Promise<string> {
  await page.waitForFunction(
    () => Boolean((window as { aniGui?: unknown }).aniGui),
    null,
    { timeout: 30_000 },
  );
  return page.evaluate(
    () => (window as unknown as { aniGui: { apiBase: string } }).aniGui.apiBase,
  );
}

/** Metadata the play page asks the API for, answered without the
 *  network; the session route goes to the real backend. */
async function stubMetadata(page: Page, base: string) {
  await page.route(`${base}/api/**`, async (r) => {
    const p = new URL(r.request().url()).pathname;
    if (p === "/api/sessions") return r.continue();
    const j = (body: unknown) =>
      r.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(body),
      });
    if (p === "/api/app-info") return j(appInfo);
    if (p === "/api/settings") return j(settings);
    if (p.startsWith("/api/kitsu/anime/")) return j(continueKitsuMatch);
    if (p.startsWith("/api/kitsu/episodes/")) return j([continueKitsuEpisode6]);
    if (p === "/api/history") return j([]);
    if (p.startsWith("/api/image")) return r.fulfill({ status: 503 });
    // The backend answers skip times with a list, empty when it
    // knows none; the page reads it as one once the length is known.
    if (p.startsWith("/api/aniskip/")) return j([]);
    return j(null);
  });
}

async function session(page: Page, base: string, upstream: string) {
  const created = await page.evaluate(
    async ({ base, upstream }) => {
      const res = await fetch(`${base}/api/sessions`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          // The API answers only a request carrying the launch's secret.
          "x-ani-gui-internal-secret":
            (window as { aniGui?: { internalSecret?: string } }).aniGui
              ?.internalSecret ?? "",
        },
        body: JSON.stringify({ upstream_url: upstream, referer: "" }),
      });
      if (!res.ok)
        throw new Error(`session: ${res.status} ${await res.text()}`);
      return (await res.json()) as { session_id: string };
    },
    { base, upstream },
  );
  return created.session_id;
}

test("the player starts on Auto at the top level whatever the settings hold, and the More menu says so on one line", async () => {
  test.setTimeout(120_000);
  const media = await mediaServer();
  const { app, page } = await launch();
  try {
    const base = await apiBase(page);
    await stubMetadata(page, base);
    const sid = await session(page, base, `${media.origin}/master.m3u8`);
    const url = new URL(page.url());
    url.pathname = `/play/${continueKitsuMatch.id}`;
    url.search = `?session=${sid}&episode=1&kind=hls`;
    await page.goto(url.toString());

    // Auto at the highest level: the 72p picture, not the 36p a cap
    // would hold.
    await expect
      .poll(
        () =>
          page.evaluate(
            () => document.querySelector("video")?.videoHeight ?? 0,
          ),
        { timeout: 30_000 },
      )
      .toBe(72);

    // The controls hide while the pointer is away; the menu is driven
    // by its button's own click.
    await page
      .locator('button.pc-btn[aria-haspopup="menu"]')
      .last()
      .dispatchEvent("click");
    const value = page.locator(".pc-menu-row-arrow .pc-menu-quality-current");
    await expect(value).toHaveText("Automática · 72p", { timeout: 10_000 });
    const lines = await value.evaluate((el) => {
      const lineHeight = parseFloat(getComputedStyle(el).lineHeight);
      return el.getBoundingClientRect().height / lineHeight;
    });
    expect(lines).toBeLessThan(1.5);
  } finally {
    await app.close();
    media.close();
  }
});
