/**
 * Each row of the downloads popover keeps one shape, whatever it holds.
 *
 * The popover lists every download: its title and episode, then where
 * it stands (a bar while it runs, "Queued" while it waits, a mark when
 * it failed) and its actions (cancel; reveal and dismiss once done;
 * dismiss once failed). Each row is that one line — the title cut with
 * an ellipsis to make room, the episode beside it, the state and the
 * actions at the line's end in the same place on every row — with what
 * an ended download still has to say on a second line under it.
 *
 * A row that ran out of room used to break instead: the row wrapped, so
 * a long title went to a line of its own, pushed to the end, with the
 * bar and the cancel under it at the start; and a title that nearly
 * fitted kept the bar beside it and sent the cancel to a line alone.
 *
 * Layout is the subject, so this runs in the packaged app's Chromium.
 * Downloads are started the way a reader starts them — from a show's
 * page, through the confirm dialog — and their streams are played by
 * the test: the page's EventSource is replaced, before the first
 * download, by one whose events the test sends, so a download stays
 * running, reports its episode and speed, finishes or fails when the
 * test says. Every API request is stubbed.
 *
 * The popover is a fixed width, so the window sizes (the smallest the
 * app allows, a common desktop one) check that it stays placed; the
 * rows inside it are the same at both. The display scales Windows
 * offers by default (125 % and 150 %) check sub-pixel rounding only:
 * Chromium on Linux does not rasterize or measure text as Windows
 * does. Letters spaced a fifth wider stand in for a wider face on
 * another platform (Segoe UI on Windows is not on a Linux runner).
 * Every locale the app ships is left-to-right, and the geometry is
 * read that way: a right-to-left locale would fail here, not pass.
 */
import { type CDPSession, type ElectronApplication, type Page } from '@playwright/test';
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

const show = (id: string, title: string, episodes: number) => ({
	...topRated[0],
	id,
	canonical_title: title,
	english_title: title,
	episode_count: episodes,
	cover_image: null
});

/** The shows downloads are started from. The first three are the
 *  titles the broken rows were reported with; the fourth is longer
 *  than any row has room for. */
const SHOWS = {
	gintama: show('9101', 'Gintama.', 12),
	bleach: show('9102', 'BLEACH: Sennen Kessen-hen - Ketsubetsu-tan', 14),
	chainsaw: show('9103', 'Chainsaw Man: Reze-hen', 1),
	long: show(
		'9104',
		'Kono Subarashii Sekai ni Shukufuku wo! 3: Bonus Stage of an Extraordinarily Long Name',
		12
	),
	frieren: show('9105', "Frieren: Beyond Journey's End", 28)
} as const;
const ALL_SHOWS = Object.values(SHOWS);

/** A message from the shipped catalogue. */
function message(locale: string, file: string, key: string): string {
	const p = path.resolve(__dirname, '../../frontend/messages', locale, `${file}.json`);
	return JSON.parse(fs.readFileSync(p, 'utf8'))[key] as string;
}

