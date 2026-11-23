/**
 * In a forced-colours theme a control's state stays visible.
 *
 * A Windows contrast theme (`forced-colors: active`) replaces the
 * colours a page sets with the theme's own: every background becomes
 * the theme's canvas colour, and text and borders its text colour. A
 * state shown only by a background or a colour then disappears: a
 * switch reads the same on and off, the chosen button of a segmented
 * control looks like the others, the rail does not say which page is
 * open, and a slider or a progress bar shows no fill.
 *
 * Layout and paint are the subject, so this runs in the packaged app's
 * Chromium with forced colours emulated (the theme's colours, not only
 * the media query: the test checks a box shadow is dropped). For each
 * state it pictures the control in one state, changes the state, and
 * pictures it again: at least a 4 by 4 patch's worth of pixels must
 * differ plainly between the two, and the two pictures must be the same
 * size. A button or a rail link is pictured by the band along its edge,
 * away from its words, which the theme colours itself (past its outer
 * two pixels, where a separator's shade changes, for a button). A fill,
 * a track or a bar is pictured shown and hidden, which must differ as
 * well. Nothing else changes between the two pictures: the mouse rests
 * in the window's corner and nothing has focus, but in the topbar's
 * results (whose field keeps focus) and the player (the mouse over the
 * player, and the timeline or the volume field focused, which holds its
 * controls up and shows the volume).
 *
 * What it covers: Home's hero pager (and a hovered track's colour),
 * the diagnostics switches for the hero's framing and the rows' style,
 * the rail's current page and its
 * version link, the settings switches and segmented controls, a show's
 * Sub/Dub control, the download dialog's modes, the download bar's and
 * the download list's progress, the topbar's picked result, the search
 * page's sort and type chips, the search's progress bar, and the
 * player's timeline and volume fills and tracks. A state shown by text
 * as well (a switch's On/Off label) is still pictured without that
 * text: the control itself must show it. Where a control opts out of
 * the theme's colours, the spec also reads that what it draws stays in
 * the theme: a switch's thumb inside its track's border, a chosen
 * button's separator and words, a picked result's words, and the bars
 * drawn whole. Keyboard focus in this theme
 * is not checked here (focus-rings.spec.ts). Every API request is
 * stubbed, but for the player test's play and stream (see
 * real-play.ts).
 */
