/**
 * The renderer's content security policy, in the packaged app.
 *
 * Two policies apply to the app's document: the bundle's <meta>
 * (SvelteKit's `kit.csp`: scripts from the app and SvelteKit's hashed
 * bootstrap) and the header the `app://` protocol handler adds to every
 * response (electron/lib/renderer-csp.cjs: every fetch directive,
 * naming the backend's lanes). These tests prove that both reach the
 * document on every load, that together they refuse what the app never
 * does, and that what the app does — plays through the stream proxy
 * with each kind of subtitle, an MP4 session, the loader — still works under
 * them. The suite-wide guard (./policy-guard.ts) fails each test on any
 * violation it did not declare.
 *
 * The media under fixtures/media is tiny and generated: a 4-second
 * 96x54 H.264 test pattern (ffmpeg `testsrc2`, baseline profile, no
 * audio) cut into 1-second HLS segments, the same as an MP4, and
 * WebVTT cues for a subtitle rendition and a sidecar track.
 *
 * Pre-condition: `pnpm run package:e2e` has produced `dist-e2e/linux-unpacked/`.
 */
import { type ElectronApplication, type Page } from '@playwright/test';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import { rendererPolicy } from '../lib/renderer-csp.cjs';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import type net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { appWindow, launchPackagedApp, packagedBinary, remountFirstPage, stopDeadApp } from './launch';
import { expect, guardPolicy, test, type PolicyGuard } from './policy-guard';
import {
	appInfo,
	continueHistory,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	topRated,
	trending
} from './fixtures/kitsu';
import { playFromContinue, routeRealPlay, writeReplayConfig, type ReplayStream } from './real-play';
import { answer } from './stubs';

const mediaDir = path.join(__dirname, 'fixtures/media');

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-e2e-policy-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	// Replays on, so a play spec's Continue card plays its own stream
	// through the real play path (real-play.ts), in English whatever the
	// host's language: the session's starting subtitle track follows the
	// UI locale, and the sidecar play's English track has to be it.
	writeReplayConfig(tmp, 'locale = "en"\n');
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
	const guard = guardPolicy(app);
	const page = await appWindow(app);
	return { app, page, guard, profile: tmp };
}

async function launch(): Promise<{
	app: ElectronApplication;
	page: Page;
	guard: PolicyGuard;
	profile: string;
}> {
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
					await stopDeadApp(dead);
				}
			}
		}
	);
}

/** The backend's origins, as the preload hands them to the page. */
async function bridge(page: Page): Promise<{ apiBase: string; lanes: Record<string, string> }> {
	await page.waitForFunction(() => Boolean(window.aniGui?.apiBase));
	return page.evaluate(() => ({
		apiBase: window.aniGui!.apiBase!,
		lanes: { ...(window.aniGui!.lanes ?? {}) } as Record<string, string>
	}));
}

/** Both halves of the policy for the document now loaded: the meta
 *  read off the document itself, the header as the protocol handler
 *  answers the same URL again (it answers every request the same way). */
async function policies(page: Page) {
	return page.evaluate(async () => {
		const res = await fetch(location.href);
		const meta = document.querySelectorAll('meta[http-equiv="content-security-policy" i]');
		return {
			header: res.headers.get('content-security-policy'),
			contentType: res.headers.get('content-type'),
			metas: [...meta].map((m) => m.getAttribute('content') ?? '')
		};
	});
}

/** A full document load of `route` in the app, not a client-side one. */
async function loadDocument(page: Page, route: string) {
	await Promise.all([
		page.waitForEvent('load'),
		page.evaluate((r) => location.assign(r), route)
	]);
}

