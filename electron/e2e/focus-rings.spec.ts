/**
 * A keyboard focus ring is drawn once, around what the control looks
 * like, and whole.
 *
 * Every focusable control gets the app's ring on `:focus-visible`
 * (frontend/src/lib/design/tokens.css): two box shadows around the
 * element's own box. Some controls drew theirs wrong:
 *
 *   - The window's minimize, maximize and close buttons. Each is the
 *     topbar's full height, so the corner stays easy to hit, and shows
 *     its hover and focus as a square chip in the middle. The chip got
 *     its ring, and the button kept the app's ring as well: a tall
 *     rectangle around the whole click target, its top edge cut off by
 *     the top of the window.
 *   - The app's mark at the top of the rail, a link home. It drew an
 *     outline as a wide pill around the mark and its name, on top of
 *     the app's ring, and the window's edge cut the pill's side off.
 *   - The rail's links and its version link span the rail from the
 *     window's edge, so the window cut their rings' side off too.
 *   - The buttons of a segmented control (Sub/Dub, the settings'
 *     choices) sit inside a rounded track that clips what overflows
 *     it, and it cut their rings.
 *   - A control the keyboard scrolls into view stops flush with the
 *     window's edge, and its ring hangs past it.
 *
 * Layout is the subject, so this runs in the packaged app's Chromium.
 * The keyboard moves focus through every stop on Home, a show's page
 * and Settings, at the smallest window the app allows and at a common
 * desktop size, with the window maximized and not, and at every stop
 * the focused control's ring is read from its computed style:
 *
 *   - no control's own ring leaves the window or is cut by a box that
 *     clips what overflows it;
 *   - a control whose chip carries the ring (`::before`) draws none of
 *     its own, and the chip's ring is inside the window;
 *   - the rail's mark rings the way the rail's links do.
 *
 * What it does not reach: menus, dialogs and popovers that open on
 * demand, and the player's page. A ring there is not checked here.
 *
 * Where a ring lies is computed from its outline and its shadows'
 * spread around the element's box, and a `::before` chip's box from its
 * size, centred on its control as the window buttons place it: a
 * pseudo-element has no box a script can read. Every API request is
 * stubbed.
 */
import { type ElectronApplication, type Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { appInfo, defaultSettings, topRated, trending } from './fixtures/kitsu';
import { appWindow, launchPackagedApp, packagedBinary, stopApp, stopDeadApp } from './launch';
import { answer } from './stubs';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-focus-rings-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
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
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') {
			return r.request().method() === 'PUT' ? answer(r, { status: 204 }) : j(defaultSettings);
		}
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(topRated);
		if (p.startsWith('/api/image')) return answer(r, { status: 503 });
		if (p === '/api/history') return j([]);
		if (p === '/api/history/card-lines') return j({});
		if (p === '/api/watched-at') return j({});
		if (p === '/api/availability/batch') {
			const { kitsu_ids } = r.request().postDataJSON() as { kitsu_ids: string[] };
			return j({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		}
		if (p === '/api/availability/warm') return answer(r, { status: 202 });
		if (p === '/api/update-check') return answer(r, { status: 204 });
		if (p.startsWith('/api/kitsu/anime/')) return j(topRated[0]);
		if (p.startsWith('/api/kitsu/episodes/')) return j([]);
		if (p === '/api/download/default-dir') return j(null);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		if (p === '/api/kitsu/search') return j([]);
		if (p.startsWith('/api/play/stream')) return answer(r, { status: 503 });
		return j(null);
	});

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
				if (dead) await stopDeadApp(dead);
			}
		}
	);
}

/** Lays the page out at `width` × `height` (see home-pill-fit.spec.ts). */
async function sizeViewport(page: Page, width: number, height: number) {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: 0,
		mobile: false
	});
	await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width);
	await cdp.detach();
}

interface Box {
	left: number;
	right: number;
	top: number;
	bottom: number;
}
interface Stop {
	/** How the control reads in a failure message. */
	name: string;
	/** The control's own ring, or null when it draws none. */
	ring: Box | null;
	/** The ring its `::before` chip carries, or null. */
	chipRing: Box | null;
	/** Its own ring's look, to compare one control's with another's. */
	look: { outline: string; boxShadow: string; borderRadius: string };
	/** The boxes, the window's first, that clip what overflows them. */
	clips: { name: string; box: Box }[];
	/** Whether this is one of the window's buttons, the rail's mark, or
	 *  one of the rail's links. */
	kind: 'winctl' | 'brand' | 'nav' | 'other';
}