import { type ElectronApplication, type Locator, type Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import type net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { appInfo, defaultSettings, topRated, trending } from './fixtures/kitsu';
import { appWindow, launchPackagedApp, packagedBinary, stopApp, stopDeadApp } from './launch';
import { playFromContinue, routeRealPlay, writeReplayConfig } from './real-play';
import { answer } from './stubs';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-forced-state-${process.pid}-${Date.now()}`);
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
		if (p === '/api/download/default-dir') return j(path.join(os.tmpdir(), 'ani-gui-e2e-dl'));
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		if (p === '/api/kitsu/search') return j([]);
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

/** Emulates a forced-colours theme, and checks it is in force: the
 *  theme drops every box shadow. */
async function forcedColours(page: Page) {
	await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: 'active' });
	expect(await page.evaluate(() => matchMedia('(forced-colors: active)').matches)).toBe(true);
	const shadow = await page.evaluate(() => {
		const probe = document.createElement('div');
		probe.style.boxShadow = '0 0 0 4px red';
		document.body.append(probe);
		const s = getComputedStyle(probe).boxShadow;
		probe.remove();
		return s;
	});
	expect(shadow, 'the theme drops box shadows').toBe('none');
}

/** Nothing focused, nothing hovered, two frames drawn. */
async function still(page: Page) {
	await page.mouse.move(1, 1);
	await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
	await page.evaluate(
		() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
	);
}

/** How many pixels of two same-size pictures differ plainly (by more
 *  than 96 over RGB), counted by the page's own image decoder. With
 *  `band`, only the pixels within that many of the picture's edge count,
 *  past its outer two pixels (a hairline separator's shade is not a
 *  state): a control's words sit in its middle, and the theme colours
 *  text itself, so a state must show around them. */
function pixelsApart(page: Page, a: Buffer, b: Buffer, band?: number, skip = 2): Promise<number> {
	return page.evaluate(
		async ([a64, b64, edge, inner]) => {
			const load = async (s: string) => {
				const img = new Image();
				img.src = `data:image/png;base64,${s}`;
				await img.decode();
				const c = document.createElement('canvas');
				c.width = img.width;
				c.height = img.height;
				const ctx = c.getContext('2d') as CanvasRenderingContext2D;
				ctx.drawImage(img, 0, 0);
				return { data: ctx.getImageData(0, 0, c.width, c.height).data, w: c.width, h: c.height };
			};
			const [{ data: x, w, h }, { data: y }] = await Promise.all([load(a64), load(b64)]);
			// Pictures of different sizes say nothing about a state.
			if (x.length !== y.length) return -1;
			let n = 0;
			for (let i = 0; i < x.length; i += 4) {
				if (edge !== null) {
					const px = (i / 4) % w;
					const py = Math.floor(i / 4 / w);
					const d = Math.min(px, py, w - 1 - px, h - 1 - py);
					if (d < inner || d >= edge) continue;
				}
				const d =
					Math.abs(x[i] - y[i]) + Math.abs(x[i + 1] - y[i + 1]) + Math.abs(x[i + 2] - y[i + 2]);
				if (d > 96) n++;
			}
			return n;
		},
		[a.toString('base64'), b.toString('base64'), band ?? null, skip] as const
	);
}

/** The least number of plainly different pixels that counts as a state
 *  the eye can see: a 4 by 4 patch. */
const SEEN = 16;

/** A picture of `target` before and after `change`: they must differ. */
async function shows(
	page: Page,
	target: Locator,
	what: string,
	change: () => Promise<void>,
	band?: number,
	{ skip = 2, keepFocus = false }: { skip?: number; keepFocus?: boolean } = {}
) {
	const settle = async () => {
		if (keepFocus) {
			await page.mouse.move(1, 1);
			await page.evaluate(
				() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
			);
		} else await still(page);
	};
	await settle();
	const before = await target.screenshot();
	await change();
	await settle();
	const after = await target.screenshot();
	const slug = what.replace(/[^a-z0-9]+/gi, '-');
	fs.writeFileSync(test.info().outputPath(`${slug}-before.png`), before);
	fs.writeFileSync(test.info().outputPath(`${slug}-after.png`), after);
	expect
		.soft(
			await pixelsApart(page, before, after, band, skip),
			`${what} shows in a forced-colours theme`
		)
		.toBeGreaterThanOrEqual(SEEN);
}

/** A system colour as the page resolves it in the theme now in force. */
function systemColour(page: Page, name: string): Promise<string> {
	return page.evaluate((n) => {
		const probe = document.createElement('span');
		probe.style.setProperty('forced-color-adjust', 'none');
		probe.style.color = n;
		document.body.append(probe);
		const c = getComputedStyle(probe).color;
		probe.remove();
		return c;
	}, name);
}

/** The computed value of `prop` on the first element `target` finds. */
const computed = (target: Locator, prop: string) =>
	target.first().evaluate((el, p) => getComputedStyle(el).getPropertyValue(p), prop);

/** A pattern matching `label` and nothing more. */
const exactly = (label: string) =>
	new RegExp(`^\\s*${label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*$`);

/** Hides or shows an element without moving anything. */
const hidden = (target: Locator, hide: boolean) =>
	target.evaluate((el, h) => {
		(el as HTMLElement).style.visibility = h ? 'hidden' : '';
	}, hide);

async function sizeViewport(page: Page, width: number, height: number) {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: 0,
		mobile: false
	});
	await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width);
	return cdp;
}

test('settings, a show, the rail and the download bar keep their state in a forced-colours theme', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch();
	try {
		const cdp = await sizeViewport(page, 1440, 900);
		await forcedColours(page);
		await expect(page.getByText(/top rated/i).first()).toBeVisible({ timeout: 15_000 });

		// Home's hero pager, with three shows to feature: it shows at all,
		// and which show is on.
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			if (p !== '/api/kitsu/trending-anilist') return r.fallback();
			const three = [0, 1, 2].map((i) => ({ ...trending[0], id: `${trending[0].id}${i}` }));
			return answer(r, {
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(three)
			});
		});
		const homeUrl = page.url();
		await page.goto('about:blank');
		await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
		await forcedColours(page);
		const pager = page.locator('.hero-pager');
		await expect(pager).toBeVisible({ timeout: 15_000 });
		await shows(page, pager, "the hero's pager", async () => {
			for (const track of await pager.locator('.hero-pager-track').all()) {
				await hidden(track, true);
			}
		});
		for (const track of await pager.locator('.hero-pager-track').all()) await hidden(track, false);
		// A track under the mouse keeps a colour of the theme's.
		const third = pager.locator('.hero-pager-dot').nth(2);
		await third.hover();
		const themed = await Promise.all(
			['GrayText', 'CanvasText', 'Highlight'].map((n) => systemColour(page, n))
		);
		expect
			.soft(themed, "a hovered hero pager track draws in one of the theme's colours")
			.toContain(await computed(third.locator('.hero-pager-track'), 'background-color'));
		await page.mouse.move(1, 1);
		const second = pager.locator('.hero-pager-dot').nth(1);
		await shows(page, second, "the hero pager's current show", () => second.click());

		// The hero's framing switch and the rows' style switch, shown
		// while the diagnostics page's toggle is on: a pressed choice.
		await page.evaluate(() => {
			localStorage.setItem(
				'ani-gui:hero-framing',
				JSON.stringify({ framing: 'pillar', source: 'anilist', controls: true })
			);
			localStorage.setItem(
				'ani-gui:row-style',
				JSON.stringify({ style: 'inline', controls: true })
			);
		});
		await page.goto('about:blank');
		await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
		await forcedColours(page);
		for (const group of ['.hero-framing-switch', '.row-style-switch']) {
			const unpressed = page.locator(`${group} button[aria-pressed="false"]`).first();
			await expect(unpressed).toBeVisible({ timeout: 15_000 });
			const label = (await unpressed.textContent())?.trim() ?? '';
			const choice = page.locator(`${group} button`, { hasText: exactly(label) });
			await shows(page, choice, `${group}, ${label} pressed`, () => choice.click(), 6);
		}

		// The rail: the Settings link, on Home and then on Settings.
		const settingsLink = page.locator('a.nav-link[href$="/settings"]');
		await shows(
			page,
			settingsLink,
			"the rail's current page",
			async () => {
				await settingsLink.click();
				await expect.poll(() => page.url()).toContain('/settings');
				await expect(page.locator('select.select-input').first()).toBeVisible({
					timeout: 15_000
				});
			},
			6,
			{ skip: 0 }
		);

		// The version link at the rail's foot, on Settings and on About.
		const foot = page.locator('a.rail-foot');
		await shows(
			page,
			foot,
			"the rail's version link, current",
			async () => {
				await foot.click();
				await expect.poll(() => page.url()).toContain('/about');
			},
			6,
			{ skip: 0 }
		);
		await settingsLink.click();
		await expect(page.locator('select.select-input').first()).toBeVisible({ timeout: 15_000 });

		// Each switch, off and on. Its On/Off label is left out of the
		// picture: the switch itself must show it. A button or a rail link
		// is pictured by the band along its edge, away from its words.
		const switches = page.locator('label.switch');
		const count = await switches.count();
		expect(count, 'Settings has switches').toBeGreaterThan(2);
		for (let i = 0; i < count; i++) {
			const sw = switches.nth(i);
			await sw.scrollIntoViewIfNeeded();
			await shows(page, sw.locator('.switch-track'), `switch ${i + 1}`, async () => {
				await sw.locator('.switch-track').click();
			});
		}

		// A switch's thumb sits inside its track's border, off and on.
		const first = switches.first();
		for (let n = 0; n < 2; n++) {
			const gaps = await first.evaluate((label) => {
				const track = label.querySelector('.switch-track') as HTMLElement;
				const thumb = label.querySelector('.switch-thumb') as HTMLElement;
				const cs = getComputedStyle(track);
				const t = track.getBoundingClientRect();
				const h = thumb.getBoundingClientRect();
				return [
					h.left - (t.left + parseFloat(cs.borderLeftWidth)),
					t.right - parseFloat(cs.borderRightWidth) - h.right,
					h.top - (t.top + parseFloat(cs.borderTopWidth)),
					t.bottom - parseFloat(cs.borderBottomWidth) - h.bottom
				];
			});
			expect
				.soft(Math.min(...gaps), `a switch's thumb clears its track's border (${gaps})`)
				.toBeGreaterThanOrEqual(0.5);
			await first.locator('.switch-track').click();
			await still(page);
		}

		// Each segmented control: another of its buttons chosen.
		const groups = page.locator('.seg:has(.seg-btn:not(.active):not(:disabled))');
		const groupCount = await groups.count();
		expect(groupCount, 'Settings has segmented controls').toBeGreaterThan(0);
		for (let i = 0; i < groupCount; i++) {
			const group = groups.nth(i);
			await group.scrollIntoViewIfNeeded();
			// The button chosen, pictured alone: a label beside the group
			// may change with the choice and is not the control's state.
			const next = group.locator('.seg-btn:not(.active):not(:disabled)').first();
			const label = (await next.textContent())?.trim() ?? '';
			const button = group.locator('.seg-btn', { hasText: exactly(label) }).first();
			await shows(
				page,
				button,
				`segmented control ${i + 1}, ${label} chosen`,
				() => button.click(),
				6
			);
		}

		// A chosen button's separator, which no longer takes the theme's
		// colour once the button opts out of it, takes the highlight.
		const chosenSep = page.locator('.seg .seg-btn.active:not(:last-child)');
		await expect(chosenSep.first()).toBeVisible();
		expect
			.soft(
				await computed(chosenSep, 'border-inline-end-color'),
				"a chosen button's separator is the highlight"
			)
			.toBe(await systemColour(page, 'Highlight'));

		// A show's Sub/Dub control.
		await page.locator('a.brand').click();
		await page.locator(`a[href$="/anime/${topRated[0].id}"]`).first().click();
		await expect.poll(() => page.url()).toContain(`/anime/${topRated[0].id}`);
		const audio = page.locator('.seg-group');
		await expect(audio.locator('.seg-btn').first()).toBeVisible({ timeout: 15_000 });
		const other = audio.locator('.seg-btn:not(.active):not(:disabled)').first();
		if (await other.count()) {
			const label = (await other.textContent())?.trim() ?? '';
			const button = audio.locator('.seg-btn', { hasText: exactly(label) }).first();
			await shows(
				page,
				button,
				`a show's Sub/Dub control, ${label} chosen`,
				() => button.click(),
				6
			);
		} else {
			// One of the two is chosen and the other disabled: the chosen
			// one must look unlike its disabled sibling, beyond its label.
			const chosen = audio.locator('.seg-btn.active');
			await expect(chosen, 'a Sub/Dub button is chosen').toHaveCount(1);
			await shows(
				page,
				chosen,
				"a show's chosen Sub/Dub button",
				async () => {
					await chosen.evaluate((el) => el.classList.remove('active'));
				},
				6
			);
		}

		// The download bar's progress, shown and hidden.
		await page.evaluate(() => {
			class SilentSource {
				readonly url: string;
				readyState = 1;
				constructor(url: string) {
					this.url = url;
				}
				addEventListener() {}
				removeEventListener() {}
				close() {
					this.readyState = 2;
				}
			}
			(window as unknown as { EventSource: unknown }).EventSource = SilentSource;
		});
		const message = (file: string, key: string) =>
			JSON.parse(
				fs.readFileSync(
					path.resolve(__dirname, '../../frontend/messages/en', `${file}.json`),
					'utf8'
				)
			)[key] as string;
		await page
			.getByRole('button', { name: message('detail', 'download_button'), exact: true })
			.click();
		const dialog = page.getByRole('dialog');
		// The dialog's mode buttons: Range chosen.
		const range = dialog.locator('.dl-mode-btn', {
			hasText: exactly(message('download', 'mode_range'))
		});
		await expect(range).toBeVisible();
		await shows(page, range, "the download dialog's Range mode, chosen", () => range.click(), 6);
		const highlightText = await systemColour(page, 'HighlightText');
		for (const words of await range.locator('*').all()) {
			if (!(await words.evaluate((el) => (el.textContent ?? '').trim().length > 0))) continue;
			expect
				.soft(await computed(words, 'color'), "the chosen mode's words are the highlight's text")
				.toBe(highlightText);
		}
		await dialog.getByRole('button', { name: message('download', 'button_confirm') }).click();
		const bar = page.locator('.dl-bar-progress');
		await expect(bar).toBeVisible({ timeout: 15_000 });
		expect
			.soft(await computed(bar.locator('span'), 'opacity'), "the download bar's progress is whole")
			.toBe('1');
		await shows(page, bar, "the download bar's progress", () =>
			hidden(bar.locator('span').first(), true)
		);

		// The download list's bar for that download.
		await page.locator('.dl-dock-trigger').click();
		const rowBar = page.locator('.dl-row-bar').first();
		await expect(rowBar).toBeVisible();
		expect
			.soft(await computed(rowBar.locator('span'), 'opacity'), "the download list's bar is whole")
			.toBe('1');
		await shows(page, rowBar, "the download list's bar", () =>
			hidden(rowBar.locator('span').first(), true)
		);
		await page.keyboard.press('Escape');

		// The topbar's live results: the row the arrow keys pick.
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const hits = topRated.map((s, i) => ({ ...s, id: `${s.id}${i}` }));
			if (p === '/api/kitsu/search') {
				return answer(r, {
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify(hits)
				});
			}
			if (p === '/api/kitsu/search/page') {
				return answer(r, {
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({ hits, next_offset: null })
				});
			}
			return r.fallback();
		});
		const field = page.locator('input[type="search"]').first();
		await field.fill('fullmetal');
		const firstHit = page.locator('.topbar-hit').first();
		await expect(firstHit).toBeVisible({ timeout: 15_000 });
		await shows(page, firstHit, "the topbar's picked result", () => field.press('ArrowDown'), 6, {
			keepFocus: true
		});
		for (const words of await firstHit.locator('*').all()) {
			if (!(await words.evaluate((el) => (el.textContent ?? '').trim().length > 0))) continue;
			expect
				.soft(await computed(words, 'color'), "the picked result's words are the highlight's text")
				.toBe(await systemColour(page, 'HighlightText'));
		}
		await field.press('Escape');

		// The search page's sort and type chips: one chosen.
		await field.fill('fullmetal');
		await field.press('Enter');
		await expect.poll(() => page.url()).toContain('/search');
		// Found by its words, so the pictures follow the same chip.
		const unsorted = page.locator('.chips[role="radiogroup"] .chip[aria-checked="false"]').first();
		await expect(unsorted).toBeVisible({ timeout: 15_000 });
		const sortLabel = (await unsorted.textContent())?.trim() ?? '';
		const sort = page.locator('.chips[role="radiogroup"] .chip', { hasText: exactly(sortLabel) });
		await shows(page, sort, 'a search sort, chosen', () => sort.click(), 6);
		const typeChip = page.locator('.chips[role="group"] .chip:not(.chip-clear)').first();
		await shows(page, typeChip, 'a search type filter, chosen', () => typeChip.click(), 6);

		// The search's progress bar, while a search is out.
		const hold = '**/api/kitsu/search/page';
		await page.route(hold, () => {
			// Held: the search stays in flight.
		});
		// A fresh page for a new query, so no pick in the topbar's results
		// can take Enter elsewhere.
		await page.goto(new URL('/search?q=bebop', page.url()).href, {
			waitUntil: 'domcontentloaded'
		});
		await forcedColours(page);
		const searching = page.locator('.search-progress');
		await expect(searching).toBeVisible({ timeout: 15_000 });
		expect
			.soft(
				await computed(searching.locator('.search-progress-bar'), 'opacity'),
				"the search's progress bar is whole"
			)
			.toBe('1');
		await shows(page, searching, "the search's progress bar", () =>
			hidden(searching.locator('.search-progress-bar'), true)
		);
		await page.unroute(hold);
		await cdp.detach().catch(() => {});
	} finally {
		await stopApp(app);
	}
});

