// Acceptance: the topbar's breadcrumb keeps the title a page named
// itself with.
//
// The layout resets the trail to a URL-only default ("Home · Anime")
// when SvelteKit reports a navigation (`afterNavigate`), and the show
// page and the player name themselves once their data lands. SvelteKit
// reports a navigation only after the page it lands on has rendered,
// so whatever the page named before that, the reset took away:
//
//   - the same show picked again from the topbar search, or any other
//     navigation to the address on screen: the show page stays mounted,
//     names nothing new, and the trail read "Home · Search · Anime";
//   - the episode pager's `?page=` on the show page, likewise;
//   - the player's switch to another episode: the player named the new
//     episode, and the trail read "Home · Watching";
//   - a show opened afresh whose detail landed before the report.
//
// With the router mocked, the scenarios play SvelteKit and the browser
// through ./session-history, as topbar-back-to-search.test.ts does,
// with the real pages mounted beside the layout. Where the page on
// screen stays (the same address, the pager, the episode switch) or
// its detail lands first, the scenario lets the page name itself before
// it reports the navigation, as SvelteKit's order allows. The stub
// page carries the route id SvelteKit would, so the layout's defaults
// are the ones the app shows.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet, tick } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setPageState, setParams, setUrl } from './page-state.svelte';
import { sessionHistory } from './session-history';
import { appConfig, kitsuRef } from './home-handlers';

vi.mock('$app/state', () => ({
	get page() {
		// The stub page with the route id SvelteKit resolves for its path.
		return {
			get url() {
				return page.url;
			},
			get params() {
				return page.params;
			},
			get state() {
				return page.state;
			},
			get route() {
				const path = page.url.pathname;
				if (path === '/') return { id: '/' };
				const show = path.match(/^\/(anime|play)\/[^/]+$/);
				return { id: show ? `/${show[1]}/[id]` : path };
			}
		};
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn(),
	replaceState: vi.fn()
}));

import { afterNavigate, replaceState } from '$app/navigation';
import Layout from '../../src/routes/+layout.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';

const SHOW = '46474';
const TITLE = 'Frieren';

let target: HTMLElement;
let pageTarget: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let mounted: ReturnType<typeof mount> | null = null;
/** Hold the show's detail until the scenario lets it go. */
let detailHeld: Promise<void> | null = null;

class FakeEventSource {
	addEventListener() {}
	close() {}
}
type GlobalLike = { EventSource?: unknown };
const g = globalThis as unknown as GlobalLike;

function useBackend() {
	server.use(
		http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, async ({ params }) => {
			if (detailHeld) await detailHeld;
			return HttpResponse.json({ ...kitsuRef(String(params.id), TITLE, 12), status: 'finished' });
		}),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
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
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

beforeEach(async () => {
	__resetApiBaseForTests(API_BASE);
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: async () => ({ ok: false, kind: 'not_found' }) }
	};
	g.EventSource = FakeEventSource;
	detailHeld = null;
	vi.mocked(afterNavigate).mockClear();
	vi.mocked(replaceState).mockReset();
	vi.mocked(replaceState).mockImplementation((_url, state) => setPageState(state));
	useBackend();
	sessionHistory.install('/');
	target = document.createElement('div');
	pageTarget = document.createElement('div');
	document.body.appendChild(target);
	document.body.appendChild(pageTarget);
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target, props: { children } });
	// The boot reads answer before the scenario starts.
	for (let i = 0; i < 40; i++) await new Promise((r) => setTimeout(r, 5));
	// Every scenario starts on Home, which drops any trail a page named
	// in an earlier one.
	sessionHistory.open();
});