async function launchOnce(locale: string, onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-dl-rows-${process.pid}-${Date.now()}`);
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
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') {
			return r.request().method() === 'PUT'
				? answer(r, { status: 204 })
				: j({ ...defaultSettings, locale });
		}
		// The hero's pick is the backend's own, made from the stubbed list.
		if (p === '/api/home/hero') return r.continue();
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(ALL_SHOWS);
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
		if (p.startsWith('/api/kitsu/anime/')) {
			const id = p.split('/').pop();
			return j(ALL_SHOWS.find((s) => s.id === id) ?? null);
		}
		if (p.startsWith('/api/kitsu/episodes/')) return j([]);
		if (p === '/api/download/default-dir') return j('/tmp/ani-gui-e2e-downloads');
		if (p === '/api/availability') {
			const title = r.request().postDataJSON()?.title as string | undefined;
			const s = ALL_SHOWS.find((x) => x.canonical_title === title);
			return j({ available: true, episode_count: s?.episode_count ?? 12, extra_episodes: [] });
		}
		if (p === '/api/kitsu/search') return j([]);
		return j(null);
	});

	const homeUrl = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	await installStreams(page);
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

/** Replaces the page's EventSource with one the test plays: each
 *  download's stream is kept, in the order the downloads started, and
 *  `emit` sends it an event. Nothing reaches the network. */
async function installStreams(page: Page) {
	await page.evaluate(() => {
		type Listener = (ev: MessageEvent) => void;
		class PlayedSource {
			readonly url: string;
			readyState = 1;
			private listeners = new Map<string, Listener[]>();
			constructor(url: string) {
				this.url = url;
				// Only the downloads' streams are played; any other (a
				// play's, which a show's page starts ahead of a click)
				// stays open and silent.
				if (new URL(url).pathname === '/api/download/stream') {
					(window as unknown as { __dlStreams: PlayedSource[] }).__dlStreams.push(this);
				}
			}
			addEventListener(type: string, fn: Listener) {
				this.listeners.set(type, [...(this.listeners.get(type) ?? []), fn]);
			}
			removeEventListener() {}
			close() {
				this.readyState = 2;
			}
			emit(type: string, data: unknown) {
				const ev = new MessageEvent(type, { data: JSON.stringify(data) });
				for (const fn of this.listeners.get(type) ?? []) fn(ev);
			}
		}
		const w = window as unknown as { __dlStreams: unknown[]; EventSource: unknown };
		w.__dlStreams = [];
		w.EventSource = PlayedSource;
	});
}

async function streamCount(page: Page): Promise<number> {
	return page.evaluate(() => (window as unknown as { __dlStreams: unknown[] }).__dlStreams.length);
}

async function emit(page: Page, index: number, type: string, data: unknown) {
	await page.evaluate(
		([i, t, d]) => {
			const streams = (
				window as unknown as { __dlStreams: { emit(t: string, d: unknown): void }[] }
			).__dlStreams;
			streams[i as number].emit(t as string, d);
		},
		[index, type, data] as const
	);
}

const progress = (page: Page, index: number, line: string) =>
	emit(page, index, 'progress', { line });

/** Opens `s`'s page from Home, the way a reader does, and starts a
 *  download of its first episode, or of all of it. */
async function startDownload(
	page: Page,
	locale: string,
	s: (typeof ALL_SHOWS)[number],
	all: boolean
): Promise<void> {
	await page.mouse.click(1, 1);
	if (!page.url().includes(`/anime/${s.id}`)) {
		await page.locator('a.brand').click();
		const card = page.locator(`a[href$="/anime/${s.id}"]`).first();
		await expect(card).toBeVisible({ timeout: 15_000 });
		await card.click();
		await expect.poll(() => page.url(), { timeout: 10_000 }).toContain(`/anime/${s.id}`);
	}
	const download = page.getByRole('button', {
		name: message(locale, 'detail', 'download_button'),
		exact: true
	});
	await expect(download).toBeEnabled({ timeout: 15_000 });
	await download.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	// The show page's dialog opens on All when the show's count is known;
	// one episode is a range of one.
	if (!all) {
		await dialog.getByRole('button', { name: message(locale, 'download', 'mode_range') }).click();
		await dialog
			.getByRole('spinbutton', { name: message(locale, 'download', 'range_to_aria_label') })
			.fill('1');
		await dialog
			.getByRole('spinbutton', { name: message(locale, 'download', 'range_from_aria_label') })
			.fill('1');
	}
	await dialog.getByRole('button', { name: message(locale, 'download', 'button_confirm') }).click();
	await expect(dialog).toHaveCount(0);
}

/** One DevTools session per page, kept open: an override lasts only as
 *  long as the session that set it. */
const sessions = new WeakMap<Page, CDPSession>();

/** Lays the page out at `width` × `height` and `scale`. */
async function sizeViewport(page: Page, width: number, height: number, scale: number) {
	let cdp = sessions.get(page);
	if (!cdp) {
		cdp = await page.context().newCDPSession(page);
		sessions.set(page, cdp);
	}
	await cdp.send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: scale,
		mobile: false
	});
	await expect
		.poll(() => page.evaluate(() => [window.innerWidth, window.devicePixelRatio]))
		.toEqual([width, scale]);
}

async function widerLetters(page: Page, wider: boolean) {
	// A constructed sheet: the window's policy refuses inline style.
	await page.evaluate((on) => {
		const doc = document as Document & { dlRowsWider?: CSSStyleSheet };
		const sheets = document.adoptedStyleSheets.filter((s) => s !== doc.dlRowsWider);
		if (on) {
			doc.dlRowsWider ??= new CSSStyleSheet();
			doc.dlRowsWider.replaceSync('.dl-dock-pop { letter-spacing: 0.12em; }');
			sheets.push(doc.dlRowsWider);
		}
		document.adoptedStyleSheets = sheets;
	}, wider);
}

async function openDock(page: Page) {
	const pop = page.locator('#dl-dock-pop');
	if ((await pop.count()) === 0) await page.locator('.dl-dock-trigger').click();
	await expect(pop).toBeVisible();
	return pop;
}

interface Box {
	left: number;
	right: number;
	top: number;
	bottom: number;
}
interface RowGeometry {
	status: string;
	row: Box;
	/** Inside the row's border: the rows after the first carry a rule
	 *  above them that the first does not. */
	rowHeight: number;
	text: Box;
	title: Box;
	ep: Box;
	mark: Box | null;
	actions: Box[];
	report: Box | null;
}

/** Every row's parts, and the popover's own box, read in one frame. */
function geometry(page: Page) {
	return page.locator('#dl-dock-pop').evaluate((pop) => {
		const box = (el: Element | null): Box | null => {
			if (!el) return null;
			const r = el.getBoundingClientRect();
			return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
		};
		const popBox = pop.getBoundingClientRect();
		const cs = getComputedStyle(pop);
		const inner = {
			left: popBox.left + parseFloat(cs.borderLeftWidth),
			right: popBox.left + parseFloat(cs.borderLeftWidth) + pop.clientWidth,
			top: popBox.top,
			bottom: popBox.bottom
		};
		const rows: RowGeometry[] = Array.from(pop.querySelectorAll('li.dl-row')).map((li) => {
			const ep = li.querySelector('.dl-row-ep') as HTMLElement;
			return {
				status: li.querySelector(':scope > .dl-row-bar')
					? 'running'
					: li.querySelector(':scope > span.dl-row-queued')
						? 'queued'
						: li.querySelector(':scope > span.dl-row-error')
							? 'failed'
							: 'done',
				row: box(li)!,
				rowHeight: li.clientHeight,
				text: box(li.querySelector('.dl-row-text'))!,
				title: box(li.querySelector('.dl-row-title'))!,
				ep: box(ep)!,
				mark: box(
					li.querySelector(
						':scope > .dl-row-bar, :scope > span.dl-row-queued, :scope > span.dl-row-error'
					)
				),
				actions: Array.from(li.querySelectorAll('.dl-row-act')).map((b) => box(b)!),
				report: box(li.querySelector('.dl-row-report'))
			};
		});
		return { inner, overflowX: pop.scrollWidth - pop.clientWidth, rows };
	});
}

type Geometry = Awaited<ReturnType<typeof geometry>>;

const EPS = 0.75;

/** The row shape the popover promises, asserted of every row. Soft, so
 *  one run reports every view and state that breaks it, with its shot. */
function assertRows(g: Geometry, at: string, count: number) {
	expect(g.rows.length, `${at}: the rows are on show`).toBe(count);
	expect.soft(g.overflowX, `${at}: nothing scrolls sideways in the popover`).toBeLessThanOrEqual(0);
	for (const [i, r] of g.rows.entries()) {
		const where = `${at}, row ${i} (${r.status}): ${JSON.stringify(r)}`;
		// The row sits inside the popover.
		expect.soft(r.row.left, where).toBeGreaterThanOrEqual(g.inner.left - EPS);
		expect.soft(r.row.right, where).toBeLessThanOrEqual(g.inner.right + EPS);
		// One line: the text, the state and the actions overlap one band.
		const line = [r.text, ...(r.mark ? [r.mark] : []), ...r.actions];
		const bandTop = Math.max(...line.map((b) => b.top));
		const bandBottom = Math.min(...line.map((b) => b.bottom));
		expect.soft(bandTop, `${where}: the row's parts share one line`).toBeLessThan(bandBottom);
		// In order along it: text, then the state, then the actions.
		const order = [r.text, ...(r.mark ? [r.mark] : []), ...r.actions];
		for (let k = 1; k < order.length; k++) {
			expect
				.soft(order[k].left, `${where}: part ${k} follows part ${k - 1}`)
				.toBeGreaterThanOrEqual(order[k - 1].right - EPS);
		}
		// Everything inside the row.
		for (const b of [...line, r.title, r.ep]) {
			expect.soft(b.left, where).toBeGreaterThanOrEqual(r.row.left - EPS);
			expect.soft(b.right, where).toBeLessThanOrEqual(r.row.right + EPS);
		}
		// The episode follows the title, whole, inside the text (which
		// clips what leaves it), and some of the title stays on show.
		expect.soft(r.title.right - r.title.left, `${where}: the title shows`).toBeGreaterThan(32);
		expect.soft(r.ep.left, where).toBeGreaterThanOrEqual(r.title.right - EPS);
		expect.soft(r.ep.right, where).toBeLessThanOrEqual(r.text.right + EPS);
		// The text starts the row.
		expect.soft(r.text.left - r.row.left, `${where}: the text starts the row`).toBeLessThan(16);
		// What an ended row has to say goes under the line.
		if (r.report) {
			expect
				.soft(r.report.top, where)
				.toBeGreaterThanOrEqual(Math.max(...line.map((b) => b.bottom)) - EPS);
			expect.soft(r.report.right, where).toBeLessThanOrEqual(r.row.right + EPS);
		}
	}
	// Every row's last action is at the same place, and every state too.
	const ends = g.rows.map((r) => r.actions[r.actions.length - 1].right);
	expect
		.soft(Math.max(...ends) - Math.min(...ends), `${at}: the last actions line up`)
		.toBeLessThan(1);
	const marks = g.rows.filter((r) => r.mark).map((r) => r.mark!.right);
	if (marks.length > 1) {
		expect
			.soft(Math.max(...marks) - Math.min(...marks), `${at}: the states line up`)
			.toBeLessThan(1);
	}
	// A queued row's label is at least a bar's width: it starts where a
	// running row's bar does, so its title is cut at the same place,
	// unless its words need more room, when it starts before the bar.
	const bars = g.rows.filter((r) => r.status === 'running').map((r) => r.mark!);
	for (const q of g.rows.filter((r) => r.status === 'queued')) {
		for (const bar of bars) {
			const label = q.mark!;
			const where = `${at}: the Queued label ${JSON.stringify(label)} against a bar ${JSON.stringify(bar)}`;
			expect
				.soft(label.right - label.left, where)
				.toBeGreaterThanOrEqual(bar.right - bar.left - EPS);
			expect.soft(label.left, where).toBeLessThanOrEqual(bar.left + EPS);
			if (label.right - label.left <= bar.right - bar.left + EPS) {
				expect.soft(Math.abs(label.left - bar.left), `${where}: starts with it`).toBeLessThan(1);
			}
		}
	}
	// Rows with nothing under their line are all one height.
	const heights = g.rows.filter((r) => !r.report).map((r) => r.rowHeight);
	expect
		.soft(Math.max(...heights) - Math.min(...heights), `${at}: one-line rows match`)
		.toBeLessThan(1);
}

