/**
 * Sidecar subtitles through a handoff that loads the playlist again,
 * in the packaged app's own Chromium with the real hls.js.
 *
 * An episode handed to the external player before its playlist is in
 * has the playlist cut off: hls.js drops its loaders at the handoff's
 * stopLoad(), so the viewer coming back has the page load it again
 * ($lib/play/handoff-gate). hls.js clears the cues of every text track
 * on the video as a playlist starts loading, the `<track>` elements'
 * included, and the browser loads a `<track>`'s file once: with its
 * src unchanged it never asks again. Before the fix, a sidecar track
 * cleared that way showed nothing for the rest of the play.
 *
 * The stream is the two-level fixture of captions-start.spec.ts, its
 * session listing two sidecar tracks, English (the start) and
 * Français. The server holds the master playlist back until the spec
 * lets it go, so the episode can be handed off before it is in; the
 * sidecar listing and files come through meanwhile. The backend's
 * check that the stream answers, a HEAD, is answered at once.
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

const mediaDir = path.join(__dirname, 'fixtures', 'captions');

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

async function launch(): Promise<{ app: ElectronApplication; page: Page; profile: string }> {
	const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-captions-reload-'));
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

/** A server for the stream, standing in for a provider's CDN. Every
 *  GET of the master playlist waits for `release`. */