/** The focused control's rings and what may clip them, read in one frame. */
function readStop(page: Page): Promise<Stop | null> {
	return page.evaluate(() => {
		const el = document.activeElement as HTMLElement | null;
		if (!el || el === document.body) return null;
		const px = (v: string) => Number.parseFloat(v) || 0;
		/** How far a computed style's outline and outer shadows reach
		 *  past the box; null when it draws neither. */
		const reach = (cs: CSSStyleDeclaration): number | null => {
			let r: number | null = null;
			if (cs.outlineStyle !== 'none' && px(cs.outlineWidth) > 0) {
				r = Math.max(0, px(cs.outlineWidth) + px(cs.outlineOffset));
			}
			if (cs.boxShadow && cs.boxShadow !== 'none') {
				// Each shadow: colour, then x y blur spread lengths; `inset`
				// shadows stay inside the box.
				for (const shadow of cs.boxShadow.split(/,(?![^(]*\))/)) {
					if (/\binset\b/.test(shadow)) continue;
					const lengths = (
						shadow.replace(/\b(rgba?|color|oklab|hsla?)\([^)]*\)/g, '').match(/-?[\d.]+px/g) ?? []
					).map(px);
					const [x = 0, y = 0, blur = 0, spread = 0] = lengths;
					const s = Math.max(Math.abs(x), Math.abs(y)) + blur + spread;
					if (s > 0) r = Math.max(r ?? 0, s);
				}
			}
			return r;
		};
		const grow = (b: DOMRect | Box, by: number): Box => ({
			left: b.left - by,
			right: b.right + by,
			top: b.top - by,
			bottom: b.bottom + by
		});
		const cs = getComputedStyle(el);
		const own = reach(cs);
		const rect = el.getBoundingClientRect();
		const before = getComputedStyle(el, '::before');
		const chipReach = before.content !== 'none' ? reach(before) : null;
		let chipRing: Box | null = null;
		if (chipReach !== null) {
			const w = px(before.width);
			const h = px(before.height);
			const cx = (rect.left + rect.right) / 2;
			const cy = (rect.top + rect.bottom) / 2;
			chipRing = grow(
				{ left: cx - w / 2, right: cx + w / 2, top: cy - h / 2, bottom: cy + h / 2 },
				chipReach
			);
		}
		const clips: { name: string; box: Box }[] = [
			{
				name: `the window (scrolled ${Math.round(window.scrollY)}, ${Math.round(
					document.documentElement.scrollHeight - window.innerHeight - window.scrollY
				)} px left below)`,
				box: { left: 0, top: 0, right: window.innerWidth, bottom: window.innerHeight }
			}
		];
		for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
			const s = getComputedStyle(a);
			if (s.overflowX === 'visible' && s.overflowY === 'visible' && s.contain === 'none') {
				continue;
			}
			const r = a.getBoundingClientRect();
			// The clip is the padding box: inside the borders.
			clips.push({
				name: `${a.tagName.toLowerCase()}.${[...a.classList].join('.')}`,
				box: {
					left: s.overflowX === 'visible' ? -Infinity : r.left + px(s.borderLeftWidth),
					right: s.overflowX === 'visible' ? Infinity : r.right - px(s.borderRightWidth),
					top: s.overflowY === 'visible' ? -Infinity : r.top + px(s.borderTopWidth),
					bottom: s.overflowY === 'visible' ? Infinity : r.bottom - px(s.borderBottomWidth)
				}
			});
		}
		const label =
			el.getAttribute('aria-label') ?? el.textContent?.trim().slice(0, 40) ?? el.tagName;
		return {
			name: `${el.tagName.toLowerCase()}.${[...el.classList].join('.')} "${label}"`,
			ring: own === null ? null : grow(rect, own),
			chipRing,
			look: { outline: cs.outline, boxShadow: cs.boxShadow, borderRadius: cs.borderRadius },
			clips,
			kind: el.classList.contains('winctl-btn')
				? 'winctl'
				: el.classList.contains('brand')
					? 'brand'
					: el.classList.contains('nav-link')
						? 'nav'
						: 'other'
		};
	});
}

/** Moves focus forward with Tab until it comes round to where it began,
 *  and reads every stop. The page's first Tab enters it at the top. */
async function walk(page: Page): Promise<Stop[]> {
	await page.mouse.click(1, 1);
	await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
	const stops: Stop[] = [];
	const seen = new Set<string>();
	for (let i = 0; i < 400; i++) {
		await page.keyboard.press('Tab');
		const id = await page.evaluate(() => {
			const el = document.activeElement as HTMLElement | null;
			if (!el || el === document.body) return null;
			el.dataset.focusWalk ??= String(Math.random());
			return el.dataset.focusWalk;
		});
		if (id === null) continue;
		if (seen.has(id)) break;
		seen.add(id);
		const stop = await readStop(page);
		if (stop) stops.push(stop);
	}
	return stops;
}

const inside = (inner: Box, outer: Box) =>
	inner.left >= outer.left - 0.5 &&
	inner.right <= outer.right + 0.5 &&
	inner.top >= outer.top - 0.5 &&
	inner.bottom <= outer.bottom + 0.5;

