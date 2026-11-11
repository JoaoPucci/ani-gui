/**
 * A stream the app plays from its `src` (an mp4) is let go of when the
 * episode is handed to the external player, and comes back where it
 * was when the viewer plays it again — in the packaged app's own
 * Chromium, whose media stack is what would go on buffering a paused
 * stream for nobody.
 *
 * The stream is `fixtures/mp4/clip.mp4`, played through the backend's
 * own stream proxy from a server that counts what it is asked for and
 * answers byte ranges as a host does. It is 150 s of ffmpeg's testsrc,
 * long enough for a point past the first 15 s and before the last 90
 * (nearer either end, a position is kept as the start, or forgotten as
 * finished):
 *
 *   ffmpeg -f lavfi -i testsrc=size=96x54:rate=12 -t 150 -c:v libx264 \
 *     -preset veryslow -crf 45 -pix_fmt yuv420p -movflags +faststart \
 *     -g 12 clip.mp4
 *
 * The external player's route is answered by the metadata stub, so
 * nothing is launched, and the app reports that the player did not
 * start, as it does when one fails to.
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

const clip = path.join(__dirname, "fixtures", "mp4", "clip.mp4");

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
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-unload-"));
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

/** A host for the clip: answers byte ranges, slowly (about 20 KB/s,
 *  so the download is still under way when the episode is handed off),
 *  and counts the bytes it sends. */
async function clipServer(): Promise<{
  origin: string;
  sent: () => number;
  close: () => void;
}> {
  const body = fs.readFileSync(clip);
  let sent = 0;
  const trickle = (res: http.ServerResponse, bytes: Buffer) => {
    let at = 0;
    const step = () => {
      if (res.destroyed || at >= bytes.length) {
        res.end();
        return;
      }
      const chunk = bytes.subarray(at, at + 2048);
      at += chunk.length;
      sent += chunk.length;
      res.write(chunk);
      setTimeout(step, 100);
    };
    step();
  };
  const server = http.createServer((req, res) => {
    const range = /bytes=(\d+)-(\d*)/.exec(req.headers.range ?? "");
    if (!range) {
      res.writeHead(200, {
        "content-type": "video/mp4",
        "content-length": body.length,
        "accept-ranges": "bytes",
      });
      trickle(res, body);
      return;
    }
    const start = Number(range[1]);
    const end = range[2] ? Number(range[2]) : body.length - 1;
    res.writeHead(206, {
      "content-type": "video/mp4",
      "content-length": end - start + 1,
      "content-range": `bytes ${start}-${end}/${body.length}`,
      "accept-ranges": "bytes",
    });
    trickle(res, body.subarray(start, end + 1));
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as net.AddressInfo;
  return {
    origin: `http://127.0.0.1:${port}`,
    sent: () => sent,
    close: () => server.close(),
  };
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

const video = (page: Page) =>
  page.evaluate(() => {
    const v = document.querySelector(
      ".player-video-slot video",
    ) as HTMLVideoElement | null;
    return v
      ? {
          src: v.getAttribute("src"),
          paused: v.paused,
          time: v.currentTime,
        }
      : null;
  });

test("an mp4 handed to the external player is let go of, and comes back where it was", async () => {
  test.setTimeout(120_000);
  const host = await clipServer();
  const { app, page } = await launch();
  try {
    const base = await apiBase(page);
    await stubMetadata(page, base);
    const sid = await session(page, base, `${host.origin}/clip.mp4`);
    const url = new URL(page.url());
    url.pathname = `/play/${continueKitsuMatch.id}`;
    url.search = `?session=${sid}&episode=6&kind=mp4`;
    await page.goto(url.toString());
    await expect
      .poll(async () => (await video(page))?.time ?? 0, { timeout: 30_000 })
      .toBeGreaterThan(1);
    // Past the first 15 s, which are kept as the episode's start rather
    // than as a point (docs/architecture.md, Player).
    await page.evaluate(() => {
      (
        document.querySelector(".player-video-slot video") as HTMLVideoElement
      ).currentTime = 20;
    });
    await expect
      .poll(async () => (await video(page))?.time ?? 0, { timeout: 30_000 })
      .toBeGreaterThan(21);

    // Hand it to the external player.
    await page.locator(`button[aria-label="More actions"]`).click();
    await page.getByRole("menuitem", { name: /Open in external/ }).click();
    await expect.poll(async () => (await video(page))?.src).toBeNull();
    const left = await page.evaluate(() => {
      const raw = localStorage.getItem("ani-gui.watch-positions") ?? "[]";
      const kept = (JSON.parse(raw) as [string, number][]).find(([k]) =>
        k.endsWith(":6"),
      );
      return kept?.[1] ?? null;
    });
    expect(left).toBeGreaterThan(20);

    // Nobody watches it here: the host sends nothing more, though the
    // download was still under way (what is in flight settles first).
    await page.waitForTimeout(1000);
    const sentAt = host.sent();
    expect(sentAt).toBeLessThan(fs.statSync(clip).size);
    await page.waitForTimeout(3000);
    expect(host.sent()).toBe(sentAt);

    // The stub's answer is no launch the app knows, so it says the
    // external player did not start, as it does when a player fails to:
    // the viewer closes that and plays it here again.
    const dialog = page.getByRole("alertdialog");
    await dialog.waitFor({ timeout: 15_000 });
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);

    // The viewer plays it again: it comes back, at its point.
    await page.locator(".player-frame").hover();
    await page.locator(".pc-buttons-row .pc-btn").first().click();
    await expect
      .poll(async () => (await video(page))?.src ?? "", { timeout: 15_000 })
      .toContain("/s/");
    await expect
      .poll(async () => (await video(page))?.time ?? 0, { timeout: 30_000 })
      .toBeGreaterThan((left as number) - 1);
    await expect.poll(async () => (await video(page))?.paused).toBe(false);
  } finally {
    await app.close();
    host.close();
  }
});
