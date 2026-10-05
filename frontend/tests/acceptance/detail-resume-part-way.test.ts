// Acceptance: the detail page's Play button goes back to an episode
// left part-way.
//
// An episode's watched mark is written when it starts, so the last
// watched episode is the one the viewer was in when they left. With
// its position kept, the button names that episode as one to
// continue — not to replay — and clicking it resolves that episode,
// not the next one. Without a kept position it goes on to the next
// episode as it always did.

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
import { readPosition, savePosition } from '../../src/lib/play/watch-position';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
/** The last episode the history records as watched. */
const LAST = 5;

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
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json({ id: 'show-1', ep_no: String(LAST), title: TITLE })
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

describe('detail route — Play goes back to an episode left part-way', () => {
	it('names the episode left part-way as one to continue, and plays it', async () => {
		savePosition(KITSU_ID, LAST, 600, 1420);
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: String(LAST) });
		await until(() => playButton(label) !== null, 'the Play button naming the episode left');
		expect(playButton(m.detail_play_button_replay({ episode: String(LAST) }))).toBeNull();

		playButton(label)!.click();
		await until(() => clickStreams(LAST).length > 0, 'the click to resolve the episode left');
		expect(clickStreams(LAST + 1)).toHaveLength(0);
	});

	it('goes on to the next episode when nothing was left part-way', async () => {
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: String(LAST + 1) });
		await until(() => playButton(label) !== null, 'the Play button naming the next episode');

		playButton(label)!.click();
		await until(() => clickStreams(LAST + 1).length > 0, 'the click to resolve the next episode');
	});

	it('marks the episode started as soon as Play is clicked, before the session lands', async () => {
		// The resolve records the watch; if the viewer never reaches the
		// player, the mark is what keeps Continue on this episode.
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: String(LAST + 1) });
		await until(() => playButton(label) !== null, 'the Play button');
		expect(readPosition(KITSU_ID, LAST + 1)).toBeNull();
		playButton(label)!.click();
		expect(readPosition(KITSU_ID, LAST + 1)).toBe(0);
	});
});
