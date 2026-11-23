/**
 * An episode tile's meta line fits its tile, in every locale.
 *
 * Under its title, an episode tile on a show's page, and an episode
 * card in the player's strip, say one short thing: the episode's
 * length, when an unaired one airs ("Выйдет 28 сент."), or that the
 * streaming catalogue does not list one yet ("Пока нет в каталоге").
 * The words differ in length from locale to locale, and the tile is at
 * its narrowest at the smallest window the app opens.
 *
 * Layout is the subject, so this runs in the packaged app's Chromium,
 * at the smallest window (electron/lib/app-window.cjs), in each of the
 * app's four locales. A show of twelve episodes is answered so that
 * the catalogue lists seven, nine have aired and the tenth airs on the
 * 28th of September: episodes 8 and 9 are not listed yet, 10 gives its
 * airing date (September's short name is the longest in Spanish and
 * ties the longest in Russian; in English and Portuguese every short
 * month has the same letters) and
 * 11 and 12 are unaired. For each of those tiles, the meta line's words
 * sit on one line, inside the tile, and the line does not overflow its
 * box. A card in the player's strip is narrower (five to a row): its
 * episode number keeps one line, and its words may take two, inside the
 * card and without overflowing, with the dot before them on their line,
 * not left at the end of the number's. Episodes 6 and 7, aired and
 * listed, give their length (6 is the one playing).
 *
 * Every API request is stubbed, but for the player's play and stream,
 * which run on the backend's replay of a seeded row (see real-play.ts).
 */
import { type ElectronApplication, type Locator, type Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import type net from 'node:net';
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
import { appWindow, launchPackagedApp, packagedBinary, stopApp, stopDeadApp } from './launch';
import { playFromContinue, routeRealPlay, writeReplayConfig } from './real-play';
import { answer } from './stubs';

const LOCALES = ['en', 'pt-BR', 'es-419', 'ru'] as const;

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

/** The show: airing, twelve episodes. */
const show = { ...continueKitsuMatch, status: 'current', episode_count: 12 };

/** 28 September, next year: a month whose short name is long. */
const AIRS_AT = Math.floor(Date.UTC(new Date().getUTCFullYear() + 1, 8, 28, 12) / 1000);

const episodes = Array.from({ length: 12 }, (_, i) => ({
	...continueKitsuEpisode6,
	id: `ep-${i + 1}`,
	number: i + 1,
	relative_number: i + 1,
	canonical_title: `Episode ${i + 1}`,
	length: 24,
	// A picture on every tile, so the meta line carries the words.
	thumbnail: {
		...continueKitsuEpisode6.thumbnail,
		original: `https://example.invalid/${i + 1}.jpg`
	}
}));

const airing = {
	aired: 9,
	next_episode: 10,
	next_airing_at: AIRS_AT,
	upcoming: [{ episode: 10, airing_at: AIRS_AT }]
};

/** Answers the show, its episodes, the catalogue's count and the
 *  schedule; every other request falls back to the routes before. */
async function routeShow(page: Page) {
	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		if (p === `/api/kitsu/anime/${show.id}`) return j(show);
		if (p.startsWith('/api/kitsu/episodes/')) return j(episodes);
		if (p.startsWith('/api/kitsu/airing/')) return j(airing);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 7, extra_episodes: [] });
		}
		return r.fallback();
	});
}

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

/** The meta line of `tile` against the tile's box: how many lines its
 *  words take, how far they reach past the tile's sides, and how far
 *  the line's content overflows its own box. */
function fit(tile: Locator, meta: string) {
	return tile.evaluate((el, sel) => {
		const line = el.querySelector(sel) as HTMLElement | null;
		if (!line) return null;
		const rects: DOMRect[] = [];
		const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
		for (let t = walker.nextNode(); t; t = walker.nextNode()) {
			const range = document.createRange();
			range.selectNodeContents(t);
			rects.push(...Array.from(range.getClientRects()).filter((r) => r.width > 0));
		}
		// Lines: a rect below everything on the line before starts a new
		// one. Words in two faces on one baseline overlap vertically
		// without sharing a top.
		rects.sort((a, b) => a.top - b.top);
		let lines = 0;
		let bottom = -Infinity;
		for (const r of rects) {
			if (r.top >= bottom - 2) {
				lines++;
				bottom = r.bottom;
			} else bottom = Math.max(bottom, r.bottom);
		}
		const left = Math.min(...rects.map((r) => r.left));
		const right = Math.max(...rects.map((r) => r.right));
		const box = el.getBoundingClientRect();
		return {
			text: line.textContent?.trim() ?? '',
			rects: rects.length,
			lines,
			pastLeft: box.left - left,
			pastRight: right - box.right,
			overflow: line.scrollWidth - line.clientWidth
		};
	}, meta);
}