const views = [
	{ label: 'smallest', width: 960, height: 600, scale: 1, wider: false },
	{ label: 'desktop', width: 1440, height: 900, scale: 1, wider: false },
	{ label: 'scale-125', width: 1280, height: 720, scale: 1.25, wider: false },
	{ label: 'scale-150', width: 1280, height: 720, scale: 1.5, wider: false },
	{ label: 'wider-face', width: 960, height: 600, scale: 1, wider: true }
] as const;

async function checkViews(page: Page, name: string, count: number) {
	const testInfo = test.info();
	for (const view of views) {
		await sizeViewport(page, view.width, view.height, view.scale);
		await widerLetters(page, view.wider);
		const pop = await openDock(page);
		// The view is the one asked for.
		expect(
			await page.evaluate(() => [window.innerWidth, window.devicePixelRatio]),
			`${name}, ${view.label}: the view`
		).toEqual([view.width, view.scale]);
		const g = await geometry(page);
		const at = `${name}, ${view.label}`;
		testInfo.annotations.push({ type: 'geometry', description: `${at}: ${JSON.stringify(g)}` });
		await pop.screenshot({ path: testInfo.outputPath(`${name}-${view.label}.png`) });
		// The popover scrolls once the rows outgrow it: its end, too.
		if (await pop.evaluate((el) => el.scrollHeight > el.clientHeight)) {
			await pop.evaluate((el) => (el.scrollTop = el.scrollHeight));
			await pop.screenshot({ path: testInfo.outputPath(`${name}-${view.label}-end.png`) });
			await pop.evaluate((el) => (el.scrollTop = 0));
		}
		assertRows(g, at, count);
	}
	await widerLetters(page, false);
	await sizeViewport(page, 1280, 800, 1);
}

