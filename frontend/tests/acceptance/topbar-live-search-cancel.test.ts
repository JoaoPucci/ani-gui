// Acceptance: a live search the query has moved past is cancelled.
//
// The topbar searches Kitsu as the viewer types, once the typing
// settles, and drops the answer to a query the viewer has typed past.
// The search itself went on, holding one of the API lane's six
// connections until Kitsu answered: a viewer typing a title slowly
// while Kitsu was slow left one search out per pause, and the page's
// own reads queued behind them. And had the dropped search been
// cancelled, its failure would have shown as Kitsu failing.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setUrl } from './page-state.svelte';
import { kitsuRef } from './home-handlers';
import { sixConnections, type Connections } from './connections';
import { hangingReads, type HangingReads } from './hanging-reads';

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

import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: async () => ({ ok: false, kind: 'not_found' }) }
	};
	reads = hangingReads();
	server.use(
		http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 }))
	);
	setUrl('/');
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (layout) unmount(layout);
	layout = null;
	pool?.restore();
	pool = null;
	reads.release();
	target.remove();
	localStorage.clear();
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

async function until(predicate: () => boolean, what: string, timeoutMs = 4000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const input = () =>
	target.querySelector(`input[aria-label="${m.app_search_aria_label()}"]`) as HTMLInputElement;

function type(q: string) {
	input().value = q;
	input().dispatchEvent(new Event('input', { bubbles: true }));
}

describe('topbar live search — the query moves on', () => {
	it('cancels the search typed past, says nothing about it, and shows the next one', async () => {
		const asked: string[] = [];
		server.use(
			http.post(`${API_BASE}/api/kitsu/search`, async (info) => {
				const { query } = (await info.request.clone().json()) as { query: string };
				asked.push(query);
				if (query === 'fri') return reads.hang(info);
				return HttpResponse.json([kitsuRef('7', 'Frieren', 28)]);
			}),
			http.post(`${API_BASE}/api/availability/batch`, () =>
				HttpResponse.json({ cached: {}, playable_episode_counts: {} })
			)
		);
		pool = sixConnections();
		const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
		layout = mount(Layout, { target, props: { children } });
		input().focus();
		input().dispatchEvent(new FocusEvent('focus'));
		type('fri');
		await until(() => reads.held.length === 1, 'the first search to be out');
		type('frieren');
		await until(() => pool!.cancelled.includes('/api/kitsu/search'), 'the first search cancelled');
		// Before the next search answers, the cancelled one shows
		// nothing: no failure, no empty list.
		await new Promise((r) => setTimeout(r, 0));
		expect(target.textContent).not.toContain(m.app_search_error_kitsu());
		expect(target.textContent).not.toContain(m.app_search_error_no_matches());
		await until(() => (target.textContent ?? '').includes('Frieren'), "the second search's hit");
		expect(asked).toEqual(['fri', 'frieren']);
		expect(target.textContent).not.toContain(m.app_search_error_kitsu());
	});

	it('cancels the search still out when the viewer submits and goes to the search page', async () => {
		server.use(http.post(`${API_BASE}/api/kitsu/search`, reads.hang));
		pool = sixConnections();
		const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
		layout = mount(Layout, { target, props: { children } });
		input().focus();
		input().dispatchEvent(new FocusEvent('focus'));
		type('frieren');
		await until(() => reads.held.length === 1, 'the search to be out');
		(input().closest('form') as HTMLFormElement).requestSubmit();
		await until(() => pool!.cancelled.includes('/api/kitsu/search'), 'the search cancelled');
	});

	it('sends no search the viewer submitted before it went out', async () => {
		let asked = 0;
		server.use(
			http.post(`${API_BASE}/api/kitsu/search`, () => {
				asked += 1;
				return HttpResponse.json([]);
			})
		);
		const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
		layout = mount(Layout, { target, props: { children } });
		input().focus();
		input().dispatchEvent(new FocusEvent('focus'));
		type('frieren');
		(input().closest('form') as HTMLFormElement).requestSubmit();
		await new Promise((r) => setTimeout(r, 500));
		expect(asked).toBe(0);
	});
});
