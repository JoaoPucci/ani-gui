// Acceptance: a Continue row for a long-running show Kitsu announces
// no episode total for resolves into a resume control.
//
// Kitsu leaves `episodeCount` null for an open-ended show that is
// still broadcasting — Detective Conan (id 210) and One Piece (id 12)
// both read null with status 'current'. A history row for one of them
// carries a four-figure episode count, and the null-count guard used
// to reject that pairing outright: every hit fell out of the
// candidate list, the resolver returned null, and the card rendered
// its /search fallback permanently.
//
// The other acceptance scenarios all use a finished show with a
// numeric count, so they stay green whether or not this path is
// wired through the route at all.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

/** As it reads in the watch history, episode tail and all. */
const HISTORY_TITLE = 'Meitantei Conan (1150 episodes)';
/** Kitsu files the same show under its English canonical title. */
const KITSU_TITLE = 'Detective Conan';
const WATCHED = 1100;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.innerHTML}`);
}

/** The row's three template branches. Only the resolved one is a button. */
const carrying = (el: Element | null) =>
	(el?.textContent?.includes(KITSU_TITLE) || el?.textContent?.includes('Meitantei Conan')) ?? false;
const resumeButton = () => Array.from(target.querySelectorAll('button')).find(carrying) ?? null;
const searchFallback = () =>
	Array.from(target.querySelectorAll('a[href*="search"]')).find(carrying) ?? null;

/** The main-series entry as Kitsu actually serves it: no total, airing. */
function longRunner() {
	return { ...kitsuRef('210', KITSU_TITLE, 0), episode_count: null, status: 'current' };
}

/** A Conan movie, as the text search returns alongside it. */
function movie() {
	return { ...kitsuRef('42313', 'Meitantei Conan: Konjou no Fist', 1), status: 'finished' };
}

describe('Continue Watching row for a show with no announced episode total', () => {
	it('resolves into a resume control rather than the /search fallback', async () => {
		server.use(
			...homeHandlers(
				{ history: [{ ep_no: String(WATCHED), id: 'allanime-1', title: HISTORY_TITLE }] },
				[
					// The movie leads, as the text search often returns it.
					// The row can only resolve if the count filter drops it
					// AND keeps the countless main series.
					http.post(`${API_BASE}/api/kitsu/search`, () =>
						HttpResponse.json([movie(), longRunner()])
					),
					http.post(`${API_BASE}/api/availability`, () =>
						HttpResponse.json({ available: true, episode_count: 1150, approximate: false })
					)
				]
			)
		);

		app = mount(HomePage, { target });

		await until(() => resumeButton() !== null, 'the long-runner row to resolve into a button');
		expect(searchFallback()).toBeNull();
	});

	it('still falls through to /search when the only countless airing hit is a different show', async () => {
		server.use(
			...homeHandlers(
				{ history: [{ ep_no: String(WATCHED), id: 'allanime-1', title: HISTORY_TITLE }] },
				[
					// Same shape as the entry above — no total, broadcasting —
					// but an unrelated show. Accepting on airing status alone
					// would cache this id for a row the user then clicks.
					http.post(`${API_BASE}/api/kitsu/search`, () =>
						HttpResponse.json([
							{ ...kitsuRef('12', 'One Piece', 0), episode_count: null, status: 'current' }
						])
					)
				]
			)
		);

		app = mount(HomePage, { target });

		await until(() => searchFallback() !== null, 'the row to fall through to search');
		expect(resumeButton()).toBeNull();
	});
});