for (const locale of ['en', 'pt-BR', 'es-419', 'ru']) {
	test(`every download row keeps one line in ${locale}`, async () => {
		test.setTimeout(240_000);
		const { app, page } = await launch(locale);
		try {
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await sizeViewport(page, 1280, 800, 1);

			// One download alone: a film's only episode, under way.
			await startDownload(page, locale, SHOWS.chainsaw, true);
			await expect.poll(() => streamCount(page)).toBe(1);
			await progress(page, 0, 'Playing episode 1');
			await checkViews(page, `${locale}-one`, 1);

			// Then the rest, one row per state.
			await startDownload(page, locale, SHOWS.bleach, true); // 1: range, under way, with speed
			await progress(page, 1, 'Playing episode 10');
			await progress(page, 1, 'status.download.rate 9856000');
			await startDownload(page, locale, SHOWS.gintama, false); // 2: done
			await emit(page, 2, 'done', { dest_dir: '/tmp/ani-gui-e2e-downloads' });
			await startDownload(page, locale, SHOWS.frieren, false); // 3: done, already here
			await progress(page, 3, 'status.download.already_here');
			await emit(page, 3, 'done', { dest_dir: '/tmp/ani-gui-e2e-downloads' });
			await startDownload(page, locale, SHOWS.long, false); // 4: failed
			await emit(page, 4, 'error', { kind: 'io', key: 'error.io', message: 'disk full' });
			await startDownload(page, locale, SHOWS.long, true); // 5: failed over a claimed file
			await progress(
				page,
				5,
				`status.download.abandoned_claim /tmp/ani-gui-e2e-downloads/${SHOWS.long.canonical_title}/Episode 1.mp4`
			);
			await emit(page, 5, 'error', { kind: 'io', key: 'error.io', message: 'claimed' });
			// Under way with no report yet, until six run at once
			// (DOWNLOAD_CONCURRENCY in frontend/src/lib/download/queue.ts:
			// change it and the stream counts below fail); the seventh
			// waits in the queue.
			await startDownload(page, locale, SHOWS.long, false); // 6
			await startDownload(page, locale, SHOWS.gintama, true); // 7
			await startDownload(page, locale, SHOWS.frieren, false); // 8
			await startDownload(page, locale, SHOWS.frieren, true); // 9
			await expect.poll(() => streamCount(page)).toBe(10);
			await startDownload(page, locale, SHOWS.bleach, false); // queued
			await expect.poll(() => streamCount(page)).toBe(10);

			const pop = await openDock(page);
			// Told apart by what each shows where it stands.
			const rows = pop.locator('li.dl-row');
			await expect(rows).toHaveCount(11);
			await expect(rows.filter({ has: page.locator(':scope > span.dl-row-queued') })).toHaveCount(
				1
			);
			await expect(rows.filter({ has: page.locator(':scope > .dl-row-bar') })).toHaveCount(6);
			await expect(rows.filter({ has: page.locator(':scope > span.dl-row-error') })).toHaveCount(2);
			await expect(rows.filter({ has: page.locator(':scope > .dl-row-report') })).toHaveCount(2);
			await checkViews(page, `${locale}-all`, 11);
		} finally {
			await stopApp(app);
		}
	});
}
