// Acceptance: when Kitsu and the provider disagree about a recap, every
// whole episode keeps the tile, the label and the request it had.
//
// Kitsu numbers the episodes the page labels tiles with; the provider
// numbers what a request plays (an integer request plays the row the
// provider numbers so, a recap's row is tagged 7.5). The cases below
// cover the provider listing a recap Kitsu does not, Kitsu counting a
// recap as a regular episode the provider lists as 7.5, and Kitsu
// counting one the provider does not list at all. In each, the whole
// tiles, their Kitsu labels, the episode each one requests and the
// tile past the provider's count are what they were before fractional
// extras were read as themselves; only the recap's own tile is new.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

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

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

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

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Kitsu title ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

/** Kitsu lists `kitsu` episodes; the provider's highest whole episode
 *  is `cap` and it lists `extras` beside them. */
function useShow(kitsu: number, cap: number, extras: string[]) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, kitsu), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: kitsu, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(kitsu))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: cap,
				extra_episodes: extras,
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
}

const tileNumbers = () =>
	Array.from(target.querySelectorAll('li[data-ep-num]')).map((li) =>
		Number(li.getAttribute('data-ep-num'))
	);
const tileButton = (n: number) =>
	target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null;

/** Click tile `n` and return the episode its interactive stream asked for. */
async function requestedBy(n: number): Promise<string> {
	const before = FakeEventSource.instances.length;
	tileButton(n)!.click();
	await until(
		() =>
			FakeEventSource.instances
				.slice(before)
				.some((i) => !i.url.includes('prefetch=1') && i.url.includes('/api/play/stream')),
		`the stream tile ${n} asks for`
	);
	const s = FakeEventSource.instances
		.slice(before)
		.filter((i) => !i.url.includes('prefetch=1') && i.url.includes('/api/play/stream'))
		.pop()!;
	const ep = new URL(s.url).searchParams.get('episode') ?? '';
	s.dispatch('error', JSON.stringify({ kind: 'timeout' }));
	await until(() => !tileButton(n)!.closest('li')!.matches('.busy'), 'the click to settle');
	return ep;
}

async function dismissOverlay() {
	const close = Array.from(target.querySelectorAll('button')).find((b) =>
		/dismiss|close|ok/i.test(b.getAttribute('aria-label') ?? b.textContent ?? '')
	);
	close?.click();
	await new Promise((r) => setTimeout(r, 10));
}

const WHOLE = Array.from({ length: 12 }, (_, i) => i + 1);

describe('detail route — Kitsu and the provider disagree about a recap', () => {
	it('the provider lists a 7.5 Kitsu does not: whole tiles as before, plus the recap', async () => {
		useShow(12, 12, ['7.5']);
		app = mount(DetailPage, { target });
		await until(() => tileButton(7.5) !== null && tileButton(12) !== null, 'the grid');
		expect(tileNumbers()).toEqual([1, 2, 3, 4, 5, 6, 7, 7.5, 8, 9, 10, 11, 12]);
		expect(tileNumbers().filter(Number.isInteger)).toEqual(WHOLE);
		expect(target.textContent).toContain('Kitsu title 8');
		expect(await requestedBy(8)).toBe('8');
		await dismissOverlay();
		expect(await requestedBy(7.5)).toBe('7.5');
		expect(tileButton(12)!.closest('li')!.querySelector('.ep-tile-recheck')).toBeNull();
	});

	it('Kitsu counts the recap as episode 8, the provider lists it as 7.5: as before, plus the recap', async () => {
		// The provider's whole episodes stop at 11; Kitsu's run to 12.
		useShow(12, 11, ['7.5']);
		app = mount(DetailPage, { target });
		await until(() => tileButton(7.5) !== null && tileButton(12) !== null, 'the grid');
		expect(tileNumbers()).toEqual([1, 2, 3, 4, 5, 6, 7, 7.5, 8, 9, 10, 11, 12]);
		expect(target.textContent).toContain('Kitsu title 8');
		expect(await requestedBy(8)).toBe('8');
		await dismissOverlay();
		expect(await requestedBy(11)).toBe('11');
		// Past the provider's count, as before.
		expect(tileButton(12)!.matches('.ep-tile-recheck')).toBe(true);
	});

	it('Kitsu counts a recap the provider does not list: exactly as before', async () => {
		useShow(13, 12, []);
		app = mount(DetailPage, { target });
		await until(() => tileButton(12) !== null, 'the grid');
		expect(tileNumbers()).toEqual(WHOLE);
		expect(await requestedBy(8)).toBe('8');
	});
});
