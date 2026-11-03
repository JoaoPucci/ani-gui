// Acceptance: a captions pick on the play page warms the next episode
// again for the language picked, and Next plays that warm.
//
// The backend keeps the subtitle tracks a listing past its cap has room
// for by the show's remembered choice, so a session warmed before the
// pick may have been cut without the language picked. The play cache
// keys on the choice: a warm for the old choice never answers a switch
// asked for the new one.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { playerVideo, playerVideoInSlot } from './player-video';
import { m } from '../../src/lib/paraglide/messages';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
}));

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

/** The episodes the page stamped as watched, in order. */
const watched: string[] = [];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	watched.length = 0;
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}`);
}

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

const LISTING = (session: string) => [
	{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/${session}/sub/0.vtt` },
	{ lang: 'es-419', label: 'Español', default: false, url: `${API_BASE}/s/${session}/sub/1.vtt` }
];

function handlers() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'The Show', 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
			watched.push(String(((await request.json()) as { episode: string }).episode));
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/s/:session/subtitles`, ({ params }) =>
			HttpResponse.json(LISTING(String(params.session)))
		)
	);
}

const params = (es: FakeEventSource) => new URL(es.url).searchParams;
const warmsOf = (episode: string) =>
	FakeEventSource.instances.filter((es) => params(es).get('episode') === episode);

/** Open the captions menu and pick the item named `name`. */
async function pick(name: string) {
	const cc = target.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_controls_cc_aria_label()}"]`
	);
	cc!.click();
	await until(() => document.querySelector('#pc-cc-menu') !== null, 'the captions menu');
	const item = Array.from(
		document.querySelectorAll<HTMLButtonElement>('#pc-cc-menu [role="menuitemradio"]')
	).find((b) => b.textContent?.trim().endsWith(name));
	if (!item) throw new Error(`no captions item named ${name}`);
	item.click();
	await tick();
}

const nextButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
	) as HTMLButtonElement | null;

describe('play route — a captions pick and the warmed next episode', () => {
	it('warms the next episode again for the language picked, and Next plays that warm', async () => {
		handlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-3', episode: '3', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => playerVideo().textTracks.length === 2, 'the tracks attached');
		await until(() => warmsOf('4').length === 1, 'the warm of the next episode');
		const before = warmsOf('4')[0];
		expect(params(before).has('caption')).toBe(false);

		await pick('Spanish (Latin America)');
		await until(() => warmsOf('4').length === 2, 'the warm for the language picked');
		const after = warmsOf('4')[1];
		expect(params(after).get('caption')).toBe('es-419');

		after.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-4',
				media_url: `${API_BASE}/s/session-4/file.mp4`,
				media_kind: 'mp4'
			})
		);
		// Let the answer settle the warm before the click, as any real
		// click comes after it.
		await new Promise((r) => setTimeout(r, 0));
		const opened = FakeEventSource.instances.length;
		await until(() => nextButton() !== null && !nextButton()!.disabled, 'the Next button');
		nextButton()!.click();
		// The switch played a session without asking for one: the warm
		// for the language picked, since the one before was dropped
		// unanswered.
		await until(() => watched.includes('4'), 'the switch to the next episode');
		expect(before.closed).toBe(true);
		expect(FakeEventSource.instances.length).toBe(opened);
	});
});
