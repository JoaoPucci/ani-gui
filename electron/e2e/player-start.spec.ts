/**
 * From a stream attaching to its first frame, the player never offers
 * Play over it, and its loading indicator is on — in the packaged
 * app's own Chromium, where the element's autoplay is real.
 *
 * An autoplaying <video> reads `paused` until its first fragment is
 * in and fires no `waiting` before then. A player that shows the
 * element's `paused` offers Play over a black frame for as long as the
 * first fragment takes, and viewers press it. The stream is the local
 * HLS fixture played through the backend's own stream proxy, from a
 * server that holds every fragment back the way a slow host does.
 */
import {
  type ElectronApplication,
  type Page,
} from "@playwright/test";
import { expect, test } from "./policy-guard";
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
const HOLD_MS = 2500;

const settings = {
  ...defaultSettings,
  locale: "en",
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
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-start-"));
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
async function mediaServer(
  holdMs: number,
): Promise<{ origin: string; close: () => void }> {
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
    // Every fragment is held back, as a slow host holds the first.
    const delay = name.endsWith(".ts") ? holdMs : 0;
    setTimeout(() => {
      res.writeHead(200, {
        "content-type": types[path.extname(name)] ?? "application/octet-stream",
        "content-length": body.length,
      });
      res.end(body);
    }, delay);
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
        headers: { "content-type": "application/json" },
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

type Sample = {
  t: number;
  paused: boolean;
  readyState: number;
  label: string | null;
  indicator: boolean;
};

test("a stream on its way reads Pause with the loading indicator on, until its first frame", async () => {
  test.setTimeout(120_000);
  const media = await mediaServer(HOLD_MS);
  const { app, page } = await launch();
  try {
    const base = await apiBase(page);
    await stubMetadata(page, base);
    const sid = await session(page, base, `${media.origin}/master.m3u8`);
    const url = new URL(page.url());
    url.pathname = `/play/${continueKitsuMatch.id}`;
    url.search = `?session=${sid}&episode=1&kind=hls`;
    await page.goto(url.toString());
    await page.locator(".pc-buttons-row .pc-btn").first().waitFor();

    // Read the element and the chrome until frames play.
    const samples: Sample[] = [];
    const t0 = Date.now();
    for (;;) {
      const s = await page.evaluate(() => {
        const v = document.querySelector(
          ".player-video-slot video",
        ) as HTMLVideoElement | null;
        return {
          paused: v?.paused ?? true,
          readyState: v?.readyState ?? 0,
          label:
            document
              .querySelector(".pc-buttons-row .pc-btn")
              ?.getAttribute("aria-label") ?? null,
          indicator:
            document.querySelector(".player-spinner.player-spinner-on") !==
            null,
        };
      });
      samples.push({ t: Date.now() - t0, ...s });
      if (!s.paused && s.readyState >= 3) break;
      if (Date.now() - t0 > 60_000) throw new Error("the stream never played");
      await page.waitForTimeout(100);
    }

    // The gap is real: the element read paused for a while.
    const waiting = samples.filter((s) => s.paused);
    expect(waiting.length).toBeGreaterThan(5);
    // Through all of it, Pause and the indicator; never Play.
    for (const s of waiting) {
      expect(s, `at ${s.t} ms`).toMatchObject({
        label: "Pause",
        indicator: true,
      });
    }

    // Frames play: the indicator goes and Pause stays.
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            document.querySelector(".player-spinner.player-spinner-on") ===
            null,
        ),
      )
      .toBe(true);
    await expect(
      page.locator(".pc-buttons-row .pc-btn").first(),
    ).toHaveAttribute("aria-label", "Pause");
  } finally {
    await app.close();
    media.close();
  }
});
