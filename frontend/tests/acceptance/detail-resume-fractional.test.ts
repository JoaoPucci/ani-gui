// Acceptance: the detail page's Play button reads a recap in history
// as the recap.
//
// History records a recap the provider numbers 7.5 as "7.5". Read as
// a whole number it became episode 7: a position kept for the recap
// was never found, so Play went on to 8 instead of back to the recap,
// and a position kept for episode 7 was taken for the recap's.

import { describe, it, vi, beforeEach, afterEach } from 'vitest';
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

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
/** The last episode the history records as watched. */
const LAST = 7.5;

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

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
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
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function useShowHandlers() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json({ id: 'show-1', ep_no: String(LAST), title: TITLE })
		),
		// The row read again once the lookup answers reads as it did.
		http.get(`${API_BASE}/api/history/show-1/number`, () =>
			HttpResponse.json({ ep_no: String(LAST) })
		)
	);
}

/** The page's primary action, found by what it says. */
const playButton = (label: string) =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(label)
	) ?? null;

/** Resolution streams the click opened — not the page's warms. */
const clickStreams = (episode: number) =>
	FakeEventSource.instances.filter(
		(i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1')
	);

describe('detail route — a recap in history', () => {
	it('goes back to a recap left part-way', async () => {
		savePosition(KITSU_ID, LAST, 600, 1420);
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '7.5' });
		await until(() => playButton(label) !== null, 'the Play button naming the recap left');
		playButton(label)!.click();
		await until(() => clickStreams(7.5).length > 0, 'the click to resolve the recap');
	});

	it('goes on from the recap to episode 8 when nothing was left part-way', async () => {
		// Episode 7's own position is not the recap's.
		savePosition(KITSU_ID, 7, 600, 1420);
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '8' });
		await until(() => playButton(label) !== null, 'the Play button naming episode 8');
	});
});
