// Acceptance: the topbar's recent searches list each search once.
//
// The list is read back from local storage on launch and rendered
// keyed by the search's text, so a stored list that names a search
// twice gave the keyed list two equal keys, which Svelte refuses
// (each_key_duplicate). Searches that differ only in case or spacing
// are the same search, as the backend's search answers them, and a
// new search replaces its earlier entry instead of sitting beside it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet } from 'svelte';

import { API_BASE, server } from './setup';
import { setUrl } from './page-state.svelte';
import { afterRead, watchPageFetches } from './page-fetches';
import { armedBy } from './scheduled-timers';
import { page } from './page-state.svelte';

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
import { RECENT_STORAGE_KEY } from '../../src/lib/topbar/recent-searches';
import { m } from '../../src/lib/paraglide/messages';

import type { KitsuAnimeRef } from '../../src/lib/api';
import { errorAnswer } from '../answers';
let target: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: { getToken: async () => ({ ok: false, kind: 'not_found' }) }
	};
	server.use(
		http.get(`${API_BASE}/api/update-check`, () => errorAnswer(503)),
		http.get(`${API_BASE}/api/settings`, () => errorAnswer(503))
	);
	setUrl('/');
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (layout) unmount(layout);
	layout = null;
	target.remove();
	localStorage.clear();
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

async function mountLayout() {
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target, props: { children } });
	await settle();
}

const input = () =>
	target.querySelector(`input[aria-label="${m.app_search_aria_label()}"]`) as HTMLInputElement;

const recents = () =>
	Array.from(target.querySelectorAll('.topbar-recent')).map(
		(b) => b.querySelector('span:last-child')?.textContent?.trim() ?? ''
	);

describe('topbar recent searches', () => {
	it('lists a search the stored list repeats once, at its first place', async () => {
		localStorage.setItem(
			RECENT_STORAGE_KEY,
			JSON.stringify(['naruto', 'jojo', 'naruto', 'Jojo ', '', 42, 'bleach'])
		);
		await mountLayout();
		input().focus();
		input().dispatchEvent(new FocusEvent('focus'));
		await vi.waitFor(() => expect(recents()).toEqual(['naruto', 'jojo', 'bleach']));
	});

	it('replaces the entry for a search made again in other case or spacing', async () => {
		// Typing arms the topbar's live search, and submitting stops it
		// before it goes out: the search page runs the search. Answered
		// here in case it were sent; the scenario checks it is not.
		let liveSearched = false;
		server.use(
			http.post(`${API_BASE}/api/kitsu/search`, () => {
				liveSearched = true;
				return HttpResponse.json<KitsuAnimeRef[]>([]);
			})
		);
		localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(['one piece', 'jojo']));
		const fetches = watchPageFetches();
		await mountLayout();
		input().value = 'One  Piece';
		const pause = armedBy(() => input().dispatchEvent(new Event('input', { bubbles: true })));
		expect(pause.pending, 'typing armed the pause before the search').toBe(1);
		(input().closest('form') as HTMLFormElement).requestSubmit();
		await vi.waitFor(() =>
			expect(JSON.parse(localStorage.getItem(RECENT_STORAGE_KEY) ?? '[]')).toEqual([
				'One  Piece',
				'jojo'
			])
		);
		// Once the live search's pause has come due or been called off,
		// a search it started has been asked, and is recorded where the
		// page asks it.
		await pause.done('the pause before the live search to end');
		await afterRead();
		expect(fetches.sent.filter((f) => f.path === '/api/kitsu/search')).toEqual([]);
		expect(liveSearched).toBe(false);
	});
});