function assertStops(stops: Stop[], at: string) {
	expect(
		stops.length,
		`${at}: the keyboard reached controls (${stops.map((s) => s.name).join(' | ')})`
	).toBeGreaterThan(3);
	const kinds = new Set(stops.map((s) => s.kind));
	for (const k of ['winctl', 'brand', 'nav'] as const) {
		expect.soft(kinds.has(k), `${at}: the walk reached a ${k} control`).toBe(true);
	}
	for (const s of stops) {
		const where = `${at}: ${s.name} ${JSON.stringify(s)}`;
		// Whole: inside the window and every box that clips it.
		for (const ring of [s.ring, s.chipRing]) {
			if (!ring) continue;
			for (const c of s.clips) {
				expect.soft(inside(ring, c.box), `${where}: its ring is cut by ${c.name}`).toBe(true);
			}
		}
		// Drawn once: a control whose chip carries the ring draws none.
		if (s.chipRing) {
			expect.soft(s.ring, `${where}: draws a ring beside its chip's`).toBeNull();
		}
		if (s.kind === 'winctl') {
			expect.soft(s.chipRing, `${where}: its chip carries the ring`).not.toBeNull();
		}
	}
	// The rail's mark rings as the rail's links do.
	const nav = stops.find((s) => s.kind === 'nav');
	const brand = stops.find((s) => s.kind === 'brand');
	if (nav && brand)
		expect.soft(brand.look, `${at}: the mark's ring looks like a rail link's`).toEqual(nav.look);
}

const views = [
	{ label: 'smallest', width: 960, height: 600 },
	{ label: 'desktop', width: 1440, height: 900 }
] as const;

test('every focus ring is drawn once and whole', async () => {
	test.setTimeout(240_000);
	const testInfo = test.info();
	const { app, page } = await launch();
	try {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await expect(page.locator('.winctl-btn')).toHaveCount(3, { timeout: 15_000 });
		const pages = [
			{
				label: 'home',
				go: async () => {
					await expect(page.getByText(/top rated/i).first()).toBeVisible({ timeout: 15_000 });
				}
			},
			{
				label: 'show',
				go: async () => {
					await page.locator(`a[href$="/anime/${topRated[0].id}"]`).first().click();
					await expect.poll(() => page.url()).toContain(`/anime/${topRated[0].id}`);
					await expect(page.locator('button.ep-tile').first()).toBeVisible({ timeout: 15_000 });
				}
			},
			{
				label: 'settings',
				go: async () => {
					await page.locator('a.nav-link[href$="/settings"]').click();
					await expect.poll(() => page.url()).toContain('/settings');
					await expect(page.locator('select.select-input').first()).toBeVisible({
						timeout: 15_000
					});
				}
			}
		];
		for (const maximized of [false, true]) {
			await app.evaluate(({ BrowserWindow }, max) => {
				const win = BrowserWindow.getAllWindows().find((w) => !w.isDestroyed() && w.isVisible());
				if (max) win?.maximize();
				else win?.unmaximize();
			}, maximized);
			for (const view of views) {
				await sizeViewport(page, view.width, view.height);
				for (const p of pages) {
					await page.locator('a.brand').click();
					await p.go();
					// Settled: the route's content is on show, so the focus reset
					// a navigation makes has happened, and two frames have drawn.
					await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
					await page.evaluate(
						() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
					);
					const at = `${p.label}, ${view.label}${maximized ? ', maximized' : ''}`;
					const stops = await walk(page);
					testInfo.annotations.push({
						type: 'stops',
						description: `${at}: ${stops.map((s) => s.name).join(' | ')}`
					});
					assertStops(stops, at);
					if (p.label === 'home') await shoot(page, `${view.label}${maximized ? '-max' : ''}`);
				}
			}
		}
	} finally {
		await stopApp(app);
	}
});

/** Shots of the window buttons and the rail's mark, each focused from
 *  the keyboard. */
async function shoot(page: Page, suffix: string) {
	const testInfo = test.info();
	const focusByKeys = async (selector: string) => {
		await page.mouse.click(1, 1);
		await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
		for (let i = 0; i < 400; i++) {
			await page.keyboard.press('Tab');
			if (await page.evaluate((s) => document.activeElement?.matches(s) ?? false, selector)) {
				return;
			}
		}
		throw new Error(`the keyboard never reached ${selector}`);
	};
	const width = await page.evaluate(() => window.innerWidth);
	for (const [i, label] of ['minimize', 'maximize', 'close'].entries()) {
		await focusByKeys(`.winctl-btn:nth-child(${i + 1})`);
		await page.screenshot({
			path: testInfo.outputPath(`winctl-${label}-${suffix}.png`),
			clip: { x: width - 220, y: 0, width: 220, height: 80 }
		});
	}
	await focusByKeys('a.brand');
	await page.screenshot({
		path: testInfo.outputPath(`brand-${suffix}.png`),
		clip: { x: 0, y: 0, width: 160, height: 200 }
	});
}