test('both halves of the policy reach the document on every load', async () => {
	const { app, page } = await launch();
	try {
		const backend = await bridge(page);
		expect(Object.keys(backend.lanes).sort()).toEqual(
			['download', 'image', 'probe', 'resolve', 'stream'].sort()
		);
		const expected = rendererPolicy({ backend, devOrigin: null });
		for (const lane of [backend.apiBase, ...Object.values(backend.lanes)]) {
			expect(expected, `${lane} is named`).toContain(lane);
		}

		const check = async (where: string) => {
			const seen = await policies(page);
			expect(seen.header, `the header on ${where}`).toBe(expected);
			expect(seen.contentType, `the content type on ${where}`).toMatch(/^text\/html/);
			expect(seen.metas, `one meta on ${where}`).toHaveLength(1);
			expect(seen.metas[0]).toMatch(/script-src 'self' 'sha256-[A-Za-z0-9+/=]+'/);
			expect(seen.metas[0]).not.toContain('unsafe');
		};
		await check('the first load');
		await loadDocument(page, '/anime/1');
		await check('a deep route');
		await page.reload();
		await check('a reload');
		// A path no file answers on Linux (Windows' filesystem would serve
		// the index for it): whatever it answers carries the policy.
		const variant = await page.evaluate(async () => {
			const res = await fetch('/INDEX.HTML');
			return res.headers.get('content-security-policy');
		});
		expect(variant).toBe(expected);

		// Every asset carries it too, with its own type kept: a module
		// script without its JavaScript type would not run at all.
		const asset = await page.evaluate(async () => {
			const src = document.querySelector('link[rel="modulepreload"]')!.getAttribute('href')!;
			const res = await fetch(src);
			return {
				header: res.headers.get('content-security-policy'),
				type: res.headers.get('content-type')
			};
		});
		expect(asset.header).toBe(expected);
		expect(asset.type).toMatch(/javascript/);

		// SvelteKit's route announcer keeps its one inline style, which the
		// policy allows by hash: it stays off screen after a navigation.
		await page.locator('a[href$="/settings"]').first().click();
		await page.waitForURL(/\/settings$/);
		await expect
			.poll(() =>
				page.evaluate(() => {
					const el = document.getElementById('svelte-announcer');
					return el && el.textContent?.trim() ? getComputedStyle(el).position : null;
				})
			)
			.toBe('absolute');
	} finally {
		await app.close();
	}
});

