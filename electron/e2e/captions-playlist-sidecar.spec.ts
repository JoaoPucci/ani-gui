/**
 * Captions on a stream whose playlist carries its own subtitle
 * renditions while its session also lists sidecar tracks, through the
 * real hls.js in the packaged app's own Chromium.
 *
 * hls.js reads and writes the video's text tracks itself: each time its
 * subtitle track is set it disables every labelled track but its own
 * pick, sidecar tracks included, and a moment after any change it takes
 * what shows for a pick of its own. It also drops the cues of a
 * rendition's fragment it parses while that rendition's track is off,
 * and never fetches the fragment again. The unit and acceptance tiers
 * run a model of that behaviour; this is where the real one runs.
 *
 * The stream is the two-level fixture of captions-start.spec.ts, with a
 * master playlist that adds two subtitle renditions: English, the
 * playlist's default, and Español, each a fragment per segment, served
 * here with their cues named "Playlist <name> at <second>". The session
 * lists two sidecar tracks: English, which the UI's language starts on
 * and which hls.js would take for its rendition of the same name and
 * language, and Français. A spec can hold the master playlist back, so
 * the sidecar listing arrives first, and hold an English fragment
 * back, so it is on its way while the viewer moves off it.
 */
import type { ElectronApplication, Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import type net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { appWindow, launchPackagedApp, packagedBinary } from './launch';
import { playFromContinue, routeRealPlay, writeReplayConfig, type ReplayStream } from './real-play';

const videoDir = path.join(__dirname, 'fixtures', 'captions');
const playlistDir = path.join(__dirname, 'fixtures', 'captions-renditions');

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

async function launch(): Promise<{ app: ElectronApplication; page: Page; profile: string }> {
	const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-captions-renditions-'));
	writeReplayConfig(tmp);
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
	const page = await appWindow(app);
	return { app, page, profile: tmp };
}

const LISTING = [
	{ lang: 'en', label: 'English', default: true },
	{ lang: 'fr', label: 'Français', default: false }
];
const RENDITION_NAMES: Record<string, string> = { en: 'English', es: 'Español' };

/** One rendition fragment: the 4 s from `4 * n`, a cue every 2 s. The
 *  segments' first PTS is 126000 (1.4 s at 90 kHz). */
function renditionFragment(lang: string, n: number): string {
	const at = (s: number) => `00:00:${String(s).padStart(2, '0')}.000`;
	const cues = [4 * n, 4 * n + 2]
		.map((s) => `${at(s)} --> ${at(s + 2)}\nPlaylist ${RENDITION_NAMES[lang]} at ${s}\n`)
		.join('\n');
	return `WEBVTT\nX-TIMESTAMP-MAP=MPEGTS:126000,LOCAL:00:00:00.000\n\n${cues}`;
}

interface Holds {
	/** Milliseconds the master playlist is held back. */
	master?: number;
	/** The English rendition's second fragment (4–8 s) held back until
	 *  this resolves. hls.js loads the fragment before the one it wants
	 *  first where that one is not loaded yet, so holding every English
	 *  fragment would hold back the first and the second would never be
	 *  asked for. */
	englishFragment?: Promise<void>;
}

/** A server for the stream, standing in for a provider's CDN. */
async function mediaServer(
	holds: Holds = {}
): Promise<{ origin: string; asked: string[]; close: () => void }> {
	const asked: string[] = [];
	const types: Record<string, string> = {
		'.m3u8': 'application/vnd.apple.mpegurl',
		'.ts': 'video/mp2t',
		'.vtt': 'text/vtt'
	};
	const sidecarCues = fs.readFileSync(path.join(videoDir, 'sidecar.vtt'), 'utf8');
	const send = (res: http.ServerResponse, type: string, body: Buffer, after = 0) => {
		setTimeout(() => {
			res.writeHead(200, { 'content-type': type, 'content-length': body.length });
			res.end(body);
		}, after);
	};
	const server = http.createServer((req, res) => {
		const at = new URL(req.url ?? '/', 'http://x').pathname;
		asked.push(at);
		const sidecar = /^\/sub\/(\d+)\.vtt$/.exec(at);
		if (sidecar && LISTING[Number(sidecar[1])]) {
			const label = LISTING[Number(sidecar[1])].label;
			return send(res, types['.vtt'], Buffer.from(sidecarCues.replaceAll('Cue at', `${label} at`)));
		}
		const fragment = /^\/(en|es)_(\d)\.vtt$/.exec(at);
		if (fragment) {
			const [, lang, n] = fragment;
			const body = Buffer.from(renditionFragment(lang, Number(n)));
			const held = lang === 'en' && n === '1' ? holds.englishFragment : undefined;
			if (held) return void held.then(() => send(res, types['.vtt'], body));
			return send(res, types['.vtt'], body);
		}
		const name = path.basename(at);
		const file = [playlistDir, videoDir]
			.map((dir) => path.join(dir, name))
			.find((f) => existsSync(f));
		if (!file) {
			res.writeHead(404).end();
			return;
		}
		const hold = name === 'master.m3u8' ? (holds.master ?? 0) : 0;
		send(res, types[path.extname(name)] ?? 'application/octet-stream', fs.readFileSync(file), hold);
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, asked, close: () => server.close() };
}

function stream(origin: string): ReplayStream {
	return {
		upstreamUrl: `${origin}/master.m3u8`,
		mediaKind: 'hls',
		subtitles: LISTING.map((t, i) => ({ ...t, url: `${origin}/sub/${i}.vtt` }))
	};
}

const settings = { use_custom_player_controls: true };

/** What the player shows: how many subtitle tracks there are, each one
 *  showing as `sidecar <label>` or `playlist <label>`, the cue text
 *  active on them, the playback position and frame height. The mark
 *  that keeps a sidecar track's label apart from a rendition's is
 *  left out, as the captions menu leaves it out. */
function shown(page: Page) {
	return page.evaluate(() => {
		const v = document.querySelector('video');
		if (!v) return null;
		const own = new Set([...v.querySelectorAll('track')].map((el) => el.track));
		const tracks = [...v.textTracks].filter((t) => t.kind === 'subtitles');
		const on = tracks.filter((t) => t.mode === 'showing');
		return {
			tracks: tracks.length,
			showing: on.map(
				(t) => `${own.has(t) ? 'sidecar' : 'playlist'} ${t.label.replaceAll('\u2060', '')}`
			),
			cues: on.flatMap((t) => [...(t.activeCues ?? [])].map((c) => (c as VTTCue).text)),
			time: v.currentTime,
			height: v.videoHeight
		};
	});
}

/** The text of the cue boxes the video draws, in its user-agent shadow
 *  tree (as captions-start.spec.ts reads them). */
async function painted(page: Page): Promise<string[]> {
	const cdp = await page.context().newCDPSession(page);
	try {
		const { root } = await cdp.send('DOM.getDocument', { depth: -1, pierce: true });
		interface DomNode {
			nodeType: number;
			nodeValue: string;
			attributes?: string[];
			children?: DomNode[];
			shadowRoots?: DomNode[];
		}
		const pseudo = (n: DomNode) => {
			const a = n.attributes ?? [];
			const at = a.indexOf('pseudo');
			return at === -1 ? '' : a[at + 1];
		};
		const text = (n: DomNode): string =>
			n.nodeType === 3
				? n.nodeValue
				: [...(n.children ?? []), ...(n.shadowRoots ?? [])].map(text).join('');
		const cues: string[] = [];
		const walk = (n: DomNode) => {
			if (pseudo(n) === 'cue') {
				cues.push(text(n));
				return;
			}
			for (const c of [...(n.children ?? []), ...(n.shadowRoots ?? [])]) walk(c);
		};
		walk(root as DomNode);
		return cues;
	} finally {
		await cdp.detach();
	}
}

/** Over `ms`, read what shows every 250 ms; every read must show
 *  exactly `which`. */
async function showsThroughout(page: Page, which: string[], ms: number) {
	const deadline = Date.now() + ms;
	while (Date.now() < deadline) {
		expect((await shown(page))?.showing).toEqual(which);
		await page.waitForTimeout(250);
	}
}

/** The captions menu's name for the playlist's English: the two
 *  English tracks read the same, so the menu numbers them in the
 *  order the video lists them. */
function playlistEnglish(page: Page): Promise<string> {
	return page.evaluate(() => {
		const v = document.querySelector('video');
		if (!v) throw new Error('no video');
		const own = new Set([...v.querySelectorAll('track')].map((el) => el.track));
		const english = [...v.textTracks].filter((t) => t.language === 'en');
		return `English ${english.findIndex((t) => !own.has(t)) + 1}`;
	});
}

/** Pick `name` from the app's captions menu, driven by its buttons'
 *  own clicks (the controls hide while the pointer is away). */
async function pick(page: Page, name: string) {
	await page.locator('button.pc-cc[aria-label="Captions"]').dispatchEvent('click');
	const items = page.locator('#pc-cc-menu [role="menuitemradio"]');
	const item =
		name === 'Off'
			? items.filter({ hasText: 'Off' })
			: items.filter({ has: page.locator('.pc-cc-name', { hasText: new RegExp(`^${name}$`) }) });
	await item.dispatchEvent('click');
}

async function seek(page: Page, to: number) {
	await page.evaluate((t) => {
		const v = document.querySelector('video');
		if (v) v.currentTime = t;
	}, to);
}

/** Play the fixture, the four subtitle tracks attached and playback
 *  running past its first second. */
async function playing(page: Page, profile: string, origin: string) {
	const play = await routeRealPlay(page, { profile, settings, stream: stream(origin) });
	await playFromContinue(page, play);
	await expect.poll(async () => (await shown(page))?.tracks ?? 0, { timeout: 30_000 }).toBe(4);
	await expect.poll(async () => (await shown(page))?.time ?? 0, { timeout: 30_000 }).toBeGreaterThan(1);
}

for (const [order, holds] of [
	['as hls.js loads its playlist', {}],
	['with the sidecar listing in before the playlist', { master: 2_000 }]
] as const) {
	test(`the sidecar track a session starts on shows alone, ${order}`, async () => {
		test.setTimeout(120_000);
		const media = await mediaServer(holds);
		const { app, page, profile } = await launch();
		try {
			await playing(page, profile, media.origin);
			await expect
				.poll(async () => (await shown(page))?.cues ?? [], { timeout: 10_000 })
				.toEqual([expect.stringMatching(/^English at /)]);
			await expect
				.poll(() => painted(page), { timeout: 10_000 })
				.toEqual([expect.stringMatching(/^English at /)]);
			await showsThroughout(page, ['sidecar English'], 3_000);
		} finally {
			await app.close();
			media.close();
		}
	});
}

// Both levels list the one subtitle group, so the quality switch here
// leaves hls.js's subtitle selection as it is; a switch between groups
// is covered by the unit tier (frontend/tests/caption-sync.test.ts).
test("each kind picked from the captions menu shows alone with its cues, through a seek and a quality switch within one subtitle group", async () => {
	test.setTimeout(150_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		await playing(page, profile, media.origin);
		await expect
			.poll(async () => (await shown(page))?.showing ?? [], { timeout: 10_000 })
			.toEqual(['sidecar English']);

		// A rendition, from the sidecar track.
		await pick(page, 'Spanish');
		await expect
			.poll(async () => (await shown(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Playlist Español at /)]);
		await expect
			.poll(() => painted(page), { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Playlist Español at /)]);
		await showsThroughout(page, ['playlist Español'], 2_000);

		// A seek keeps it, with the cues where it lands.
		await seek(page, 14);
		await expect
			.poll(async () => (await shown(page))?.cues ?? [], { timeout: 15_000 })
			.toEqual([expect.stringMatching(/^Playlist Español at 1[46]$/)]);
		await showsThroughout(page, ['playlist Español'], 2_000);

		// A quality switch keeps it.
		const before = (await shown(page))?.height ?? 0;
		await seek(page, 2);
		await page.locator('button.pc-btn[aria-haspopup="menu"][aria-label="More"]').dispatchEvent('click');
		await page.locator('.pc-menu-row-arrow', { hasText: 'Quality' }).dispatchEvent('click');
		await page
			.locator('.pc-menu-quality-row', { hasText: before === 72 ? '36p' : '72p' })
			.dispatchEvent('click');
		await expect
			.poll(async () => (await shown(page))?.height ?? 0, { timeout: 20_000 })
			.toBe(before === 72 ? 36 : 72);
		await showsThroughout(page, ['playlist Español'], 2_000);

		// A sidecar track, from the rendition.
		await pick(page, 'French');
		await expect
			.poll(async () => (await shown(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Français at /)]);
		await showsThroughout(page, ['sidecar Français'], 2_000);

		// Off.
		await pick(page, 'Off');
		await showsThroughout(page, [], 2_000);

		// The other English: the playlist's, from Off.
		await pick(page, await playlistEnglish(page));
		await expect
			.poll(async () => (await shown(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Playlist English at /)]);
		await showsThroughout(page, ['playlist English'], 2_000);
	} finally {
		await app.close();
		media.close();
	}
});

// The viewer moves off a rendition while its fragment is on its way:
// hls.js parsed it with the rendition's track off, dropped its cues and
// never fetched it again, so the rendition, picked back, showed
// nothing there.
test("a rendition's fragment that lands after the viewer moved off it is there when it is picked again", async () => {
	test.setTimeout(120_000);
	let release: () => void = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const media = await mediaServer({ englishFragment: held });
	const { app, page, profile } = await launch();
	try {
		await playing(page, profile, media.origin);
		await page.evaluate(() => document.querySelector('video')?.pause());
		// Inside the second fragment (4–8 s), clear of its cue boundaries.
		await seek(page, 7);
		await expect.poll(async () => (await shown(page))?.time ?? 0).toBeCloseTo(7, 0);
		const english = await playlistEnglish(page);
		await pick(page, english);
		// Off it while the fragment at 7 s is on its way.
		await expect.poll(() => media.asked.includes('/en_1.vtt'), { timeout: 10_000 }).toBe(true);
		await pick(page, 'French');
		await expect
			.poll(async () => (await shown(page))?.showing ?? [], { timeout: 10_000 })
			.toEqual(['sidecar Français']);
		// The fragment held back lands now, with the viewer off it.
		release();
		await page.waitForTimeout(2_000);
		await pick(page, english);
		await expect
			.poll(async () => (await shown(page))?.cues ?? [], { timeout: 5_000 })
			.toEqual(['Playlist English at 6']);
		await expect.poll(() => painted(page), { timeout: 5_000 }).toEqual(['Playlist English at 6']);
	} finally {
		release();
		await app.close();
		media.close();
	}
});
