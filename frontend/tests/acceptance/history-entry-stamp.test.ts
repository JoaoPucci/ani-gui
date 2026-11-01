// Acceptance: the topbar's breadcrumb knows whether there is a page
// behind the one on screen, and the layout keeps what it needs for that
// on each history entry through SvelteKit's own state, never by writing
// `history.state` itself.
//
// SvelteKit owns `history.state`: every entry it writes carries its own
// keys, and its dev runtime warns that a direct `history.pushState` or
// `history.replaceState` "will conflict with SvelteKit's router". The
// supported way to keep something on an entry is shallow routing:
// `replaceState('', state)` from `$app/navigation`, read back on a back
// or forward press as `page.state`.
//
// With the router mocked, the scenario plays SvelteKit and the browser
// through ./session-history: it reports each navigation to the
// layout's `afterNavigate` hooks, and on a back press it puts the
// entry's state back on `page.state`, as SvelteKit restores it.

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

import { afterNavigate, replaceState } from '$app/navigation';
import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';

const quietBackend = [
	http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 }))
];

let target: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let historyWrites: ReturnType<typeof vi.spyOn>;

const breadcrumb = () =>
	target.querySelector(`nav[aria-label="${m.app_nav_breadcrumb_aria_label()}"]`);

beforeEach(async () => {
	__resetApiBaseForTests(API_BASE);
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: () => ({ ok: false, kind: 'not_found' }) }
	};
	vi.mocked(afterNavigate).mockClear();
	vi.mocked(replaceState).mockReset();
	// SvelteKit's replaceState puts the state it was given on page.state.
	vi.mocked(replaceState).mockImplementation((_url, state) => setPageState(state));
	historyWrites = vi.spyOn(window.history, 'replaceState');
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
	sessionHistory.uninstall();
	setPageState({});
	setUrl('/search');
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

describe('the layout keeps its back-stack depth on the history entry', () => {
	it('writes it through SvelteKit, never through history.replaceState', async () => {
		sessionHistory.open();
		await tick();
		expect(breadcrumb(), 'nothing is behind the page the app opened on').toBeNull();

		sessionHistory.push('link', '/search?q=frieren');
		await tick();
		expect(vi.mocked(replaceState)).toHaveBeenCalledTimes(1);
		expect(vi.mocked(replaceState)).toHaveBeenCalledWith(
			'',
			expect.objectContaining({ aniGuiDepth: 1 })
		);
		expect(historyWrites).not.toHaveBeenCalled();
		expect(breadcrumb(), 'Home is behind the search page').not.toBeNull();
	});

	it('reads it back from page.state on a back press, and writes nothing', async () => {
		sessionHistory.open();
		sessionHistory.push('link', '/search?q=frieren');
		sessionHistory.push('link', '/anime/46474');
		await tick();
		expect(breadcrumb()).not.toBeNull();

		vi.mocked(replaceState).mockClear();
		sessionHistory.back();
		await tick();
		expect(breadcrumb(), 'Home is still behind the search page').not.toBeNull();

		sessionHistory.back();
		await tick();
		expect(breadcrumb(), 'nothing is behind Home').toBeNull();
		expect(vi.mocked(replaceState)).not.toHaveBeenCalled();
		expect(historyWrites).not.toHaveBeenCalled();
	});
});
