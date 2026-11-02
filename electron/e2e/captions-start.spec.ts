/**
 * The subtitle track a session starts on shows alone, in the packaged
 * app's own Chromium, through playback, a seek and a quality switch.
 *
 * Chromium runs its own track selection a moment after `<track>`
 * elements join a video, and while a subtitle track shows it turns on
 * one more of the tracks no script has set since they joined. A start
 * switched on after the tracks joined left a second language showing
 * beside it — Arabic over English on a nine-language episode. The
 * unit and acceptance tiers run under happy-dom, which has no such
 * selection; this is where the real one runs.
 *
 * The stream is a local two-level HLS fixture played through the
 * backend's own stream proxy; its sidecar listing (Arabic first, then
 * English, the provider's default, then French) is answered at the
 * stream lane's URLs, as the CSP-era e2e does, because the public
 * session route takes no caller-supplied tracks.
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
import { launchPackagedApp, packagedBinary } from "./launch";

const mediaDir = path.join(__dirname, "fixtures", "captions");

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`,
    );
  }
});

async function launch(): Promise<{ app: ElectronApplication; page: Page }> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-captions-"));
  const app = await launchPackagedApp({
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    ANI_GUI_UNATTENDED: "1",
  });
  const page = await app.firstWindow();
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

async function bridge(page: Page) {
  await page.waitForFunction(
    () => Boolean((window as { aniGui?: unknown }).aniGui),
    null,
    {
      timeout: 30_000,
    },
  );
  return page.evaluate(() => {
    const g = (
      window as unknown as {
        aniGui: { apiBase: string; lanes: Record<string, string> };
      }
    ).aniGui;
    return { apiBase: g.apiBase, lanes: g.lanes };
  });
}

/** Metadata the play page asks the API for, answered without the
 *  network; the session route goes to the real backend. */
async function stubMetadata(page: Page, apiBase: string) {
  await page.route(`${apiBase}/api/**`, async (r) => {
    const p = new URL(r.request().url()).pathname;
    if (p === "/api/sessions") return r.continue();
    const j = (body: unknown) =>
      r.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(body),
      });
    if (p === "/api/app-info") return j(appInfo);
    if (p === "/api/settings")
      return j({ ...defaultSettings, use_custom_player_controls: true });
    if (p.startsWith("/api/kitsu/anime/")) return j(continueKitsuMatch);
    if (p.startsWith("/api/kitsu/episodes/")) return j([continueKitsuEpisode6]);
    if (p === "/api/history") return j([]);
    if (p.startsWith("/api/image")) return r.fulfill({ status: 503 });
    return j(null);
  });
}

const LISTING = [
  { lang: "ar", label: "العربية", default: false },
  { lang: "en", label: "English", default: true },
  { lang: "fr", label: "Français", default: false },
];

/** A session on the fixture stream, with the sidecar listing answered
 *  at the stream lane. */
async function session(
  page: Page,
  apiBase: string,
  stream: string,
  upstream: string,
) {
  const created = await page.evaluate(
    async ({ apiBase, upstream }) => {
      const res = await fetch(`${apiBase}/api/sessions`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ upstream_url: upstream, referer: "" }),
      });
      if (!res.ok)
        throw new Error(`session: ${res.status} ${await res.text()}`);
      return (await res.json()) as { session_id: string };
    },
    { apiBase, upstream },
  );
  const sid = created.session_id;
  const cues = fs.readFileSync(path.join(mediaDir, "sidecar.vtt"), "utf8");
  await page.route(`${stream}/s/${sid}/subtitles`, (r) =>
    r.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "access-control-allow-origin": "*" },
      body: JSON.stringify(
        LISTING.map((t, i) => ({
          ...t,
          url: `${stream}/s/${sid}/sub/${i}.vtt`,
        })),
      ),
    }),
  );
  await page.route(`${stream}/s/${sid}/sub/*.vtt`, (r) => {
    const i = Number(
      path.basename(new URL(r.request().url()).pathname, ".vtt"),
    );
    r.fulfill({
      status: 200,
      contentType: "text/vtt",
      headers: { "access-control-allow-origin": "*" },
      body: cues.replaceAll("Cue at", `${LISTING[i].label} at`),
    });
  });
  return sid;
}