afterEach(async () => {
	if (mounted) unmount(mounted);
	mounted = null;
	// Let the page's last reads settle before the next scenario.
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
	if (layout) unmount(layout);
	layout = null;
	target.remove();
	pageTarget.remove();
	sessionHistory.uninstall();
	setPageState({});
	setParams({});
	setUrl('/search');
	delete g.EventSource;
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

async function until(predicate: () => boolean, what: string, timeoutMs = 5_000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- crumbs ---\n${crumbs().join(' · ')}`);
}

const crumbs = () =>
	[
		...(target
			.querySelector(`nav[aria-label="${m.app_nav_breadcrumb_aria_label()}"]`)
			?.querySelectorAll('li:not(.sep)') ?? [])
	].map((li) => li.textContent?.trim() ?? '');
/** The show page has its detail: the title is on the page. */
const landed = () => pageTarget.textContent?.includes(TITLE) ?? false;

const home = m.breadcrumb_home();
const search = m.breadcrumb_search();
const showPath = `/anime/${SHOW}`;

function mountShow() {
	setParams({ id: SHOW });
	mounted = mount(DetailPage, { target: pageTarget });
}

/** Unmount the page on screen, as SvelteKit does on leaving its route. */
function leave() {
	if (mounted) unmount(mounted);
	mounted = null;
}

/** Open the show the way it goes the first time: SvelteKit reports the
 *  navigation, and the show's detail lands after it. */
async function openShow() {
	sessionHistory.push('link', showPath);
	mountShow();
	await until(landed, 'the show detail to land');
	await tick();
}

describe('the breadcrumb on a show', () => {
	it('names the show after the same show is picked again from the topbar search', async () => {
		sessionHistory.push('link', '/search?q=frieren');
		await openShow();
		expect(crumbs()).toEqual([home, search, TITLE]);

		// The topbar's live results go to the address on screen; the
		// show page stays mounted.
		sessionHistory.push('goto', showPath);
		await tick();
		expect(crumbs()).toEqual([home, search, TITLE]);
	});

	it('keeps the title when the show page moves to another page of episodes', async () => {
		await openShow();
		sessionHistory.replace(`${showPath}?page=2`);
		await tick();
		expect(crumbs()).toEqual([home, TITLE]);
	});

	it('names a show opened again whose detail lands before the navigation is reported', async () => {
		sessionHistory.push('link', '/search?q=frieren');
		await openShow();
		leave();
		sessionHistory.back();
		await tick();
		expect(crumbs()).toEqual([home, search]);

		setUrl(showPath);
		mountShow();
		await until(landed, 'the show detail to land');
		sessionHistory.push('link', showPath);
		await tick();
		expect(crumbs()).toEqual([home, search, TITLE]);
	});

	it('does not carry the title onto the next page', async () => {
		sessionHistory.push('link', '/search?q=frieren');
		await openShow();
		leave();
		sessionHistory.push('link', '/search?q=himmel');
		await tick();
		expect(crumbs()).toEqual([home, search]);
	});

	it('does not name the page after it when its detail lands late', async () => {
		let release!: () => void;
		detailHeld = new Promise<void>((r) => (release = r));
		sessionHistory.push('link', showPath);
		mountShow();
		await tick();

		// Off to the player for the same show before the detail lands:
		// the route keeps the id, the show page goes.
		leave();
		sessionHistory.push('link', `/play/${SHOW}?episode=1`);
		await tick();
		release();
		for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
		expect(crumbs()).toEqual([home, m.breadcrumb_watching()]);
	});
});

describe('the breadcrumb on the player', () => {
	it('names the new episode after the player switches to it', async () => {
		sessionHistory.push('link', `/play/${SHOW}?episode=1`);
		setParams({ id: SHOW });
		mounted = mount(PlayPage, { target: pageTarget });
		const ep = (n: number) => m.play_head_title_episode_prefix({ episode: String(n) });
		await until(() => crumbs().join('|') === [home, TITLE, ep(1)].join('|'), 'episode 1');

		// The player moves its URL to the next episode, renders it, and
		// only then is the navigation reported.
		setUrl(`/play/${SHOW}`, { episode: '2' });
		await tick();
		sessionHistory.replace(`/play/${SHOW}?episode=2`);
		await tick();
		expect(crumbs()).toEqual([home, TITLE, ep(2)]);
	});
});