async function mediaServer(): Promise<{
	origin: string;
	/** The GETs of the master playlist asked so far. */
	masters: () => number;
	release: () => void;
	close: () => void;
}> {
	let release: () => void = () => {};
	const released = new Promise<void>((resolve) => (release = resolve));
	let masters = 0;
	const sidecarCues = fs.readFileSync(path.join(mediaDir, 'sidecar.vtt'), 'utf8');
	const types: Record<string, string> = {
		'.m3u8': 'application/vnd.apple.mpegurl',
		'.ts': 'video/mp2t',
		'.vtt': 'text/vtt'
	};
	const server = http.createServer((req, res) => {
		const at = new URL(req.url ?? '/', 'http://x').pathname;
		const send = (type: string, body: Buffer) => {
			res.writeHead(200, { 'content-type': type, 'content-length': body.length });
			res.end(body);
		};
		const sidecar = /^\/sub\/(\d+)\.vtt$/.exec(at);
		const name = path.basename(at);
		const file = path.join(mediaDir, name);
		if (!sidecar && !existsSync(file)) return void res.writeHead(404).end();
		const type = types[path.extname(name)] ?? 'application/octet-stream';
		if (req.method === 'HEAD') return void res.writeHead(200, { 'content-type': type }).end();
		if (sidecar) {
			const label = LISTING[Number(sidecar[1])]?.label;
			if (!label) return void res.writeHead(404).end();
			return send(type, Buffer.from(sidecarCues.replaceAll('Cue at', `${label} at`)));
		}
		if (name === 'master.m3u8') {
			masters += 1;
			return void released.then(() => send(type, fs.readFileSync(file)));
		}
		send(type, fs.readFileSync(file));
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return {
		origin: `http://127.0.0.1:${port}`,
		masters: () => masters,
		release,
		close: () => server.close()
	};
}

function stream(origin: string): ReplayStream {
	return {
		upstreamUrl: `${origin}/master.m3u8`,
		mediaKind: 'hls',
		subtitles: LISTING.map((t, i) => ({ ...t, url: `${origin}/sub/${i}.vtt` }))
	};
}

const settings = { use_custom_player_controls: true };

/** The sidecar tracks: each one's label, mode, whether its file is in,
 *  and how many cues it holds (null while it is disabled); the cue
 *  text active on the tracks showing; and the playback position. */
function sidecars(page: Page) {
	return page.evaluate(() => {
		const v = document.querySelector('video');
		if (!v) return null;
		const els = [...v.querySelectorAll('track')];
		return {
			tracks: els.map((el) => ({
				label: el.label,
				mode: el.track.mode,
				loaded: el.readyState === HTMLTrackElement.LOADED,
				cues: el.track.cues?.length ?? null
			})),
			cues: els
				.filter((el) => el.track.mode === 'showing')
				.flatMap((el) => [...(el.track.activeCues ?? [])].map((c) => (c as VTTCue).text)),
			time: v.currentTime
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

/** Pick `name` from the app's captions menu, driven by its buttons'
 *  own clicks (the controls hide while the pointer is away). */
async function pick(page: Page, name: string) {
	await page.locator('button.pc-cc[aria-label="Captions"]').dispatchEvent('click');
	await page
		.locator('#pc-cc-menu [role="menuitemradio"]', { hasText: name })
		.dispatchEvent('click');
}

/** Hands the episode to the external player, which the stub answers
 *  will not start, and closes the app's notice of that; then plays the
 *  embedded video again, which asks for the playlist again. */
async function handOffAndComeBack(page: Page, media: { masters: () => number }) {
	const before = media.masters();
	await page.locator('button[aria-label="More actions"]').click();
	await page.getByRole('menuitem', { name: /Open in external/ }).click();
	const dialog = page.getByRole('alertdialog');
	await dialog.waitFor({ timeout: 15_000 });
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
	await page.locator('.player-frame').hover();
	await page.locator('.pc-buttons-row .pc-btn').first().click();
	await expect.poll(() => media.masters(), { timeout: 10_000 }).toBe(before + 1);
}

/** Plays the card's episode, its master playlist held back, until the
 *  start's sidecar file is in. */
async function heldAtStart(page: Page, profile: string, media: { origin: string }) {
	const play = await routeRealPlay(page, { profile, settings, stream: stream(media.origin) });
	await playFromContinue(page, play);
	await expect
		.poll(async () => (await sidecars(page))?.tracks ?? [], { timeout: 30_000 })
		.toEqual([
			{ label: 'English', mode: 'showing', loaded: true, cues: 12 },
			{ label: 'Français', mode: 'disabled', loaded: false, cues: null }
		]);
}

test('the sidecar track showing when the episode is handed off before its playlist is in has its cues when the viewer comes back', async () => {
	test.setTimeout(120_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		await heldAtStart(page, profile, media);
		await handOffAndComeBack(page, media);
		media.release();
		await expect
			.poll(async () => (await sidecars(page))?.time ?? 0, { timeout: 30_000 })
			.toBeGreaterThan(1);
		expect((await sidecars(page))?.tracks[0]).toEqual({
			label: 'English',
			mode: 'showing',
			loaded: true,
			cues: 12
		});
		await expect
			.poll(async () => (await sidecars(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^English at /)]);
		await expect
			.poll(() => painted(page), { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^English at /)]);
	} finally {
		media.release();
		await app.close();
		media.close();
	}
});

test('sidecar tracks turned off before two handoffs in a row show their cues when picked after', async () => {
	test.setTimeout(150_000);
	const media = await mediaServer();
	const { app, page, profile } = await launch();
	try {
		await heldAtStart(page, profile, media);
		// French is picked, so its file comes in, then captions go off:
		// both tracks loaded, both off.
		await pick(page, 'French');
		await expect
			.poll(async () => (await sidecars(page))?.tracks[1] ?? null, { timeout: 10_000 })
			.toEqual({ label: 'Français', mode: 'showing', loaded: true, cues: 12 });
		await pick(page, 'Off');
		await expect
			.poll(async () => (await sidecars(page))?.tracks.map((t) => t.mode) ?? [])
			.toEqual(['disabled', 'disabled']);

		// Handed off and back twice, the playlist held back both times.
		await handOffAndComeBack(page, media);
		await handOffAndComeBack(page, media);
		media.release();
		await expect
			.poll(async () => (await sidecars(page))?.time ?? 0, { timeout: 30_000 })
			.toBeGreaterThan(1);
		// Off stays off.
		expect((await sidecars(page))?.tracks.map((t) => t.mode)).toEqual(['disabled', 'disabled']);

		await pick(page, 'English');
		await expect
			.poll(async () => (await sidecars(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^English at /)]);
		await expect
			.poll(() => painted(page), { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^English at /)]);

		await pick(page, 'French');
		await expect
			.poll(async () => (await sidecars(page))?.cues ?? [], { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Français at /)]);
		await expect
			.poll(() => painted(page), { timeout: 10_000 })
			.toEqual([expect.stringMatching(/^Français at /)]);
	} finally {
		media.release();
		await app.close();
		media.close();
	}
});
