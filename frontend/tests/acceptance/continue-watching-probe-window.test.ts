// Acceptance: what a Continue Watching row is while its Kitsu match is
// still resolving, and what it becomes afterwards.
//
// The row has three states, and the middle one exists only because of
// a bug. A row whose match had not landed used to render as the
// `/search` fallback, so clicking a card during the probe window threw
// the user into search instead of resuming (#112). It now renders as a
// non-interactive placeholder until the match is known, and only then
// resolves into a resume button — or, if resolution definitively
// failed, into the `/search` link.
//
// Which state is on screen at which moment is the whole behaviour, so
// it cannot be observed below the route.

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

const SHOW = 'Cowboy Bebop';
const HISTORY = [{ ep_no: '3', id: 'allanime-1', title: SHOW }];

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

/** Elements of the Continue row, one per template branch. */
const carrying = (el: Element | null) => el?.textContent?.includes(SHOW) ?? false;
/** Probe window: a non-interactive placeholder, marked busy. */
const loadingCard = () =>
	Array.from(target.querySelectorAll('[aria-busy="true"]')).find(carrying) ?? null;
/** Resolved: the resume button. */
const resumeButton = () => Array.from(target.querySelectorAll('button')).find(carrying) ?? null;
/** Resolution failed: falls through to search. */
const searchFallback = () =>
	Array.from(target.querySelectorAll('a[href*="search"]')).find(carrying) ?? null;

describe('Continue Watching row during the probe window', () => {
	it('is a non-interactive placeholder, not a /search link, until the match lands', async () => {
		let releaseMatch: () => void = () => {};
		const matchHeld = new Promise<void>((resolve) => {
			releaseMatch = resolve;
		});

		server.use(
			...homeHandlers({ history: HISTORY }, [
				// Held open, so the row sits in the probe window for as
				// long as the assertions need rather than for as long as
				// the worker happens to take.
				http.post(`${API_BASE}/api/kitsu/search`, async () => {
					await matchHeld;
					return HttpResponse.json([kitsuRef('1', SHOW, 26)]);
				})
			])
		);

		app = mount(HomePage, { target });

		// The row is on screen AND still unresolved: the placeholder is
		// what proves both at once, where waiting on the title alone
		// would also match the resolved and failed branches.
		await until(() => loadingCard() !== null, 'the row to render its loading placeholder');
		expect(searchFallback()).toBeNull();
		expect(resumeButton()).toBeNull();

		releaseMatch();

		await until(() => resumeButton() !== null, 'the row to resolve into a resume button');
		expect(searchFallback()).toBeNull();
		expect(loadingCard()).toBeNull();
	});

	it('falls through to /search once resolution has definitively failed', async () => {
		// The default search handler answers with no hits, so the row's
		// match resolves to null rather than staying unknown.
		server.use(...homeHandlers({ history: HISTORY }));

		app = mount(HomePage, { target });

		await until(() => searchFallback() !== null, 'the unresolvable row to offer search');
		expect(resumeButton()).toBeNull();
	});

	it("reserves the resolved card's shape instead of growing into it", async () => {
		// The rail resolves row by row under a bounded pool, so the
		// cards land at different moments. If the placeholder is
		// smaller than what replaces it, every landing nudges the row —
		// and the user watches it settle for as long as the slowest
		// probe takes.
		//
		// The body is a grid, so what decides the card's height is how
		// many rows it has. Counting them is the size question without
		// a layout engine to ask, which happy-dom does not provide.
		let releaseMatch: () => void = () => {};
		const matchHeld = new Promise<void>((resolve) => {
			releaseMatch = resolve;
		});

		server.use(
			...homeHandlers({ history: HISTORY }, [
				http.post(`${API_BASE}/api/kitsu/search`, async () => {
					await matchHeld;
					return HttpResponse.json([kitsuRef('1', SHOW, 26)]);
				})
			])
		);

		app = mount(HomePage, { target });

		await until(() => loadingCard() !== null, 'the row to render its placeholder');
		const whileLoading = loadingCard()!.querySelector('.resume-body')!.childElementCount;

		releaseMatch();
		await until(() => resumeButton() !== null, 'the row to resolve');
		const whenResolved = resumeButton()!.querySelector('.resume-body')!.childElementCount;

		expect(whileLoading).toBe(whenResolved);
	});
});
