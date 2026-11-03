// Acceptance: the topbar's back affordance after a navigation that
// replaced the page's history entry.
//
// Two navigations in the app replace the entry on screen rather than
// add one: the detail page's episode pager (`?page=`) and the player's
// episode switch. Both go through `goto(url, { replaceState: true })`,
// which SvelteKit reports to `afterNavigate` as a `goto`, the same as a
// navigation that adds an entry. Counted as a step forward, each left
// the layout one entry deeper than the real history, so after a back
// press onto the page the app opened on the breadcrumb still offered
// a way back from a page with nothing behind it.
//
// The real page makes the navigation. The mocked `goto` hands what the
// page asked for to the session-history model (./session-history),
// which adds or replaces an entry as the browser would and reports the
// navigation to the layout as SvelteKit does.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet, flushSync } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setPageState, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { sessionHistory } from './session-history';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn(),
	replaceState: vi.fn()
}));

import { afterNavigate, beforeNavigate, goto, replaceState } from '$app/navigation';
import Layout from '../../src/routes/+layout.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { m } from '../../src/lib/paraglide/messages';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
const EPISODES = 24;

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

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

function useShowHandlers() {
	server.use(
		http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: EPISODES, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(EPISODES))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

let chrome: HTMLElement;
let content: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let route: ReturnType<typeof mount> | null = null;

beforeEach(async () => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: () => ({ ok: false, kind: 'not_found' }) }
	};
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	vi.mocked(afterNavigate).mockClear();
	vi.mocked(beforeNavigate).mockClear();
	vi.mocked(replaceState).mockReset();
	// SvelteKit's replaceState puts the state it was given on page.state.
	vi.mocked(replaceState).mockImplementation((_url, state) => setPageState(state));
	// The page's own navigations land in the browser's history.
	vi.mocked(goto).mockReset();
	vi.mocked(goto).mockImplementation(async (url, opts) => sessionHistory.goto(url, opts));
	useShowHandlers();
	setParams({ id: KITSU_ID });
	sessionHistory.install('/');
	chrome = document.createElement('div');
	content = document.createElement('div');
	document.body.append(chrome, content);
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target: chrome, props: { children } });
	sessionHistory.open();
	// The boot reads (settings, then the update check) answer before the
	// scenario starts.
	for (let i = 0; i < 40; i++) await new Promise((r) => setTimeout(r, 5));
});

afterEach(() => {
	if (route) unmount(route);
	route = null;
	if (layout) unmount(layout);
	layout = null;
	chrome.remove();
	content.remove();
	sessionHistory.uninstall();
	delete g.EventSource;
	setPageState({});
	setUrl('/search');
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${content.textContent}`);
}

const breadcrumb = () =>
	chrome.querySelector(`nav[aria-label="${m.app_nav_breadcrumb_aria_label()}"]`);

/** A link from Home lands on `url`, and its route mounts. */
function openFromHome(url: string, Page: typeof DetailPage | typeof PlayPage) {
	sessionHistory.push('link', url);
	route = mount(Page, { target: content });
	flushSync();
}

/** The viewer presses back. The route on screen goes with the entry it
 *  leaves, before the one pressed onto lands. */
function pressBackOutOfRoute() {
	if (route) unmount(route);
	route = null;
	sessionHistory.back();
	flushSync();
}

describe('the back affordance after paging a show’s episodes', () => {
	it('is gone once back on the page the app opened on', async () => {
		openFromHome(`/anime/${KITSU_ID}`, DetailPage);
		const next = () =>
			content.querySelector(
				`button[aria-label="${m.detail_episodes_pager_next_aria_label()}"]`
			) as HTMLButtonElement | null;
		await until(() => next() !== null && !next()!.disabled, 'the episode pager');
		expect(breadcrumb(), 'Home is behind the show').not.toBeNull();

		next()!.click();
		await until(() => page.url.searchParams.get('page') === '2', 'the second page of episodes');
		expect(vi.mocked(goto)).toHaveBeenLastCalledWith(
			expect.stringContaining('page=2'),
			expect.objectContaining({ replaceState: true })
		);
		expect(sessionHistory.index, 'the pager replaced the show’s entry').toBe(1);
		flushSync();
		expect(breadcrumb(), 'Home is still behind the show').not.toBeNull();

		pressBackOutOfRoute();
		expect(sessionHistory.index).toBe(0);
		expect(breadcrumb(), 'nothing is behind Home').toBeNull();
	});
});

describe('the back affordance after switching episodes in the player', () => {
	it('is gone once back on the page the app opened on', async () => {
		openFromHome(`/play/${KITSU_ID}?session=session-1&episode=1&kind=mp4`, PlayPage);
		await until(() => (content.textContent ?? '').includes(TITLE), 'the show detail');
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=2')),
			'the next-episode warm stream'
		);
		FakeEventSource.instances
			.filter((i) => i.url.includes('episode=2'))
			.at(-1)!
			.dispatch(
				'done',
				JSON.stringify({
					session_id: 'session-2',
					media_url: `${API_BASE}/s/session-2/file.mp4`,
					media_kind: 'mp4'
				})
			);
		await new Promise((r) => setTimeout(r, 20));
		expect(breadcrumb(), 'Home is behind the player').not.toBeNull();

		(content.querySelector('li[data-ep-num="2"] button') as HTMLButtonElement).click();
		await until(() => page.url.searchParams.get('episode') === '2', 'the switch to episode 2');
		expect(vi.mocked(goto)).toHaveBeenLastCalledWith(
			expect.stringContaining('episode=2'),
			expect.objectContaining({ replaceState: true })
		);
		expect(sessionHistory.index, 'the switch replaced the player’s entry').toBe(1);
		flushSync();
		expect(breadcrumb(), 'Home is still behind the player').not.toBeNull();

		pressBackOutOfRoute();
		expect(sessionHistory.index).toBe(0);
		expect(breadcrumb(), 'nothing is behind Home').toBeNull();
	});
});
