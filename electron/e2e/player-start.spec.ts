/**
 * From a stream attaching to its first frame, the player never offers
 * Play over it, and its loading indicator is on — in the packaged
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

async function launch(): Promise<{
  app: ElectronApplication;
  page: Page;
  profile: string;
}> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-start-"));
  writeReplayConfig(tmp);
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
