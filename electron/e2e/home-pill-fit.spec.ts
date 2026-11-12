/**
 * A guessed Continue card's pill fits its card in every locale.
 *
 * On hover or keyboard focus a guessed card shows "≈ Not what you
 * watched?" at the top corner its delete chip leaves free. The card is
 * a fixed 16rem whatever the window's size, and the chip pops out of
 * the opposite corner: the pill stops at 13.375rem, a --space-2 short
 * of it. Every locale's sentence fits on one line at the app's type
 * size with the fonts this was measured on, Portuguese with the least
 * to spare. Words that outgrow the room, on a wider face or in a
 * longer locale, wrap to balanced lines: every word on show, inside
 * the card, clear of the chip, and the button keeps the whole sentence
 * for a name. The third view below spaces the letters a fifth wider
 * to make that happen here.
 *
 * Layout is the subject, so this runs in the packaged app's Chromium
 * with the fonts the platform has, at the smallest window the app
 * allows and at a common desktop size. Every API request is stubbed.
 */
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
	appInfo,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	topRated,
	trending
} from './fixtures/kitsu';
import {
	appWindow,
	cardLinesAnswer,
	launchPackagedApp,
	packagedBinary,
	stopApp,
	stopDeadApp
} from './launch';

/** One row, which Continue Watching guesses: it records no show. */
const history = [{ id: 'allmanga-test-show', ep_no: '5', title: 'Continue Test Show' }];

const episodes = Array.from({ length: 12 }, (_, i) => ({
	...continueKitsuEpisode6,
	id: `ep-${i + 1}`,
	number: i + 1,
	relative_number: i + 1,
	canonical_title: `Episode ${i + 1}`
}));

/** A 1×1 PNG, so the card shows artwork behind the pill. */
const PIXEL = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
	'base64'
);

/** The pill's words, from the shipped catalogue. */
function pillText(locale: string): string {
	const file = path.resolve(__dirname, '../../frontend/messages', locale, 'home.json');
	return JSON.parse(fs.readFileSync(file, 'utf8')).link_not_watched as string;
}

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

