import { it } from 'vitest';
import { http, HttpResponse } from 'msw';

import { API_BASE, server } from '../../acceptance/setup';
import { send, useHandlers, wait } from './helpers';

/** A play answered at once, with a body that takes `ms` to arrive. */
function slowBodiedPlay(ms: number, body: string) {
	server.use(
		http.post(`${API_BASE}/api/play`, () => {
			const stream = new ReadableStream<Uint8Array>({
				async start(controller) {
					await wait(ms);
					controller.enqueue(new TextEncoder().encode(body));
					controller.close();
				}
			});
			return new HttpResponse(stream, {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		})
	);
	return fetch(`${API_BASE}/api/play`, {
		method: 'POST',
		body: JSON.stringify({ episode: '1' })
	}).catch(() => {});
}

const SESSION = JSON.stringify({
	session_id: 's-1',
	media_url: `${API_BASE}/s/s-1/file.mp4`,
	media_kind: 'mp4'
});

it('leaves a request unanswered while a play answer is still being read', async () => {
	// The other request is answered after the page unmounted, while the
	// teardown waits on the play's answer: still unanswered at unmount.
	useHandlers(60);
	await slowBodiedPlay(200, SESSION);
	send('/slow');
	await wait(5);
});

it('reads a play answer past the grace that turns out to be no session', async () => {
	await slowBodiedPlay(1300, '{"id":"s-1","kind":"mp4"}');
	await wait(5);
});

it('runs after the late answers', async () => {
	await wait(400);
});
