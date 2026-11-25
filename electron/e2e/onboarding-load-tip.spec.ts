/**
 * The player's load tip, on the packaged app: a stream played through
 * the backend's play route and stream proxy (real-play.ts) from a local
 * server that holds every fragment back as a slow host does, or not at
 * all. The tip is the on-demand `player-up-next`, started from the
 * Debug page's "Show it", as the maintainer starts it.
 *
 * - a start that lasts a second shows it over the start veil; it never
 *   takes focus, so the player's keys keep working, and when the first
 *   frame comes it stays, folded to a plate, until Escape closes it;
 * - a start that lands sooner never shows it;
 */
import type { ElectronApplication, Page } from "@playwright/test";
import { expect, test } from "./policy-guard";
import fs, { existsSync } from "node:fs";
import http from "node:http";
import type net from "node:net";
import os from "node:os";
import path from "node:path";
import {
  appWindow,
  launchPackagedApp,
  packagedBinary,
  stopApp,
} from "./launch";
import {
  playFromContinue,
  routeRealPlay,
  writeReplayConfig,
} from "./real-play";

const mediaDir = path.join(__dirname, "fixtures", "captions");
const settings = { locale: "en", use_custom_player_controls: true };

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`,
    );
  }
});

/** A server for the media fixture, holding every fragment `holdMs`. */
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
    setTimeout(
      () => {
        res.writeHead(200, {
          "content-type":
            types[path.extname(name)] ?? "application/octet-stream",
          "content-length": body.length,
        });
        res.end(body);
      },
      name.endsWith(".ts") ? holdMs : 0,
    );
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as net.AddressInfo;
  return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

async function launch(holdMs: number) {
  const media = await mediaServer(holdMs);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-load-tip-"));
  writeReplayConfig(tmp, 'locale = "en"\n');
  const app: ElectronApplication = await launchPackagedApp(
    {
      ...process.env,
      XDG_STATE_HOME: path.join(tmp, "state"),
      XDG_CONFIG_HOME: path.join(tmp, "config"),
      XDG_CACHE_HOME: path.join(tmp, "cache"),
      XDG_DATA_HOME: path.join(tmp, "data"),
    },
    { profile: tmp, onboardingVersion: "0.19.0" },
  );
  const page = await appWindow(app);
  const play = await routeRealPlay(page, {
    profile: tmp,
    settings,
    stream: { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: "hls" },
  });
  // The feature tips are the backend's own.
  await page.route("**/api/onboarding**", (r) => r.continue());
  return { app, page, play, media };
}

/** Navigate inside the app, as a link press does, keeping the session. */
async function go(page: Page, href: string): Promise<void> {
  await page.evaluate((to) => {
    const a = document.createElement("a");
    a.href = to;
    document.body.appendChild(a);
    a.click();
    a.remove();
  }, href);
  await expect.poll(() => new URL(page.url()).pathname).toBe(href);
}

async function showPlayerTip(page: Page): Promise<void> {
  await go(page, "/diagnostics");
  const row = page.locator('[data-testid="tip-key"][data-id="player-up-next"]');
  await row.getByRole("button", { name: /show it/i }).click();
  await expect(row).toContainText(/waiting/i);
  await go(page, "/");
}

const tip = (page: Page) => page.getByTestId("onboarding-load-tip");
const veil = (page: Page) =>
  page.locator("section.player-frame").getAttribute("data-start-veil");

test("a slow start shows the tip without taking focus, and it stays as a plate until Escape", async () => {
  test.setTimeout(180_000);
  const { app, page, play, media } = await launch(4000);
  try {
    await showPlayerTip(page);
    await playFromContinue(page, play);
    await expect(tip(page)).toBeVisible({ timeout: 15_000 });
    expect(await veil(page)).toBe("start");
    await expect(tip(page).locator(".note")).toHaveAttribute(
      "data-phase",
      "loading",
    );
    // Never the focus: it sits outside the tip, so the player's keys
    // stay the player's.
    expect(
      await page.evaluate(
        () =>
          document
            .querySelector('[data-testid="onboarding-load-tip"]')
            ?.contains(document.activeElement) ?? false,
      ),
    ).toBe(false);
    await expect(page.locator('.load-tip-host [role="status"]')).toContainText(
      /0\.19\.0/,
    );

    await expect(tip(page).locator(".note")).toHaveAttribute(
      "data-phase",
      "playing",
      {
        timeout: 30_000,
      },
    );
    await page.keyboard.press("Escape");
    await expect(tip(page)).toHaveCount(0);
  } finally {
    media.close();
    await stopApp(app);
  }
});

test("a start that lands within a second never shows the tip", async () => {
  test.setTimeout(180_000);
  const { app, page, play, media } = await launch(0);
  try {
    await showPlayerTip(page);
    await playFromContinue(page, play);
    await expect.poll(() => veil(page), { timeout: 30_000 }).toBe("off");
    await page.waitForTimeout(1_500);
    await expect(tip(page)).toHaveCount(0);
  } finally {
    media.close();
    await stopApp(app);
  }
});
