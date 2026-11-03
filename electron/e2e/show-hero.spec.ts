/**
 * The show page never scrolls sideways — in the packaged app's own
 * Chromium, where layout and CSS animations are real.
 *
 * Two things have pushed it past the window's trailing edge:
 *
 *   - The hero band's entry animation. The band escapes the layout's
 *     gutters and the rail column so the banner bleeds edge to edge,
 *     and it arrives scaled up and settles to its own size. A
 *     transformed box adds to the page's scrollable overflow unless an
 *     ancestor clips it, so for as long as the scale lasted the page
 *     grew a horizontal scrollbar.
 *   - The topbar's breadcrumb. Arriving from inside the app, the
 *     topbar carries the trail (Home, Search, the show's title) beside
 *     the search pill and the window controls. The trail would not
 *     shrink, so at the smallest window a long title pushed the window
 *     controls, and the page with them, past the edge for as long as
 *     the page stayed open. The trail now gives way, and it gives
 *     way from the title: the short labels beside it stay whole. The
 *     search pill still narrows first, as it did before the trail
 *     could shrink, so the title is only cut once the pill can give
 *     no more.
 *
 * So the reader arrives the way a reader does — searching, then
 * opening the show — at the app's smallest window, at a width where
 * the topbar is short of room and the pill narrows to make it, and at
 * a common desktop width, with motion on. The page's overflow is sampled on every
 * frame from the click through the entry animation (and the sampler
 * must have seen the hero's entry running, or the check proves
 * nothing), and read again once everything has settled.
 *
 * Every /api/* request is stubbed (see home-continue.spec.ts for why
 * the route sits on the page and the bounce that follows it), and the
 * image route answers with a generated banner so the hero has a cover
 * to frame — the default framing paints an enlarged, blurred copy of
 * it behind the image, which the band itself clips.
 *
 * Pre-condition: `pnpm run package` has produced `dist/linux-unpacked/`.
 */
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { killTree, withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { appInfo, defaultSettings } from './fixtures/kitsu';
import { appWindow, launchPackagedApp, packagedBinary } from './launch';

/** The app's minimum window; a width where the topbar is short of
 *  room, so the order in which pill and trail give way shows (the spec
 *  asserts that the row is short there); a common desktop width. */
const WIDTHS = [960, 1100, 1280];
const SQUEEZED = 1100;

/** Long enough to fill the breadcrumb's cap on a title. */
const TITLE = 'JoJo no Kimyou na Bouken Part 6: Stone Ocean';
/** The show, under an id of its own for each width, so every visit
 *  loads it afresh rather than from what the last one cached. */
const showAt = (id: string) => ({
	id,
	slug: 'hero-test-show',
	canonical_title: TITLE,
	titles: { en_jp: TITLE },
	synopsis: 'Test show used by the show-hero e2e suite.',
	start_date: '2024-01-12',
	end_date: null,
	episode_count: 12,
	average_rating: 70,
	subtype: 'TV',
	status: 'finished',
	age_rating: null,
	popularity_rank: null,
	poster_image: {
		large: 'https://media.kitsu.app/anime/poster_images/hero-test/large.jpg'
	},
	cover_image: {
		original: 'https://media.kitsu.app/anime/cover_images/hero-test/original.jpg'
	},
	anilist_banner: 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/hero-test.jpg'
});

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
	let c = n;
	for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	return c >>> 0;
});
function crc32(buf: Buffer): number {
	let c = 0xffffffff;
	for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
	return (c ^ 0xffffffff) >>> 0;
}
function chunk(type: string, data: Buffer): Buffer {
	const len = Buffer.alloc(4);
	len.writeUInt32BE(data.length);
	const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(body));
	return Buffer.concat([len, body, crc]);
}
/** A banner-shaped PNG: a horizontal gradient, so the framing is
 *  visible in a failure screenshot. */