async function assertFits(tile: Locator, meta: string, at: string, maxLines = 1) {
	const f = await fit(tile, meta);
	expect(f, `${at}: the meta line is there`).not.toBeNull();
	if (!f) return;
	const where = `${at} "${f.text}"`;
	expect.soft(f.text.length, `${where}: says something`).toBeGreaterThan(0);
	expect.soft(f.rects, `${where}: its words are drawn`).toBeGreaterThan(0);
	expect
		.soft(f.lines, `${where}: on ${maxLines === 1 ? 'one line' : `at most ${maxLines} lines`}`)
		.toBeLessThanOrEqual(maxLines);
	expect.soft(f.pastLeft, `${where}: inside the tile on the left`).toBeLessThanOrEqual(0.5);
	expect.soft(f.pastRight, `${where}: inside the tile on the right`).toBeLessThanOrEqual(0.5);
	expect.soft(f.overflow, `${where}: does not overflow its line`).toBeLessThanOrEqual(0);
}

async function launchShow(locale: string) {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const tmp = path.join(os.tmpdir(), `ani-gui-meta-fit-${process.pid}-${Date.now()}`);
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
			pending = app;
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
				if (p === '/api/kitsu/trending-anilist') return j(trending);
				if (p === '/api/kitsu/top-rated') return j(topRated);
				if (p.startsWith('/api/image')) return answer(r, { status: 503 });
				if (p === '/api/history') return j([]);
				if (p === '/api/update-check') return answer(r, { status: 204 });
				return j(null);
			});
			await routeShow(page);
			pending = null;
			return { app, page };
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

for (const locale of LOCALES) {
	test(`a show's episode tiles fit their meta line at the smallest window (${locale})`, async () => {
		test.setTimeout(120_000);
		const { app, page } = await launchShow(locale);
		try {
			await sizeViewport(page, 960, 600);
			await page.goto(new URL(`/anime/${show.id}`, page.url()).href, {
				waitUntil: 'domcontentloaded'
			});
			const tile = (n: number) => page.locator(`li[data-ep-num="${n}"] .ep-tile`);
			// The catalogue's count and the schedule are in.
			await expect(tile(8)).toHaveClass(/ep-tile-recheck/, { timeout: 15_000 });
			await expect(tile(10)).toHaveClass(/ep-tile-unaired/, { timeout: 15_000 });
			for (const n of [6, 7, 8, 9, 10, 11, 12]) {
				await tile(n).scrollIntoViewIfNeeded();
				await assertFits(tile(n), '.ep-meta', `${locale}, a show's episode ${n}`);
			}
		} finally {
			await stopApp(app);
		}
	});
}

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

for (const locale of LOCALES) {
	test(`the player's episode cards fit their meta line at the smallest window (${locale})`, async () => {
		test.setTimeout(150_000);
		const media = await mediaServer();
		try {
			const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-meta-fit-player-'));
			writeReplayConfig(profile, `locale = "${locale}"\n`);
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
					settings: { locale, use_custom_player_controls: true },
					stream: { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: 'hls' },
					show: { match: show, episode: episodes[5] }
				});
				await routeShow(page);
				await sizeViewport(page, 960, 600);
				await playFromContinue(page, play);
				// The strip shows the page of five holding the episode on
				// (6): 6 to 10.
				const card = (n: number) => page.locator(`li[data-ep-num="${n}"] .ep-card`);
				await expect(card(8)).toHaveClass(/ep-card-recheck/, { timeout: 30_000 });
				await expect(card(10)).toHaveClass(/ep-card-unaired/, { timeout: 15_000 });
				for (const n of [6, 7, 8, 9, 10]) {
					await card(n).scrollIntoViewIfNeeded();
					// A card is narrow at this window (five to a row): its
					// number keeps one line, and its words may take a second,
					// inside the card, never past it.
					await assertFits(card(n), '.ep-card-foot-num', `${locale}, the player's episode ${n}`);
					await assertFits(
						card(n),
						'.ep-card-foot-len',
						`${locale}, the player's episode ${n}'s words`,
						2
					);
					// The separator goes with the words: no line ends on it. It
					// is either an element of its own, which must sit on the
					// words' first line, or drawn before the words, which puts
					// it there.
					const dot = await card(n).evaluate((el) => {
						const w = el.querySelector('.ep-card-foot-len');
						if (!w) return { shown: false, apart: 0 };
						const d = el.querySelector('.ep-card-foot-dot');
						if (!d) {
							const before = getComputedStyle(w, '::before').content;
							return { shown: before.includes('·'), apart: 0 };
						}
						const range = document.createRange();
						range.selectNodeContents(w);
						const first = Array.from(range.getClientRects()).find((r) => r.width > 0);
						const dr = d.getBoundingClientRect();
						return {
							shown: true,
							apart: first ? Math.abs(dr.top + dr.height / 2 - (first.top + first.height / 2)) : 0
						};
					});
					expect.soft(dot.shown, `${locale}, the player's episode ${n}: shows its dot`).toBe(true);
					expect
						.soft(
							dot.apart,
							`${locale}, the player's episode ${n}: its dot sits on its words' line`
						)
						.toBeLessThanOrEqual(4);
				}
			} finally {
				await stopApp(app);
			}
		} finally {
			media.close();
		}
	});
}
