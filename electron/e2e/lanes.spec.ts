/**
 * Held requests on one lane delay nothing on another — in the packaged
 * app's own Chromium, against its own backend.
 *
 * Chromium opens at most six connections at a time to one host and
 * port over plain HTTP and queues every request past the sixth. When
 * the API, the image route and the stream proxy shared one origin,
 * availability probes against a slow provider filled the six and the
 * player's playlist and segment fetches queued behind them until hls.js
 * gave up on them. The backend now listens on one origin per lane and
 * the renderer sends each kind of request to its own (the frontend's
 * unit and acceptance tests pin which request goes where); this proves
 * the half only a real browser can: that requests held on the probe,
 * resolve, download and image lanes leave the API and the stream proxy
 * answering at once.
 *
 * What holds them is the image route asked for an upstream that never
 * answers: a TCP server that accepts and says nothing, so the
 * backend's fetch waits on the TLS handshake until its connect timeout
 * (ten seconds, the metadata client's; the handshake counts as part
 * of connecting).
 * Every lane serves every route, so the same held request stands in
 * for a slow probe on the probe lane, a slow resolution on the resolve
 * lane, an open download stream on the download lane and a cold poster
 * on the image lane — the connection is what is being held, whatever
 * the route. A control request on a held lane must itself wait, or the
 * holds did not take and the rest of the test would prove nothing.
 *
 * Nothing here is timed. "At once" is read as an order: the API call,
 * the session and the play's first proxy request each answer while
 * every held request is still held, so none of them can have waited
 * for one of those connections to come free; and the control answers
 * only after a held request on its lane has let its connection go.
 * The holds end on the backend's connect timeout, so an answer slower
 * than that would fail the order — it can fail on a machine that slow,
 * never pass because of it.
 *
 * Pre-condition: `pnpm run package` has produced `dist/linux-unpacked/`.
 */