function bannerPng(width = 1900, height = 400): Buffer {
	const ihdr = Buffer.alloc(13);
	ihdr.writeUInt32BE(width, 0);
	ihdr.writeUInt32BE(height, 4);
	ihdr[8] = 8; // bit depth
	ihdr[9] = 2; // truecolour RGB
	const row = Buffer.alloc(1 + width * 3);
	for (let x = 0; x < width; x++) {
		const t = x / (width - 1);
		row[1 + x * 3] = Math.round(40 + 180 * t);
		row[2 + x * 3] = 90;
		row[3 + x * 3] = Math.round(200 - 150 * t);
	}
	const raw = Buffer.concat(Array.from({ length: height }, () => row));
	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk('IHDR', ihdr),
		chunk('IDAT', zlib.deflateSync(raw)),
		chunk('IEND', Buffer.alloc(0))
	]);
}

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-e2e-hero-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await launchPackagedApp({
		...process.env,
		XDG_STATE_HOME: path.join(tmp, 'state'),
		XDG_CONFIG_HOME: path.join(tmp, 'config'),
		XDG_CACHE_HOME: path.join(tmp, 'cache'),
		XDG_DATA_HOME: path.join(tmp, 'data'),
		ANI_GUI_UNATTENDED: '1'
	});
	onLaunch(app);
	const page = await appWindow(app);
	const png = bannerPng();
	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			r.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body)
			});
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j(defaultSettings);
		if (p.startsWith('/api/image')) {
			return r.fulfill({ status: 200, contentType: 'image/png', body: png });
		}
		if (p.startsWith('/api/kitsu/anime/')) return j(showAt(p.split('/').pop() ?? ''));
		if (p === '/api/kitsu/search') return j([]);
		if (p.startsWith('/api/kitsu/episodes/')) return j([]);
		if (p === '/api/history') return j([]);
		if (p === '/api/watched-at') return j({});
		return j(null);
	});
	// Motion on, whatever the host prefers: the entry animation is one
	// of the two things under test.
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	const homeUrl = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	return { app, page };
}

async function launch() {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce((app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) {
					killTree(dead.process().pid);
					await dead.close();
				}
			}
		}
	);
}

/** Resizes the window's content to `width` and waits for the page to
 *  see it. Under Xvfb there is no window manager, and the app's own
 *  maximize can land after launch, so the request is repeated until
 *  the width holds. */
async function resize(app: ElectronApplication, page: Page, width: number) {
	for (let i = 0; i < 50; i++) {
		await app.evaluate(({ BrowserWindow }, w) => {
			const win = BrowserWindow.getAllWindows()[0];
			if (win.isMaximized()) win.unmaximize();
			win.setContentSize(w, 800);
		}, width);
		if (await page.evaluate((w) => window.innerWidth === w, width)) return;
		await page.waitForTimeout(200);
	}
	throw new Error(`the window never reached ${width}px`);
}

/** Navigates inside the app, as a link the reader follows would. */
async function follow(page: Page, href: string) {
	await page.evaluate((h) => {
		const a = document.createElement('a');
		a.href = h;
		document.body.appendChild(a);
		a.click();
		a.remove();
	}, href);
}

/** What the per-frame sampler below leaves on the window. */
type Sampled = {
	__worst: { over: number; past: string[] };
	/** Frames sampled while the loaded hero's entry animation ran. */
	__entryFrames: number;
	__sampling: boolean;
};

/** How far the page reaches past the window's trailing edge right now,
 *  and the boxes that do. */
function overflow(page: Page) {
	return page.evaluate(() => {
		const root = document.documentElement;
		const past = [...document.querySelectorAll<HTMLElement>('body *')]
			.filter((el) => el.getBoundingClientRect().right > root.clientWidth + 0.5)
			.map((el) => `${el.tagName.toLowerCase()}.${[...el.classList].join('.')}`)
			.slice(0, 8);
		return { over: root.scrollWidth - root.clientWidth, past };
	});
}