/** A loopback server the policy does not name, counting what reaches it. */
async function trapServer(): Promise<{ origin: string; hits: () => string[]; close: () => void }> {
	const hits: string[] = [];
	const server = http.createServer((req, res) => {
		hits.push(req.url ?? '');
		res.writeHead(200, { 'content-type': 'text/plain' }).end('reached');
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, hits: () => [...hits], close: () => server.close() };
}

/** A violation line, as Chromium logs it or as the guard's listener
 *  reports it (JSON, escaped once more when relayed through stdout). */
function reported(directive: string, blocked: string): RegExp {
	const q = '\\\\?"';
	const esc = blocked.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&');
	return new RegExp(`${q}directive${q}:${q}${directive}${q},${q}blocked${q}:${q}${esc}`);
}

test('the policy refuses what the app never does', async () => {
	const trap = await trapServer();
	const { app, page, guard } = await launch();
	try {
		await bridge(page);
		await expect(page.locator('#app')).not.toBeEmpty({ timeout: 15_000 });
		const trapPattern = new RegExp(trap.origin.replace(/[.:/]/g, '\\$&'));
		// Chromium's own lines for the trap name its origin; the rest are
		// matched by directive and what was blocked.
		guard.expectViolation(trapPattern);
		guard.expectViolation(/Refused to execute inline script/);
		guard.expectViolation(reported('script-src-elem', 'inline'));
		guard.expectViolation(/Refused to apply inline style/);
		guard.expectViolation(reported('style-src-elem', 'inline'));
		guard.expectViolation(/Refused to evaluate a string as JavaScript/);
		guard.expectViolation(reported('script-src', 'eval'));
		guard.expectViolation(/violates the document's Content Security Policy/);

		const outcome = await page.evaluate(async (trapOrigin) => {
			const seen: { directive: string; blocked: string }[] = [];
			document.addEventListener('securitypolicyviolation', (e) =>
				seen.push({ directive: e.effectiveDirective, blocked: e.blockedURI })
			);
			const settle = (p: Promise<unknown>) =>
				p.then(
					() => 'allowed',
					() => 'refused'
				);
			const w = window as unknown as { __inlineRan?: number; __timerRan?: number };

			const script = document.createElement('script');
			script.textContent = 'window.__inlineRan = 1';
			document.head.append(script);

			const style = document.createElement('style');
			style.textContent = '#app { outline: 7px solid rgb(1, 2, 3) }';
			document.head.append(style);

			// Strings handed to the page's own timers: an evaluation inside
			// page.evaluate itself runs in DevTools' context, which the
			// page's policy does not govern.
			(setTimeout as unknown as (code: string, ms: number) => number)('window.__timerRan = 1', 0);

			const image = await new Promise<string>((resolve) => {
				const img = new Image();
				img.onload = () => resolve('allowed');
				img.onerror = () => resolve('refused');
				img.src = `${trapOrigin}/image.png`;
			});
			const prefetch = document.createElement('link');
			prefetch.rel = 'prefetch';
			prefetch.href = `${trapOrigin}/prefetch`;
			document.head.append(prefetch);
			const frame = document.createElement('iframe');
			frame.src = `${trapOrigin}/frame`;
			document.body.append(frame);
			const connect = await settle(fetch(`${trapOrigin}/fetch`));
			await new Promise((r) => setTimeout(r, 500));
			return {
				connect,
				image,
				inlineScriptRan: w.__inlineRan === 1,
				inlineStyleApplied: getComputedStyle(document.getElementById('app')!).outlineWidth === '7px',
				timerStringRan: w.__timerRan === 1,
				seen
			};
		}, trap.origin);

		expect({ ...outcome, seen: undefined }).toEqual({
			connect: 'refused',
			image: 'refused',
			inlineScriptRan: false,
			inlineStyleApplied: false,
			timerStringRan: false,
			seen: undefined
		});
		// Refused before any connection: the server never heard of it.
		expect(trap.hits()).toEqual([]);
		// A report for each refusal, naming its rule. A frame's report names
		// only the origin it was refused.
		const reports = [
			...new Set(outcome.seen.map((v) => `${v.directive} ${v.blocked.replace(trap.origin, '<trap>')}`))
		];
		expect(reports.sort()).toEqual(
			[
				'connect-src <trap>/fetch',
				'default-src <trap>/prefetch',
				'frame-src <trap>',
				'img-src <trap>/image.png',
				'script-src eval',
				'script-src-elem inline',
				'style-src-elem inline'
			].sort()
		);
	} finally {
		await app.close();
		trap.close();
	}
});

/** A server for the media fixtures, standing in for a provider's CDN. */
async function mediaServer(): Promise<{ origin: string; close: () => void }> {
	const types: Record<string, string> = {
		'.m3u8': 'application/vnd.apple.mpegurl',
		'.ts': 'video/mp2t',
		'.mp4': 'video/mp4',
		'.vtt': 'text/vtt'
	};
	const server = http.createServer((req, res) => {
		const name = path.basename(new URL(req.url ?? '/', 'http://x').pathname);
		const file = path.join(mediaDir, name);
		if (!existsSync(file)) {
			res.writeHead(404).end();
			return;
		}
		const body = fs.readFileSync(file);
		res.writeHead(200, {
			'content-type': types[path.extname(name)] ?? 'application/octet-stream',
			'content-length': body.length,
			'accept-ranges': 'bytes'
		});
		res.end(body);
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

/** The session a play opened, read off the player's URL. */
function sessionOf(page: Page): string {
	const sid = new URL(page.url()).searchParams.get('session');
	expect(sid, 'the player names its session').toBeTruthy();
	return sid!;
}

/** The player's state, read off its <video>. */
function videoState(page: Page) {
	return page.evaluate(() => {
		const v = document.querySelector('video');
		if (!v) return null;
		return {
			src: v.currentSrc || v.src,
			time: v.currentTime,
			error: v.error?.code ?? null,
			cues: [...v.textTracks].flatMap((t) => [...(t.cues ?? [])].map((c) => (c as VTTCue).text)),
			tracks: [...v.querySelectorAll('track')].map((t) => t.getAttribute('src'))
		};
	});
}

/** The rendition requests a play made, as the stream lane answered them. */
function renditionResponses(page: Page): { url: string; status: number }[] {
	const seen: { url: string; status: number }[] = [];
	page.on('response', (r) => {
		if (/[?&]s=subs|[?&]r=subs/.test(r.url())) seen.push({ url: r.url(), status: r.status() });
	});
	return seen;
}

// The two kinds of subtitles are played apart: what the policy has to
// allow is each one's path, and each is proved on a stream of its own.
// How the two behave together on one stream is
// captions-playlist-sidecar.spec.ts's.

test('a play through the stream proxy plays, with its sidecar subtitles, under the policy', async () => {
	test.setTimeout(90_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		const { lanes } = await bridge(page);
		// The Continue card's play, through the backend's play route: it
		// opens a session on the fixture's media playlist, with a sidecar
		// track the backend fetches and serves from the stream lane.
		const stream: ReplayStream = {
			upstreamUrl: `${media.origin}/video.m3u8`,
			mediaKind: 'hls',
			subtitles: [{ lang: 'en', label: 'Sidecar', default: false, url: `${media.origin}/sidecar.vtt` }]
		};
		const play = await routeRealPlay(page, { profile, settings: {}, stream });
		await playFromContinue(page, play);
		const sid = sessionOf(page);
		// hls.js feeds the element through a MediaSource blob (media-src
		// blob:), its playlist and segments come from the stream lane
		// (connect-src), and playback advances.
		await expect
			.poll(async () => (await videoState(page))?.time ?? 0, { timeout: 30_000 })
			.toBeGreaterThan(0.5);
		const hls = await videoState(page);
		expect(hls?.src.startsWith('blob:')).toBe(true);
		expect(hls?.error).toBeNull();

		// The sidecar <track> loads from the stream lane (media-src). It
		// is the session's start for the English UI, so it shows, and its
		// cue is there; the track element's readyState (2 loaded,
		// 3 failed) shows the load itself.
		await expect
			.poll(async () => (await videoState(page))?.tracks ?? [], { timeout: 10_000 })
			.toEqual([`${lanes.stream}/s/${sid}/sub/0.vtt`]);
		await expect
			.poll(
				() =>
					page.evaluate(() => {
						const v = document.querySelector('video')!;
						return {
							cues: [...v.textTracks].flatMap((t) => [...(t.cues ?? [])].map((c) => (c as VTTCue).text)),
							trackStates: [...v.querySelectorAll('track')].map((t) => t.readyState)
						};
					}),
				{ timeout: 15_000 }
			)
			.toEqual({ cues: ['A cue from the sidecar track'], trackStates: [2] });
	} finally {
		await app.close();
		media.close();
	}
});

test('a play through the stream proxy plays, with its playlist subtitles, under the policy', async () => {
	test.setTimeout(90_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		const { lanes } = await bridge(page);
		const renditions = renditionResponses(page);
		// A master playlist with a default subtitle rendition and no
		// sidecar track: nothing but hls.js touches the text tracks.
		const stream: ReplayStream = { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: 'hls' };
		const play = await routeRealPlay(page, { profile, settings: {}, stream });
		await playFromContinue(page, play);
		const sid = sessionOf(page);
		await expect
			.poll(async () => (await videoState(page))?.time ?? 0, { timeout: 30_000 })
			.toBeGreaterThan(0.5);
		// The rendition's cue reaches the player, in the track hls.js
		// made for it...
		await expect
			.poll(
				() =>
					page.evaluate(() =>
						[...document.querySelector('video')!.textTracks].map((t) => ({
							label: t.label,
							cues: [...(t.cues ?? [])].map((c) => (c as VTTCue).text)
						}))
					),
				{ timeout: 15_000 }
			)
			.toEqual([{ label: 'English', cues: ['A cue from the playlist'] }]);
		// ...and its playlist and fragment came from the stream lane
		// (connect-src), answered.
		const lane = `${lanes.stream}/s/${sid}/`;
		expect(renditions.map((r) => r.url.startsWith(lane) && r.status === 200)).toEqual([true, true]);
	} finally {
		await app.close();
		media.close();
	}
});

test('an MP4 play goes straight from the stream lane, under the policy', async () => {
	test.setTimeout(90_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		const { lanes } = await bridge(page);
		const play = await routeRealPlay(page, {
			profile,
			settings: {},
			stream: { upstreamUrl: `${media.origin}/clip.mp4`, mediaKind: 'mp4' }
		});
		await playFromContinue(page, play);
		const sid = sessionOf(page);
		// The element loads the file from the stream lane itself
		// (media-src), with no MediaSource in between.
		await expect
			.poll(async () => (await videoState(page))?.time ?? 0, { timeout: 30_000 })
			.toBeGreaterThan(0.5);
		const file = await videoState(page);
		expect(file?.src).toBe(`${lanes.stream}/s/${sid}/file.mp4`);
		expect(file?.error).toBeNull();
	} finally {
		await app.close();
		media.close();
	}
});

test('the loader animates under the policy', async () => {
	test.setTimeout(90_000);
	const { app, page } = await launch();
	try {
		let homeAsked: (() => void) | null = null;
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p === '/api/app-info') return j(appInfo);
			if (p === '/api/settings') return j(defaultSettings);
			if (p === '/api/kitsu/trending-anilist') return j(trending);
			if (p === '/api/kitsu/top-rated') return j(topRated);
			if (p.startsWith('/api/image')) return answer(r, { status: 503 });
			if (p === '/api/history') {
				homeAsked?.();
				return j(continueHistory);
			}
			if (p === '/api/watched-at') return j({ [continueHistory[0].id]: 1_700_000_000 });
			if (p.startsWith('/api/allmanga-kitsu-map/')) return j(continueKitsuMatch.id);
			if (p.startsWith('/api/kitsu/anime/')) return j(continueKitsuMatch);
			if (p.startsWith('/api/kitsu/episodes/')) return j([continueKitsuEpisode6]);
			if (p === '/api/availability')
				return j({ available: true, episode_count: 12, extra_episodes: [] });
			// The play never resolves: the overlay stays up.
			if (p === '/api/play/stream' || p === '/api/play') return;
			return j(null);
		});
		// A fresh mount, so every request of the home page goes through
		// the routes above.
		await remountFirstPage(page, () => new Promise<void>((resolve) => (homeAsked = resolve)));

		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.locator('button.resume-card').first();
		await expect(card).toBeVisible({ timeout: 20_000 });
		await card.click();

		const svg = page.locator('.backdrop .anim svg');
		await expect(svg).toBeVisible({ timeout: 10_000 });
		// lottie's light build drew the animation, and it moves.
		const frame = () => svg.evaluate((el) => el.innerHTML);
		await expect.poll(async () => (await frame()).length, { timeout: 5_000 }).toBeGreaterThan(1000);
		const first = await frame();
		// Under a loaded Xvfb, Chromium can stop producing frames for a
		// window nobody looks at, and lottie advances only on animation
		// frames. Invalidating the window asks for one before each read.
		await expect
			.poll(
				async () => {
					await app.evaluate(({ BrowserWindow }) => {
						for (const w of BrowserWindow.getAllWindows()) w.webContents.invalidate();
					});
					return frame();
				},
				{ timeout: 40_000, intervals: [100, 250] }
			)
			.not.toBe(first);
	} finally {
		await app.close();
	}
});