async function launchOnce(locale: string, onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-pill-fit-${process.pid}-${Date.now()}`);
	const config = path.join(tmp, 'config');
	fs.mkdirSync(path.join(config, 'ani-gui'), { recursive: true });
	fs.writeFileSync(path.join(config, 'ani-gui', 'config.toml'), `locale = "${locale}"\n`);
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: config,
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data'),
			ANI_GUI_UNATTENDED: '1'
		},
		{ profile: tmp }
	);
	onLaunch(app);
	const page = await appWindow(app);

	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j({ ...defaultSettings, locale });
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(topRated);
		if (p.startsWith('/api/image')) {
			return r.fulfill({ status: 200, contentType: 'image/png', body: PIXEL });
		}
		if (p === '/api/history') return j(history);
		if (p === '/api/history/card-lines') return j(cardLinesAnswer(r.request().postDataJSON()));
		if (p === '/api/watched-at') return j({ [history[0].id]: 1_700_000_000 });
		// A mapping no play stored: the card's show is a guess.
		if (p.endsWith('/played')) return j(null);
		if (p.startsWith('/api/allmanga-kitsu-map/')) return j(continueKitsuMatch.id);
		if (p.startsWith('/api/kitsu/anime/')) return j(continueKitsuMatch);
		if (p.startsWith('/api/kitsu/episodes/')) return j(episodes);
		if (p.startsWith('/api/title-match')) return j(null);
		if (p === '/api/kitsu/search') return j([]);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		return j(null);
	});

	// A fresh mount whose every request flows through the stubs above
	// (see home-continue.spec.ts).
	const homeUrl = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	return { app, page };
}

async function launch(locale: string) {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce(locale, (app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) await stopDeadApp(dead);
			}
		}
	);
}

/** Lays the page out at `width` × `height`. The compositor a test
 *  runs under may hold the window maximized, so the page's viewport is
 *  set through DevTools rather than by resizing the window. */
async function sizeViewport(page: Page, width: number, height: number) {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: 0,
		mobile: false
	});
	await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width);
}

/** The pill's box against its card's and the chip's, read in one frame. */
function geometry(page: Page) {
	return page.locator('.resume-cell:has(.resume-link)').first().evaluate((cell) => {
		const box = (el: Element) => {
			const r = el.getBoundingClientRect();
			return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
		};
		const pill = cell.querySelector('.resume-link') as HTMLElement;
		const chip = cell.querySelector('.resume-delete') as HTMLElement;
		const poster = cell.querySelector('.resume-poster') as HTMLElement;
		// The words' lines, by their tops: each line's width and middle.
		const byTop = new Map<number, { width: number; middle: number }>();
		const rects: DOMRect[] = [];
		const texts = document.createTreeWalker(pill, NodeFilter.SHOW_TEXT);
		for (let t = texts.nextNode(); t; t = texts.nextNode()) {
			const range = document.createRange();
			range.selectNodeContents(t);
			rects.push(...Array.from(range.getClientRects()));
		}
		for (const r of rects) {
			if (r.width <= 0) continue;
			const top = Math.round(r.top);
			const line = byTop.get(top) ?? { width: 0, middle: r.top + r.height / 2 };
			line.width += r.width;
			byTop.set(top, line);
		}
		const lines = [...byTop.entries()].sort(([a], [b]) => a - b).map(([, l]) => l);
		const mark = (pill.querySelector('svg[data-guess-mark]') as Element).getBoundingClientRect();
		return {
			pill: box(pill),
			chip: box(chip),
			poster: box(poster),
			overflowX: pill.scrollWidth - pill.clientWidth,
			overflowY: pill.scrollHeight - pill.clientHeight,
			lines: lines.length,
			lineWidths: lines.map((l) => Math.round(l.width)),
			markOffFirstLine: lines.length ? Math.abs(mark.top + mark.height / 2 - lines[0].middle) : 0,
			opacity: getComputedStyle(pill).opacity
		};
	});
}

/** Where the pill is looked at: the smallest window the app opens
 *  (electron/lib/app-window.cjs), a common desktop one, and the smallest
 *  again with letters a fifth wider than this platform's face, standing
 *  in for a wider face on another platform or a future locale's longer
 *  words. The card is the same 16rem in all three. */
const views = [
	{ label: 'smallest', width: 960, height: 600, wider: false },
	{ label: 'normal', width: 1440, height: 900, wider: false },
	{ label: 'wider-face', width: 960, height: 600, wider: true }
] as const;

for (const locale of ['en', 'pt-BR', 'es-419', 'ru']) {
	// A cold launch, a remount and three views, each hovered, measured
	// and shot: more than the suite's default 30 s on a slow runner.
	test(`the guessed card's pill fits its card in ${locale}`, async () => {
		test.setTimeout(120_000);
		const testInfo = test.info();
		const { app, page } = await launch(locale);
		try {
			await page.emulateMedia({ reducedMotion: 'reduce' });
			const words = pillText(locale);
			const cell = page.locator('.resume-cell:has(.resume-link)').first();
			// The ≈ is hidden from assistive tech: the sentence is the name.
			const pill = cell.getByRole('button', { name: words, exact: true });

			for (const view of views) {
				await sizeViewport(page, view.width, view.height);
				await page.evaluate((wider) => {
					const id = 'pill-fit-wider';
					document.getElementById(id)?.remove();
					if (!wider) return;
					const style = document.createElement('style');
					style.id = id;
					style.textContent = '.resume-link { letter-spacing: 0.12em; }';
					document.head.append(style);
				}, view.wider);
				await expect(cell).toBeVisible({ timeout: 15_000 });
				await cell.locator('.resume-card').hover();
				await expect(pill).toHaveCount(1);
				await expect.poll(async () => (await geometry(page)).opacity).toBe('1');

				const g = await geometry(page);
				const at = `${locale}, ${view.label}: ${JSON.stringify(g)}`;
				testInfo.annotations.push({ type: 'geometry', description: at });
				// Every word on show: nothing scrolled away or cut inside the pill.
				expect(g.overflowX, at).toBeLessThanOrEqual(0);
				expect(g.overflowY, at).toBeLessThanOrEqual(0);
				// Over the artwork, inside the card.
				expect(g.pill.left, at).toBeGreaterThanOrEqual(g.poster.left);
				expect(g.pill.right, at).toBeLessThanOrEqual(g.poster.right);
				expect(g.pill.bottom, at).toBeLessThanOrEqual(g.poster.bottom);
				// Clear of the delete chip on the other corner.
				expect(g.pill.right, at).toBeLessThan(g.chip.left);
				// One line, or two balanced ones, the ≈ on the first.
				expect(g.lines, at).toBeGreaterThanOrEqual(1);
				expect(g.lines, at).toBeLessThanOrEqual(2);
				expect(Math.min(...g.lineWidths) / Math.max(...g.lineWidths), at).toBeGreaterThan(0.5);
				expect(g.markOffFirstLine, at).toBeLessThanOrEqual(2);

				// The chip pops out of the card's corner: the shot keeps it.
				const around = 16;
				await page.screenshot({
					path: testInfo.outputPath(`pill-${locale}-${view.label}.png`),
					clip: {
						x: g.poster.left - around,
						y: g.chip.top - around,
						width: g.chip.right - g.poster.left + 2 * around,
						height: g.poster.bottom - g.chip.top + 2 * around
					}
				});
			}
		} finally {
			await stopApp(app);
		}
	});
}