test('the show page does not scroll sideways while it opens or once it has', async () => {
	test.setTimeout(90_000);
	const { app, page } = await launch();
	try {
		for (const width of WIDTHS) {
			await resize(app, page, width);
			await follow(page, '/search?q=jojo');
			await expect(page.locator('.crumbs')).toBeVisible();

			// From the click on, every frame: the worst overflow seen and
			// the boxes behind it.
			await page.evaluate(() => {
				const w = window as unknown as Sampled;
				w.__worst = { over: 0, past: [] };
				w.__entryFrames = 0;
				w.__sampling = true;
				const tick = () => {
					const root = document.documentElement;
					const entering = document.getAnimations().some((a) => {
						if (!(a instanceof CSSAnimation) || a.playState !== 'running') return false;
						const el = (a.effect as KeyframeEffect | null)?.target as Element | null;
						return (
							a.animationName.endsWith('detail-hero-enter') &&
							el?.classList.contains('hero') === true &&
							!el.classList.contains('hero-skeleton')
						);
					});
					if (entering) w.__entryFrames++;
					const over = root.scrollWidth - root.clientWidth;
					if (over > w.__worst.over) {
						const past = [...document.querySelectorAll<HTMLElement>('body *')]
							.filter((el) => el.getBoundingClientRect().right > root.clientWidth + 0.5)
							.map((el) => `${el.tagName.toLowerCase()}.${[...el.classList].join('.')}`)
							.slice(0, 8);
						w.__worst = { over, past };
					}
					if (w.__sampling) requestAnimationFrame(tick);
				};
				requestAnimationFrame(tick);
			});
			await follow(page, `/anime/hero-test-${width}`);

			// The page has its show, the hero its decoded cover, the
			// breadcrumb its title, and the entry animation has run out.
			await expect(page.locator('.hero-backdrop')).toHaveCount(1, {
				timeout: 15_000
			});
			await page.waitForFunction(() => {
				const img = document.querySelector<HTMLImageElement>('.hero-img');
				return Boolean(img?.complete && img.naturalWidth > 0);
			});
			await expect(page.locator('.crumbs')).toContainText(TITLE);
			await page.waitForFunction(() =>
				document.getAnimations().every(
					// Svelte scopes keyframe names with a prefix.
					(a) =>
						!(a instanceof CSSAnimation) ||
						!/hero-(enter|in)$/.test(a.animationName) ||
						a.playState === 'finished'
				)
			);
			await page.waitForTimeout(500);

			const entry = await page.evaluate(() => {
				const w = window as unknown as Sampled;
				w.__sampling = false;
				return { ...w.__worst, frames: w.__entryFrames };
			});
			expect
				.soft(entry.frames, `at ${width}px no frame was sampled while the hero entered`)
				.toBeGreaterThan(0);
			expect
				.soft(
					entry.over,
					`at ${width}px the page scrolled sideways while it opened; past the edge: ${entry.past.join(', ')}`
				)
				.toBeLessThanOrEqual(0);

			const rest = await overflow(page);
			if (process.env.HERO_SHOTS) {
				await page.screenshot({
					path: path.join(process.env.HERO_SHOTS, `show-${width}.png`)
				});
			}
			expect
				.soft(
					rest.over,
					`at ${width}px the open page scrolls sideways; past the edge: ${rest.past.join(', ')}`
				)
				.toBeLessThanOrEqual(0);

			// The trail makes its room from the title; the short labels
			// beside it stay whole.
			const cut = await page.evaluate(() =>
				[...document.querySelectorAll<HTMLElement>('.crumbs li a, .crumbs li span')]
					.filter((el) => el.scrollWidth > el.clientWidth)
					.map((el) => el.textContent?.trim() ?? '')
			);
			expect
				.soft(
					cut.filter((label) => label !== TITLE),
					`at ${width}px labels cut short`
				)
				.toEqual([]);

			// The pill narrows first: if the trail is narrower than its
			// full width, the pill is already at its narrowest.
			const squeeze = await page.evaluate(() => {
				const crumbs = document.querySelector<HTMLElement>('.crumbs')!;
				const pill = document.querySelector<HTMLElement>('.topbar-search')!;
				// The box's width at `width`, out of the row's shrinking.
				const natural = (el: HTMLElement, width: string) => {
					const was = el.style.cssText;
					el.style.width = width;
					el.style.flex = 'none';
					const w = el.getBoundingClientRect().width;
					el.style.cssText = was;
					return w;
				};
				return {
					crumbs: crumbs.getBoundingClientRect().width,
					crumbsFull: natural(crumbs, 'max-content'),
					pill: pill.getBoundingClientRect().width,
					pillMin: natural(pill, 'min-content'),
					pillMax: parseFloat(getComputedStyle(pill).maxInlineSize)
				};
			});
			if (width === SQUEEZED) {
				// The row has to be short here, or the order below never shows.
				expect
					.soft(squeeze.pill, `at ${width}px the search pill had all the room it wanted`)
					.toBeLessThan(squeeze.pillMax - 0.5);
			}
			if (squeeze.crumbs < squeeze.crumbsFull - 0.5) {
				expect
					.soft(
						squeeze.pill,
						`at ${width}px the trail was cut while the search pill could still narrow`
					)
					.toBeLessThanOrEqual(squeeze.pillMin + 0.5);
			}
		}
	} finally {
		await app.close();
	}
});
