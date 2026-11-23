/**
 * From a stream attaching to its first frame, the player never offers
 * Play over it, and its start veil is on — in the packaged
 * app's own Chromium, where the element's autoplay is real.
 *
 * An autoplaying <video> reads `paused` until its first fragment is
 * in and fires no `waiting` before then. A player that shows the
 * element's `paused` offers Play over a black frame for as long as the
 * first fragment takes, and viewers press it. The stream is the local
 * HLS fixture, from a server that holds every fragment back the way a
 * slow host does; the play is the Continue card's, clicked, and runs
 * through the backend's play route and stream proxy (real-play.ts).
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
import { appWindow, launchPackagedApp, packagedBinary } from "./launch";
import { continueKitsuEpisode6, continueKitsuMatch } from "./fixtures/kitsu";
import { playFromContinue, routeRealPlay, writeReplayConfig } from "./real-play";

const mediaDir = path.join(__dirname, "fixtures", "captions");
const HOLD_MS = 2500;

const settings = {
  locale: "en",
  use_custom_player_controls: true,
};

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`,
    );
  }
});

async function launch(locale = "en"): Promise<{
  app: ElectronApplication;
  page: Page;
  profile: string;
}> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-start-"));
  writeReplayConfig(tmp, `locale = "${locale}"\n`);
  const app = await launchPackagedApp({
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    ANI_GUI_UNATTENDED: "1",
  }, { profile: tmp });
  const page = await appWindow(app);
  return { app, page, profile: tmp };
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
  const { app, page, profile } = await launch();
  try {
    const play = await routeRealPlay(page, {
      profile,
      settings,
      stream: { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: "hls" },
    });
    await playFromContinue(page, play);
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
 * longest episode titles among its entries, on its 367th episode. The
 * title card must keep them inside the frame, above the controls, the
 * episode title on two lines at most and the loader in sight, with each
 * locale's own caption. */
const longShow = {
  ...continueKitsuMatch,
  // Long enough to reach the episode the card plays.
  episode_count: 367,
  canonical_title:
    "Gintama': Futon ni Haitte kara Buki Nokoshi ni Kizuite Neru ni Nerenai Toki mo Aru",
};
const longEpisode = {
  ...continueKitsuEpisode6,
  id: "ep-367",
  number: 367,
  relative_number: 367,
  // Gintama's three longest episode titles joined: at the window the
  // test sets (below), long enough that the clamp must cut it, so the
  // check is a check of the clamp.
  canonical_title:
    "Do Something Uncharacteristic, and Something Uncharacteristic Will Happen. Two in Girl Years Is Equal to Ten in Man Years. The Unemployed Cannot Be Stained By Anything",
};

const startingCaption: Record<string, string> = {
  en: "Loading episode 367",
  "pt-BR": "Carregando episódio 367",
  "es-419": "Cargando episodio 367",
  ru: "Загрузка серии 367",
};

/** The window the layout case is measured in, set on the page's own
 *  window: the card's type scales with the frame, whose height follows
 *  the window's, and a short frame drops the show line, so the case
 *  holds the window still rather than measure whatever the session
 *  gave it. Under Xvfb there is no window manager, and the app's own
 *  maximize can land after launch, so the request is repeated until
 *  the size holds. */
const WINDOW = { width: 1280, height: 960 };
async function holdWindow(app: ElectronApplication, page: Page) {
  const win = await app.browserWindow(page);
  for (let i = 0; i < 50; i++) {
    await win.evaluate((w, size) => {
      if (w.isMaximized()) w.unmaximize();
      w.setContentSize(size.width, size.height);
    }, WINDOW);
    if (
      await page.evaluate(
        (size) =>
          window.innerWidth === size.width &&
          window.innerHeight === size.height,
        WINDOW,
      )
    )
      return;
    await page.waitForTimeout(200);
  }
  throw new Error(`the window never reached ${WINDOW.width}x${WINDOW.height}`);
}

for (const locale of ["en", "pt-BR", "es-419", "ru"]) {
  test(`the title card keeps Gintama's longest titles inside the frame (${locale})`, async () => {
    test.setTimeout(120_000);
    // Fragments held past the test: the start stays on screen.
    const media = await mediaServer(90_000);
    const { app, page, profile } = await launch(locale);
    try {
      const realPlay = await routeRealPlay(page, {
        profile,
        settings: { ...settings, locale },
        stream: {
          upstreamUrl: `${media.origin}/master.m3u8`,
          mediaKind: "hls",
        },
        show: { match: longShow, episode: longEpisode },
      });
      await page.evaluate(() =>
        localStorage.setItem("ani-gui:player-start-look", "card"),
      );
      await holdWindow(app, page);
      await playFromContinue(page, realPlay);
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
          // The clamp is what holds the title to two lines: it must be
          // cutting text off, not just finding room.
          titleClamped: title.scrollHeight > title.clientHeight + 1,
          showShown: show.getBoundingClientRect().height > 0,
          showOneLine:
            show.getBoundingClientRect().height <
            parseFloat(getComputedStyle(show).lineHeight) * 1.5,
          caption:
            document.querySelector(".start-veil .card-caption")?.textContent ??
            "",
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
      expect(layout.titleClamped).toBe(true);
      expect(layout.showShown).toBe(true);
      expect(layout.showOneLine).toBe(true);
      expect(layout.caption).toBe(startingCaption[locale]);

      // The controls are out of sight, but a control the keyboard
      // reaches shows above the veil, not under it.
      const play = page.locator(".pc-buttons-row .pc-btn").first();
      await play.focus();
      // The veil lets the pointer through, so a hit test would skip it:
      // for the measurement only, it takes the pointer, and the hit test
      // returns whatever is painted on top.
      // Set through the CSSOM: the window's content security policy
      // refuses an inline style sheet, but not an element's style.
      await page.evaluate(() => {
        for (const el of document.querySelectorAll<HTMLElement>(
          ".start-veil, .start-veil *",
        )) {
          el.style.setProperty("pointer-events", "auto", "important");
        }
      });
      await page.waitForTimeout(400);
      const onTop = await play.evaluate((el) => {
        const r = el.getBoundingClientRect();
        const hit = document.elementFromPoint(
          r.left + r.width / 2,
          r.top + r.height / 2,
        );
        return hit === el || el.contains(hit);
      });
      expect(onTop).toBe(true);
    } finally {
      await app.close();
      media.close();
    }
  });
}
