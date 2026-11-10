// Acceptance: the topbar's back chevron returns to the search a show
// was opened from, the way the browser's back button does.
//
// The chevron walks "up" the breadcrumb trail, and a show's trail is
// Home then the show, so from a show it opened Home: a search the
// reader came from was neither named nor reachable from the topbar.
// When the page behind is a search, the trail now names it, and the
// chevron (or the crumb) steps back to that history entry rather than
// opening a new one, so the search page comes back as it was left.
//
// With the router mocked, the scenario plays SvelteKit as
// history-entry-stamp.test.ts does, through ./session-history.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet, tick } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setPageState, setUrl } from './page-state.svelte';
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

import { afterNavigate, goto, replaceState } from '$app/navigation';
import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { breadcrumb as trail, defaultTrailFor } from '../../src/lib/breadcrumb';

const quietBackend = [
	http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 }))
];

let target: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let historyWrites: ReturnType<typeof vi.spyOn>;
let backPresses: ReturnType<typeof vi.spyOn>;

/** Land a navigation the way SvelteKit and the browser do, through
 *  the session-history model (./session-history): the launch, a link
 *  that adds an entry, or a back press onto `to`. */
function land(type: 'enter' | 'link' | 'popstate', to: string) {
	if (type === 'enter') sessionHistory.open();
	else if (type === 'link') sessionHistory.push('link', to);
	else sessionHistory.back();
	const landed = page.url.pathname + page.url.search;
	if (landed !== to) throw new Error(`landed on ${landed}, not ${to}`);
}

const breadcrumb = () =>
	target.querySelector(`nav[aria-label="${m.app_nav_breadcrumb_aria_label()}"]`);

beforeEach(async () => {
	__resetApiBaseForTests(API_BASE);
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: async () => ({ ok: false, kind: 'not_found' }) }
	};
	vi.mocked(afterNavigate).mockClear();
	vi.mocked(replaceState).mockReset();
	// SvelteKit's replaceState puts the state it was given on page.state.
	vi.mocked(replaceState).mockImplementation((_url, state) => setPageState(state));
	historyWrites = vi.spyOn(window.history, 'replaceState');
	backPresses = vi.spyOn(window.history, 'back').mockImplementation(() => {});
	vi.mocked(goto).mockClear();
	server.use(...quietBackend);
	sessionHistory.install('/');
	target = document.createElement('div');
	document.body.appendChild(target);
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target, props: { children } });
	// The boot reads (settings, then the update check) answer before the
	// scenario starts, so none is still out when its page unmounts.
	for (let i = 0; i < 40; i++) await new Promise((r) => setTimeout(r, 5));
});

afterEach(() => {
	if (layout) unmount(layout);
	layout = null;
	target.remove();
	historyWrites.mockRestore();
	backPresses.mockRestore();
	sessionHistory.uninstall();
	setPageState({});
	setUrl('/search');
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

/** The trail a route sets once it is up: the show page names the show
 *  under Home, the search page is Home then Search. The stub router
 *  carries no route id, so the scenario sets it as the route would. */
function routeTrail(path: string) {
	if (path.startsWith('/anime/')) {
		trail.set([{ label: m.breadcrumb_home(), href: '/' }, { label: 'Frieren' }]);
	} else {
		trail.set(defaultTrailFor(path.split('?')[0]));
	}
}

/** A navigation lands, and the route it lands on sets its trail. */
function go(...args: Parameters<typeof land>) {
	land(...args);
	routeTrail(args[1]);
}

const crumbs = () =>
	[...(breadcrumb()?.querySelectorAll('li:not(.sep)') ?? [])].map((li) => li.textContent?.trim());
const chevron = () =>
	breadcrumb()?.querySelector(`a[aria-label="${m.breadcrumb_up_aria_label()}"]`) as HTMLElement;
const crumbLink = (label: string) =>
	[...(breadcrumb()?.querySelectorAll('li a') ?? [])].find(
		(a) => a.textContent?.trim() === label
	) as HTMLElement | undefined;

describe('the topbar back chevron on a show opened from a search', () => {
	it('names the search in the trail and steps back to it', async () => {
		go('enter', '/');
		go('link', '/search?q=frieren');
		go('link', '/anime/46474');
		await tick();
		expect(crumbs()).toEqual([m.breadcrumb_home(), m.breadcrumb_search(), 'Frieren']);
		expect(crumbLink(m.breadcrumb_search())?.getAttribute('href')).toBe('/search?q=frieren');

		chevron().click();
		expect(backPresses).toHaveBeenCalledTimes(1);
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
	});

	it('steps back from the Search crumb as well', async () => {
		go('enter', '/');
		go('link', '/search?q=frieren');
		go('link', '/anime/46474');
		await tick();
		crumbLink(m.breadcrumb_search())!.click();
		expect(backPresses).toHaveBeenCalledTimes(1);
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
	});

	it('still names the search after the reader comes back to the show from another page', async () => {
		go('enter', '/');
		go('link', '/search?q=frieren');
		go('link', '/anime/46474');
		go('link', '/anime/52991');
		await tick();
		expect(crumbs()).toEqual([m.breadcrumb_home(), 'Frieren']);

		go('popstate', '/anime/46474');
		await tick();
		expect(crumbs()).toEqual([m.breadcrumb_home(), m.breadcrumb_search(), 'Frieren']);
		chevron().click();
		expect(backPresses).toHaveBeenCalledTimes(1);
	});

	it('opens Home, as before, from a show that was not opened from a search', async () => {
		go('enter', '/');
		go('link', '/anime/46474');
		await tick();
		expect(crumbs()).toEqual([m.breadcrumb_home(), 'Frieren']);
		chevron().click();
		expect(vi.mocked(goto)).toHaveBeenCalledWith('/');
		expect(backPresses).not.toHaveBeenCalled();
	});

	it('keeps the search page its own trail', async () => {
		go('enter', '/');
		go('link', '/search?q=frieren');
		go('link', '/search?q=himmel');
		await tick();
		expect(crumbs()).toEqual([m.breadcrumb_home(), m.breadcrumb_search()]);
	});
});
