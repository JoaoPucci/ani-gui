/**
 * From a stream attaching to its first frame, the player never offers
 * Play over it, and its start veil is on — in the packaged
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

async function launch(
  locale = "en",
): Promise<{ app: ElectronApplication; page: Page }> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-start-"));
  const config = path.join(tmp, "config", "ani-gui");
  fs.mkdirSync(config, { recursive: true });
  fs.writeFileSync(path.join(config, "config.toml"), `locale = "${locale}"\n`);
  const app = await launchPackagedApp({
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    ANI_GUI_UNATTENDED: "1",
  });
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
async function stubMetadata(
  page: Page,
  base: string,
  meta: {
    settings?: object;
    show?: object;
    episodes?: object[];
  } = {},
) {
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
    if (p === "/api/settings") return j(meta.settings ?? settings);
    if (p.startsWith("/api/kitsu/anime/"))
      return j(meta.show ?? continueKitsuMatch);
    if (p.startsWith("/api/kitsu/episodes/"))
      return j(meta.episodes ?? [continueKitsuEpisode6]);
    if (p === "/api/history") return j([]);
    if (p.startsWith("/api/image")) return r.fulfill({ status: 503 });
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
            document
              .querySelector("section.player-frame")
              ?.getAttribute("data-start-veil") === "start",
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
            document
              .querySelector("section.player-frame")
              ?.getAttribute("data-start-veil") === "off",
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

/* Gintama's own titles, from Kitsu: the longest show title and the
 * longest episode title among its entries, on its 367th episode. The
 * title card must keep them inside the frame, above the controls, the
 * episode title on two lines at most and the loader in sight, in every
 * locale's caption. */
const longShow = {
  ...continueKitsuMatch,
  canonical_title:
    "Gintama': Futon ni Haitte kara Buki Nokoshi ni Kizuite Neru ni Nerenai Toki mo Aru",
};
const longEpisode = {
  ...continueKitsuEpisode6,
  id: "ep-367",
  number: 367,
  relative_number: 367,
  canonical_title:
    "Do Something Uncharacteristic, and Something Uncharacteristic Will Happen",
};

for (const locale of ["en", "pt-BR", "ru"]) {
  test(`the title card keeps Gintama's longest titles inside the frame (${locale})`, async () => {
    test.setTimeout(120_000);
    // Fragments held past the test: the start stays on screen.
    const media = await mediaServer(90_000);
    const { app, page } = await launch(locale);
    try {
      const base = await apiBase(page);
      await stubMetadata(page, base, {
        settings: { ...settings, locale },
        show: longShow,
        episodes: [longEpisode],
      });
      await page.evaluate(() =>
        localStorage.setItem("ani-gui:player-start-look", "card"),
      );
      const sid = await session(page, base, `${media.origin}/master.m3u8`);
      const url = new URL(page.url());
      url.pathname = `/play/${continueKitsuMatch.id}`;
      url.search = `?session=${sid}&episode=367&kind=hls`;
      await page.goto(url.toString());
      await page.locator(".start-veil[data-look='card'] .card-title").waitFor();
      await expect(page.locator(".start-veil .card-title")).toHaveText(
        longEpisode.canonical_title,
      );
      // The loader arrives after its beat.
      await page.waitForTimeout(1200);

      const layout = await page.evaluate(() => {
        const box = (sel: string) => {
          const r = document.querySelector(sel)!.getBoundingClientRect();
          return {
            top: r.top,
            bottom: r.bottom,
            left: r.left,
            right: r.right,
            height: r.height,
          };
        };
        const title = document.querySelector(".start-veil .card-title")!;
        const lineHeight = parseFloat(getComputedStyle(title).lineHeight);
        const show = document.querySelector(".start-veil .card-show")!;
        return {
          frame: box("section.player-frame"),
          card: box(".start-veil .card"),
          numeral: box(".start-veil .numeral"),
          strip: box(".start-veil .card-status .strip"),
          stripOpacity: getComputedStyle(
            document.querySelector(".start-veil .card-status .strip")!,
          ).opacity,
          titleLines: title.getBoundingClientRect().height / lineHeight,
          showOneLine:
            show.getBoundingClientRect().height <
            parseFloat(getComputedStyle(show).lineHeight) * 1.5,
        };
      });
      const inside = (b: typeof layout.card) =>
        b.left >= layout.frame.left &&
        b.right <= layout.frame.right &&
        b.top >= layout.frame.top &&
        b.bottom <= layout.frame.bottom - 80;
      expect(inside(layout.card), JSON.stringify(layout)).toBe(true);
      expect(inside(layout.numeral), JSON.stringify(layout)).toBe(true);
      expect(inside(layout.strip), JSON.stringify(layout)).toBe(true);
      expect(layout.strip.height).toBeGreaterThan(0);
      expect(Number(layout.stripOpacity)).toBeGreaterThan(0.9);
      expect(layout.titleLines).toBeLessThanOrEqual(2.1);
      expect(layout.showOneLine).toBe(true);
    } finally {
      await app.close();
      media.close();
    }
  });
}
