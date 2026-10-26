import { it } from 'vitest';
import { http, HttpResponse } from 'msw';

import { API_BASE, server } from '../../acceptance/setup';
import { wait } from './helpers';

const play = (body: object) =>
	fetch(`${API_BASE}/api/play`, { method: 'POST', body: JSON.stringify(body) }).catch(() => {});

it('answers a play with an empty 204', async () => {
	server.use(http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })));
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with an older shape', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () => HttpResponse.json({ id: 's-1', kind: 'mp4' }))
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a session', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/file.mp4`,
				media_kind: 'mp4'
			})
		)
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a warm with an empty 204', async () => {
	server.use(http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })));
	await play({ episode: '2', prefetch: true });
	await wait(20);
});

it('answers a play with a failure', async () => {
	server.use(http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 503 })));
	await play({ episode: '1' });
	await wait(20);
});

it('leaves a play behind that is answered with an empty 204 after it ended', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, async () => {
			await wait(40);
			return new HttpResponse(null, { status: 204 });
		})
	);
	void play({ episode: '1' });
	await wait(5);
});

it('runs while the earlier play is answered', async () => {
	await wait(80);
});

it('answers a play with a session whose media kind is not one there is', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/manifest.mpd`,
				media_kind: 'dash'
			})
		)
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a session whose version is not one there is', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/file.mp4`,
				media_kind: 'mp4',
				version: 'directors-cut'
			})
		)
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a session whose optional fields have the wrong type', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/file.mp4`,
				media_kind: 'mp4',
				cache_hit: 'yes',
				episode: 3,
				subtitles: [{ lang: 'en', label: 'English', url: '/s/s-1/sub/0.vtt' }]
			})
		)
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a whole session', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/master.m3u8`,
				media_kind: 'hls',
				cache_hit: true,
				subtitles: [
					{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/s-1/sub/0.vtt` }
				],
				episode: '12.5',
				former_episode: '24.5',
				versions: [
					{ kind: 'uncensored', show_key: 'hianime:a-1', has_mode: true },
					{ kind: 'standard', show_key: 'hianime:a-2' }
				],
				version: 'standard'
			})
		)
	);
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a non-session whose body is still arriving as the scenario ends', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () => {
			const body = new ReadableStream<Uint8Array>({
				async start(controller) {
					await wait(30);
					controller.enqueue(new TextEncoder().encode('{"id":"s-1","kind":"mp4"}'));
					controller.close();
				}
			});
			return new HttpResponse(body, {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		})
	);
	// The page took the answer and moved on; its body is not awaited.
	await fetch(`${API_BASE}/api/play`, {
		method: 'POST',
		body: JSON.stringify({ episode: '1' })
	}).catch(() => {});
	await wait(5);
});

it('runs after the slow body', async () => {
	await wait(80);
});
