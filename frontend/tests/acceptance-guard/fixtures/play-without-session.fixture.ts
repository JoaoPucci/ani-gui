import { it } from 'vitest';
import { http, HttpResponse } from 'msw';

import { API_BASE, server } from '../../acceptance/setup';
import { wait } from './helpers';

import type { CreateSessionResponse } from '../../../src/lib/api';
import { malformedAnswer, sessionAnswer } from '../../answers';
const play = (body: object) =>
	fetch(`${API_BASE}/api/play`, { method: 'POST', body: JSON.stringify(body) }).catch(() => {});

it('answers a play with an empty 204', async () => {
	server.use(http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })));
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with an older shape', async () => {
	server.use(http.post(`${API_BASE}/api/play`, () => malformedAnswer({ id: 's-1', kind: 'mp4' })));
	await play({ episode: '1' });
	await wait(20);
});

it('answers a play with a session', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json<CreateSessionResponse>(
				sessionAnswer({
					session_id: 's-1',
					media_url: `${API_BASE}/s/s-1/file.mp4`,
					media_kind: 'mp4'
				})
			)
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

/** A whole session with `wrong` over it: one field the guard must
 *  refuse, so each scenario pins one check. */
const sessionWith = (wrong: Record<string, unknown>) =>
	malformedAnswer({
		...sessionAnswer({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4'
		}),
		...wrong
	});

/** A whole session less `field`. */
const sessionWithout = (
	field: 'session_id' | 'media_url' | 'media_kind' | 'cache_hit' | 'subtitles'
) => {
	const whole: Record<string, unknown> = {
		...sessionAnswer({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4'
		})
	};
	delete whole[field];
	return malformedAnswer(whole);
};

const ONE_WRONG: [string, () => HttpResponse<never>][] = [
	[
		'whose media kind is not one there is',
		() => sessionWith({ media_url: `${API_BASE}/s/s-1/manifest.mpd`, media_kind: 'dash' })
	],
	['whose version is not one there is', () => sessionWith({ version: 'directors-cut' })],
	['whose episode is not text', () => sessionWith({ episode: 3 })],
	[
		'whose subtitle has no default flag',
		() => sessionWith({ subtitles: [{ lang: 'en', label: 'English', url: '/s/s-1/sub/0.vtt' }] })
	],
	['whose cache_hit is not a flag', () => sessionWith({ cache_hit: 'yes' })],
	['whose episode_unverified is not a flag', () => sessionWith({ episode_unverified: 'no' })],
	['whose own_listing is not a flag', () => sessionWith({ own_listing: 1 })],
	['that leaves out cache_hit', () => sessionWithout('cache_hit')],
	['that leaves out subtitles', () => sessionWithout('subtitles')],
	['that leaves out session_id', () => sessionWithout('session_id')],
	['that leaves out media_url', () => sessionWithout('media_url')],
	['that leaves out media_kind', () => sessionWithout('media_kind')],
	[
		'whose subtitles are not a list',
		() =>
			sessionWith({
				subtitles: { lang: 'en', label: 'English', default: true, url: '/s/s-1/sub/0.vtt' }
			})
	],
	[
		'whose versions are not a list',
		() => sessionWith({ versions: { kind: 'standard', show_key: 'k' } })
	],
	[
		'whose subtitle language is not text',
		() =>
			sessionWith({
				subtitles: [
					{ ...{ lang: 'en', label: 'English', default: true, url: '/s/s-1/sub/0.vtt' }, lang: 1 }
				]
			})
	],
	[
		'whose subtitle label is not text',
		() =>
			sessionWith({
				subtitles: [
					{
						...{ lang: 'en', label: 'English', default: true, url: '/s/s-1/sub/0.vtt' },
						label: null
					}
				]
			})
	],
	[
		'whose subtitle url is not text',
		() =>
			sessionWith({
				subtitles: [
					{ ...{ lang: 'en', label: 'English', default: true, url: '/s/s-1/sub/0.vtt' }, url: 0 }
				]
			})
	],
	['whose session_id is not text', () => sessionWith({ session_id: 7 })],
	['whose media_url is not text', () => sessionWith({ media_url: null })],
	['whose former_episode is not text', () => sessionWith({ former_episode: 24.5 })],
	[
		'whose version entry is not a kind there is',
		() => sessionWith({ versions: [{ kind: 'directors-cut', show_key: 'hianime:a-1' }] })
	],
	['whose version entry has no show key', () => sessionWith({ versions: [{ kind: 'standard' }] })],
	[
		'whose version entry has_mode is not a flag',
		() =>
			sessionWith({ versions: [{ kind: 'standard', show_key: 'hianime:a-1', has_mode: 'yes' }] })
	]
];

for (const [what, answer] of ONE_WRONG) {
	it(`answers a play with a session ${what}`, async () => {
		server.use(http.post(`${API_BASE}/api/play`, answer));
		await play({ episode: '1' });
		await wait(20);
	});
}

it('answers a play with a whole session', async () => {
	server.use(
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json<CreateSessionResponse>({
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