import { _electron as electron, expect, test, type ElectronApplication } from '@playwright/test';
import { killTree, withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const electronDir = path.resolve(__dirname, '..');
const packagedBinary = path.join(electronDir, 'dist/linux-unpacked/ani-gui');

/** How many requests each held lane is given — more than Chromium's
 *  six, so every one of its connections is taken. */
const HELD_PER_LANE = 7;
/** The lanes the test holds. */
const HELD_LANES = ['probe', 'resolve', 'download', 'image'];
/** How many connections Chromium opens to one host and port. */
const CHROMIUM_PER_ORIGIN = 6;

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

/** A TCP server that accepts every connection and never says a word,
 *  counting the connections it has accepted. */
async function silentServer(): Promise<{
	port: number;
	accepted: () => number;
	close: () => void;
}> {
	const sockets = new Set<net.Socket>();
	let accepted = 0;
	const server = net.createServer((s) => {
		accepted += 1;
		sockets.add(s);
		s.on('error', () => {});
		s.on('close', () => sockets.delete(s));
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return {
		port,
		accepted: () => accepted,
		close: () => {
			for (const s of sockets) s.destroy();
			server.close();
		}
	};
}

/** An upstream serving a one-line master playlist, for a play's
 *  first proxy request. */
async function upstream(): Promise<{ port: number; close: () => void }> {
	const server = http.createServer((_req, res) => {
		res.writeHead(200, { 'content-type': 'application/vnd.apple.mpegurl' });
		res.end('#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nlow/index.m3u8\n');
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { port, close: () => server.close() };
}

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-e2e-lanes-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await electron.launch({
		executablePath: packagedBinary,
		args: ['--no-sandbox'],
		env: {
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data'),
			ANI_GUI_UNATTENDED: '1'
		}
	});
	onLaunch(app);
	const page = await app.firstWindow();
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

test('requests held on the probe, resolve, download and image lanes delay neither the API nor a play', async () => {
	test.setTimeout(90_000);
	const silent = await silentServer();
	const media = await upstream();
	const { app, page } = await launch();
	try {
		await page.waitForFunction(() => Boolean(window.aniGui?.apiBase));
		const lanes = await page.evaluate(
			({ silentPort, heldLanes, heldPerLane }) => {
				const bridge = window.aniGui!;
				const lanes = { ...(bridge.lanes ?? {}) } as Record<string, string>;
				const stop = new AbortController();
				// How many held requests on each lane have let their
				// connection go, read when each request below answers.
				const released: Record<string, number> = {};
				const w = window as unknown as {
					__stopHeld: AbortController;
					__released: Record<string, number>;
				};
				w.__stopHeld = stop;
				w.__released = released;
				for (const lane of heldLanes) {
					released[lane] = 0;
					for (let i = 0; i < heldPerLane; i++) {
						const hang = `https://127.0.0.1:${silentPort}/${lane}-${i}.jpg`;
						void fetch(`${lanes[lane]}/api/image?url=${encodeURIComponent(hang)}`, {
							signal: stop.signal
						})
							.catch(() => {})
							.finally(() => {
								released[lane] += 1;
							});
					}
				}
				return lanes;
			},
			{
				silentPort: silent.port,
				heldLanes: HELD_LANES,
				heldPerLane: HELD_PER_LANE
			}
		);
		// Every connection Chromium has for each held lane carries a
		// held request: each has reached the backend, which has opened
		// its connection upstream.
		const holding = HELD_LANES.length * CHROMIUM_PER_ORIGIN;
		await expect
			.poll(silent.accepted, {
				message: 'every held lane has its connections taken'
			})
			.toBeGreaterThanOrEqual(holding);
		// Taken by the first requests of each lane, not by later ones
		// that a released hold let through.
		const releasedBeforeStart = await page.evaluate(() =>
			Object.values(
				(window as unknown as { __released: Record<string, number> }).__released
			).reduce((a, b) => a + b, 0)
		);
		expect(releasedBeforeStart, 'no hold has let go yet').toBe(0);

		const result = await page.evaluate(
			async ({ mediaPort }) => {
				const bridge = window.aniGui!;
				const api = bridge.apiBase!;
				const lanes = { ...(bridge.lanes ?? {}) } as Record<string, string>;
				const w = window as unknown as {
					__stopHeld: AbortController;
					__released: Record<string, number>;
				};
				const stop = w.__stopHeld;
				const releasedNow = () => Object.values(w.__released).reduce((a, b) => a + b, 0);
				// A request, how it settled, and how many held requests
				// had let their connection go when it did.
				const ordered = async (url: string, init?: RequestInit) => {
					const settled = await fetch(url, {
						...init,
						signal: stop.signal
					}).then(
						async (r) => ({ ok: r.ok, body: await r.text() }),
						() => ({ ok: false, body: '' })
					);
					return {
						settled,
						released: releasedNow(),
						probeReleased: w.__released.probe
					};
				};

				// The control: a held lane's seventh-and-later request
				// waits for one of the held to let its connection go.
				const control = ordered(`${lanes.probe}/healthz`);
				const apiCall = await ordered(`${api}/api/settings`);
				const session = await ordered(`${api}/api/sessions`, {
					method: 'POST',
					headers: { 'content-type': 'application/json' },
					body: JSON.stringify({
						upstream_url: `http://127.0.0.1:${mediaPort}/master.m3u8`,
						referer: ''
					})
				});
				const mediaUrl = session.settled.ok
					? (JSON.parse(session.settled.body) as { media_url: string }).media_url
					: '';
				const firstProxyRequest = mediaUrl ? await ordered(mediaUrl) : null;
				// The control answers once the holds start to end on the
				// backend's connect timeout; nothing aborts it before.
				const controlled = await control;
				stop.abort();
				return {
					api,
					control: controlled,
					apiCall,
					session,
					mediaUrl,
					firstProxyRequest
				};
			},
			{ mediaPort: media.port }
		);

		const origins = new Set([result.api, ...Object.values(lanes)]);
		expect(Object.keys(lanes).sort()).toEqual(
			['download', 'image', 'probe', 'resolve', 'stream'].sort()
		);
		expect(origins.size, 'every lane is an origin of its own').toBe(6);

		expect(result.control.settled.ok, 'the control answered').toBe(true);
		expect(
			result.control.probeReleased,
			'the held lane really is full: the control waited for a held request to let go'
		).toBeGreaterThan(0);
		expect(result.apiCall.settled.ok, 'the API answered').toBe(true);
		expect(result.apiCall.released, 'the API answered while every hold stood').toBe(0);
		expect(result.session.settled.ok, 'the session was made').toBe(true);
		expect(result.session.released, 'the session was made while every hold stood').toBe(0);
		expect(result.mediaUrl.startsWith(`${lanes.stream}/s/`)).toBe(true);
		expect(result.firstProxyRequest?.settled.ok, "the play's first proxy request answered").toBe(
			true
		);
		expect(
			result.firstProxyRequest?.released,
			"the play's first proxy request answered while every hold stood"
		).toBe(0);
		expect(result.firstProxyRequest?.settled.body).toContain('#EXTM3U');
	} finally {
		await page.evaluate(() =>
			(window as unknown as { __stopHeld?: AbortController }).__stopHeld?.abort()
		);
		await app.close();
		silent.close();
		media.close();
	}
});
