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
