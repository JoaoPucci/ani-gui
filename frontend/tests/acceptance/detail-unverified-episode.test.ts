// Acceptance: the detail page with a show whose rows past Kitsu's
// count no reading reached.
//
// hianime lists Noragami's two OADs as rows 13 and 14 of a
// twelve-episode show. When the backend could not read whether they
// are extras, availability answers with no count, and the strip offers
// Kitsu's twelve episodes alone. A row the history recorded as 13 then
// plays as the provider's row 13, and the session says its episode is
// unverified: it is marked watched, and nothing is sent to the
// trackers, which would clamp 13 to their own total.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { savePosition } from '../../src/lib/play/watch-position';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { Provider } from '../../src/lib/account/types';

const KITSU_ID = '42';
const TITLE = 'Noragami';
const TRACKERS: Provider[] = ['mal', 'anilist'];

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

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let writes: string[];
let marked: string[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	writes = [];
	marked = [];
	setParams({ id: KITSU_ID });
	for (const provider of TRACKERS) {
		accountStore.setConnected(provider, {
			access_token: `${provider}-bearer`,
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-1',
			username: 'tester',
			avatar_url: null
		});
	}
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	for (const provider of TRACKERS) accountStore.setDisconnected(provider);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Kitsu title ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

/** Noragami as a page sees it when the rows past the count were not
 *  read: Kitsu's twelve, available with no count, and the history's
 *  row 13. */
function useUnreadShow() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: null,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json({ id: 'hianime:noragami-916', ep_no: '13', title: TITLE })
		),
		http.get(`${API_BASE}/api/history/:id/number`, () => HttpResponse.json({ ep_no: '13' })),
		http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
			const body = (await request.json()) as { episode?: string };
			marked.push(String(body.episode));
			return new HttpResponse(null, { status: 204 });
		}),
		// The list editor reads each connected tracker's row.
		http.get(`${API_BASE}/api/account/entry/:provider`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/account/update/:provider`, ({ params }) => {
			writes.push(String(params.provider));
			return HttpResponse.json(null);
		})
	);
}

const tileNumbers = () =>
	Array.from(target.querySelectorAll('li[data-ep-num]')).map((li) =>
		Number(li.getAttribute('data-ep-num'))
	);

/** The page's primary action, found by what it says. */
const playButton = (label: string) =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(label)
	) ?? null;

describe('detail route — rows past the count no reading reached', () => {
	it('offers Kitsu’s twelve episodes alone', async () => {
		useUnreadShow();
		app = mount(DetailPage, { target });
		await until(() => tileNumbers().length > 0, 'the grid');
		expect(tileNumbers()).toEqual(Array.from({ length: 12 }, (_, i) => i + 1));
	});

	it('marks an unverified row 13 watched and sends the trackers nothing', async () => {
		// Left part-way, so Continue names row 13 itself.
		savePosition(KITSU_ID, 13, 600, 1420);
		useUnreadShow();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '13' });
		await until(() => playButton(label) !== null, 'the Play button naming row 13');
		playButton(label)!.click();
		await until(
			() =>
				FakeEventSource.instances.some(
					(i) => i.url.includes('episode=13') && !i.url.includes('prefetch=1')
				),
			'the click to resolve row 13'
		);
		const es = FakeEventSource.instances
			.filter((i) => i.url.includes('episode=13') && !i.url.includes('prefetch=1'))
			.pop()!;
		es.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-13',
				media_url: `${API_BASE}/s/session-13/master.m3u8`,
				media_kind: 'hls',
				episode: '13',
				episode_unverified: true
			})
		);
		await until(() => marked.includes('13'), 'row 13 marked watched');
		// The write-back fires beside the watch stamp; give it the turn
		// it would take.
		await new Promise((r) => setTimeout(r, 50));
		expect(writes).toEqual([]);
	});
});