/** What the player shows: the labels of the subtitle tracks showing,
 *  the cue text on screen, the playback position and frame height. */
function shown(page: Page) {
  return page.evaluate(() => {
    const v = document.querySelector("video");
    if (!v) return null;
    const tracks = [...v.textTracks].filter((t) => t.kind === "subtitles");
    const on = tracks.filter((t) => t.mode === "showing");
    return {
      tracks: tracks.length,
      showing: on.map((t) => t.label),
      cues: on.flatMap((t) =>
        [...(t.activeCues ?? [])].map((c) => (c as VTTCue).text),
      ),
      time: v.currentTime,
      height: v.videoHeight,
    };
  });
}

/** Over `ms`, read what shows every 250 ms; every read must show the
 *  track labelled `label` alone. */
async function showsAloneThroughout(page: Page, label: string, ms: number) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    expect((await shown(page))?.showing).toEqual([label]);
    await page.waitForTimeout(250);
  }
}

test("the track a session starts on shows alone through playback, a seek and a quality switch", async () => {
  test.setTimeout(120_000);
  const media = await mediaServer();
  const { app, page } = await launch();
  try {
    const { apiBase, lanes } = await bridge(page);
    await stubMetadata(page, apiBase);
    const sid = await session(
      page,
      apiBase,
      lanes.stream,
      `${media.origin}/master.m3u8`,
    );
    const url = new URL(page.url());
    url.pathname = `/play/${continueKitsuMatch.id}`;
    url.search = `?session=${sid}&episode=1&kind=hls`;
    await page.goto(url.toString());

    // The three tracks attach and playback runs.
    await expect
      .poll(async () => (await shown(page))?.tracks ?? 0, { timeout: 30_000 })
      .toBe(3);
    await expect
      .poll(async () => (await shown(page))?.time ?? 0, { timeout: 30_000 })
      .toBeGreaterThan(1);

    // English — the UI locale's language — shows, alone, its cues on
    // screen, and stays alone while it plays.
    await expect
      .poll(async () => (await shown(page))?.cues ?? [], { timeout: 10_000 })
      .toEqual([expect.stringMatching(/^English at /)]);
    await showsAloneThroughout(page, "English", 3_000);

    // A seek keeps it.
    await page.evaluate(() => {
      const v = document.querySelector("video");
      if (v) v.currentTime = 14;
    });
    await expect
      .poll(async () => (await shown(page))?.time ?? 0, { timeout: 15_000 })
      .toBeGreaterThan(14.5);
    await showsAloneThroughout(page, "English", 2_000);

    // A quality switch from the player's menu keeps it.
    const before = (await shown(page))?.height ?? 0;
    await page.evaluate(() => {
      const v = document.querySelector("video");
      if (v) v.currentTime = 2;
    });
    // The controls hide while the pointer is away; the menu is driven
    // by its buttons' own clicks.
    await page
      .locator('button.pc-btn[aria-haspopup="menu"][aria-label="More"]')
      .dispatchEvent("click");
    await page
      .locator(".pc-menu-row-arrow", { hasText: "Quality" })
      .dispatchEvent("click");
    const other = before === 72 ? "36p" : "72p";
    await page
      .locator(".pc-menu-quality-row", { hasText: other })
      .dispatchEvent("click");
    await expect
      .poll(async () => (await shown(page))?.height ?? 0, { timeout: 20_000 })
      .toBe(before === 72 ? 36 : 72);
    await showsAloneThroughout(page, "English", 2_000);
  } finally {
    await app.close();
    media.close();
  }
});