/** A server for the captions media fixture (see player-start.spec.ts). */
async function mediaServer(): Promise<{ origin: string; close: () => void }> {
	const dir = path.join(__dirname, 'fixtures', 'captions');
	const types: Record<string, string> = {
		'.m3u8': 'application/vnd.apple.mpegurl',
		'.ts': 'video/mp2t'
	};
	const server = http.createServer((req, res) => {
		const name = path.basename(new URL(req.url ?? '/', 'http://x').pathname);
		const file = path.join(dir, name);
		if (!existsSync(file)) {
			res.writeHead(404).end();
			return;
		}
		const body = fs.readFileSync(file);
		res.writeHead(200, {
			'content-type': types[path.extname(name)] ?? 'application/octet-stream',
			'content-length': body.length
		});
		res.end(body);
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

test("the player's timeline and volume keep their fill in a forced-colours theme", async () => {
	test.setTimeout(180_000);
	const media = await mediaServer();
	try {
		const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-forced-state-player-'));
		writeReplayConfig(profile, 'locale = "en"\n');
		const app = await launchPackagedApp(
			{
				...process.env,
				XDG_STATE_HOME: path.join(profile, 'state'),
				XDG_CONFIG_HOME: path.join(profile, 'config'),
				XDG_CACHE_HOME: path.join(profile, 'cache'),
				XDG_DATA_HOME: path.join(profile, 'data'),
				ANI_GUI_UNATTENDED: '1'
			},
			{ profile }
		);
		try {
			const page = await appWindow(app);
			const play = await routeRealPlay(page, {
				profile,
				settings: { locale: 'en', use_custom_player_controls: true },
				stream: { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: 'hls' }
			});
			await sizeViewport(page, 1440, 900);
			await forcedColours(page);
			await playFromContinue(page, play);
			const scrubber = page.locator('.pc-scrubber');
			await expect(scrubber).toBeVisible({ timeout: 30_000 });
			// Halfway through, paused, so the fill is there to see.
			await page.waitForFunction(() => (document.querySelector('video')?.duration ?? 0) > 2, {
				timeout: 30_000
			});
			await page.evaluate(() => {
				const video = document.querySelector('video') as HTMLVideoElement;
				video.pause();
				video.currentTime = video.duration / 2;
			});
			// The mouse over the player, where it stays for the pictures.
			const box = await page.locator('section.player-frame').boundingBox();
			if (box) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			// A part hidden, then shown.
			const apart = async (frame: Locator, part: Locator, what: string) => {
				const before = await frame.screenshot();
				await hidden(part, true);
				await page.evaluate(
					() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
				);
				const after = await frame.screenshot();
				expect
					.soft(await pixelsApart(page, before, after), `${what} shows in a forced-colours theme`)
					.toBeGreaterThanOrEqual(SEEN);
			};
			// The controls show while the pointer is over the player or
			// focus is in them. On CI they had faded by the time the
			// timeline was pictured with the pointer alone; the timeline's
			// focus holds them up, and their opacity is checked.
			await scrubber.focus();
			await expect(page.locator('.player-controls')).toHaveCSS('opacity', '1');
			const fill = page.locator('.pc-scrubber-fill');
			await expect(fill).toBeVisible();
			await apart(scrubber, fill, "the timeline's fill");
			// The track the fill runs along, with the fill hidden: the length
			// of the episode, against which the fill reads.
			await apart(scrubber, page.locator('.pc-scrubber-track'), "the timeline's track");
			await hidden(page.locator('.pc-scrubber-track'), false);
			await hidden(fill, false);

			// The volume, revealed by its field's focus.
			const volume = page.locator('.pc-volume');
			await page.locator('.pc-volume-slider input').focus();
			await expect(page.locator('.player-controls')).toHaveCSS('opacity', '1');
			const vFill = page.locator('.pc-volume-fill');
			await expect(vFill).toBeVisible();
			await apart(volume, vFill, "the volume's fill");
			await apart(volume, page.locator('.pc-volume-track'), "the volume's track");
		} finally {
			await stopApp(app);
		}
	} finally {
		media.close();
	}
});
