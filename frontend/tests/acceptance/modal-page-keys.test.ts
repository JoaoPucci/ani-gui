// Acceptance: a modal's page does not answer its own shortcuts behind it.
//
// The player's shortcuts and the topbar's search shortcuts listen on
// the window, so they kept acting while a modal held the keyboard. With
// a play failure up, n or p switched the episode under it and f put the
// player back into fullscreen, which hides the failure the modal had
// just left fullscreen to show. Ctrl+K and / opened the topbar search
// behind the update dialog. A modal is the page's only keyboard target
// while it is up.
//
// It also pins the show page's play failure handing focus back to the
// episode that was clicked, through the loading overlay that came
// before it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import Layout from '../../src/routes/+layout.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { updateStore } from '../../src/lib/update/store.svelte';
import { RECENT_STORAGE_KEY } from '../../src/lib/topbar/recent-searches';

const KITSU_ID = '40684';
const TITLE = 'Ongoing Show';
const VERDICT = JSON.stringify({
	kind: 'episode_unavailable',
	key: 'error.play.episode_unavailable'
});

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

let target: HTMLElement;
let apps: ReturnType<typeof mount>[] = [];
const fullscreenRequests = vi.fn(async () => {});
const proto = HTMLElement.prototype as unknown as { requestFullscreen?: unknown };
const realRequestFullscreen = proto.requestFullscreen;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	fullscreenRequests.mockClear();
	proto.requestFullscreen = fullscreenRequests;
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	for (const app of apps) unmount(app);
	apps = [];
	updateStore.closeDialog();
	updateStore.setAvailable(null);
	proto.requestFullscreen = realRequestFullscreen;
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
const settle = () => new Promise((r) => setTimeout(r, 50));
const active = () => document.activeElement;

/** A key pressed where focus is, as the window's listeners hear it. */
function press(key: string, init: KeyboardEventInit = {}) {
	const at = (document.activeElement ?? document.body) as HTMLElement;
	at.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
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

function useShowHandlers() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig('sub'))),
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
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json({ cached: {}, playable_episode_counts: {} })
		),
		http.post(`${API_BASE}/api/availability/warm`, () => new HttpResponse(null, { status: 202 })),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

const tile = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;
const failure = () => target.querySelector<HTMLElement>('[role="alertdialog"]');
const interactive = (episode: number) =>
	FakeEventSource.instances.filter(
		(i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1')
	);

/** Click episode `episode`'s tile from the keyboard's place on it and
 *  fail its stream; resolves once the failure is up. */
async function clickAndFail(episode: number) {
	await until(() => tile(episode) !== null, `the tile for episode ${episode}`);
	const opener = tile(episode)!;
	opener.focus();
	opener.click();
	await until(() => interactive(episode).length > 0, "the click's interactive stream");
	interactive(episode).pop()!.dispatch('error', VERDICT);
	await until(() => failure() !== null, 'the failure overlay');
	await settle();
	return opener;
}

describe('the player, under a play failure', () => {
	async function mountPlayer() {
		useShowHandlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		apps.push(mount(PlayPage, { target }));
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	}

	it('answers f with fullscreen while nothing is over it', async () => {
		await mountPlayer();
		await settle();
		(active() as HTMLElement | null)?.blur();
		press('f');
		expect(fullscreenRequests).toHaveBeenCalledTimes(1);
	});

	it('does not go fullscreen on f, nor switch episode on n or p', async () => {
		await mountPlayer();
		await clickAndFail(2);
		const streams = FakeEventSource.instances.length;
		for (const key of ['f', 'n', 'p']) press(key);
		await settle();
		expect(fullscreenRequests).not.toHaveBeenCalled();
		expect(FakeEventSource.instances.length).toBe(streams);
		expect(failure()).not.toBeNull();
	});
});

describe('a show page’s play failure', () => {
	it('gives focus back to the episode that was clicked when it closes', async () => {
		useShowHandlers();
		apps.push(mount(DetailPage, { target }));
		const opener = await clickAndFail(5);
		expect(failure()!.contains(active())).toBe(true);
		press('Escape');
		await until(() => failure() === null, 'the failure to close');
		await until(() => active() === opener, 'focus back on the episode');
	});
});

describe('the topbar, under the update dialog', () => {
	it('opens no search on Ctrl+K or /', async () => {
		server.use(
			http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
			...homeHandlers({})
		);
		// A remembered search, so an opened search shows its dropdown.
		window.localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(['bebop']));
		const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
		apps.push(mount(Layout, { target, props: { children } }));
		await settle();
		const search = target.querySelector<HTMLInputElement>('.topbar input');
		expect(search).not.toBeNull();
		updateStore.setAvailable({
			tag: 'v9.9.9',
			name: 'v9.9.9',
			url: 'https://example.invalid/releases/v9.9.9',
			publishedAt: '2026-11-01T00:00:00Z',
			body: 'Notes.'
		});
		updateStore.openDialog();
		await until(() => target.querySelector('.update-dialog') !== null, 'the update dialog');
		await settle();
		const dialog = target.querySelector('.update-dialog')!;
		press('k', { ctrlKey: true });
		press('/');
		await settle();
		expect(active()).not.toBe(search);
		expect(dialog.contains(active())).toBe(true);
		expect(target.querySelector('.topbar-dropdown')).toBeNull();
		// Nor one left open for when the dialog closes.
		press('Escape');
		await until(() => target.querySelector('.update-dialog') === null, 'the dialog to close');
		await settle();
		expect(target.querySelector('.topbar-dropdown')).toBeNull();
	});
});
