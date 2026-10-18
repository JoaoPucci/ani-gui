// Acceptance: the extras a provider appends after a show's last
// episode — Noragami's two OADs, which hianime lists as rows 13 and 14
// of a twelve-episode show — are tiles of their own beside episode 12,
// named as extras, and request themselves by their own numbers.
// Whether they are extras is the backend's call (docs/title-resolution.md,
// "Episode numbers"); the page shows what availability answers.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';

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
const TITLE = 'Noragami';

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
const tile = (n: number) => target.querySelector(`li[data-ep-num="${n}"]`);
const tileButton = (n: number) =>
	(tile(n)?.querySelector('button') ?? null) as HTMLButtonElement | null;

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

const WHOLE = Array.from({ length: 12 }, (_, i) => i + 1);

describe('detail route — extras a provider appends after the last episode', () => {
	// Noragami: Kitsu counts twelve episodes; hianime lists its two OADs
	// after them, which the backend now answers as the extras 12.1 and
	// 12.2 beside episode 12, capped at twelve.
	it('shows the OADs as extras beside episode 12, never as episodes 13 and 14', async () => {
		useShow(12, 12, ['12.1', '12.2']);
		app = mount(DetailPage, { target });
		await until(() => tileButton(12.2) !== null, 'the grid');
		expect(tileNumbers()).toEqual([...WHOLE, 12.1, 12.2]);
		for (const n of WHOLE) {
			expect(tile(n)?.textContent, `tile ${n}`).toContain(`Kitsu title ${n}`);
		}
		expect(tile(12.1)?.textContent).toContain(
			m.detail_episode_extra_title_fallback({ num: '12.1' })
		);
		expect(tile(12.2)?.textContent).toContain(
			m.detail_episode_extra_title_fallback({ num: '12.2' })
		);
		expect(tile(12.1)?.textContent).not.toContain(m.detail_episode_title_fallback({ num: '13' }));
		expect(tileButton(13)).toBeNull();
		expect(await requestedBy(12.1)).toBe('12.1');
		expect(await requestedBy(12)).toBe('12');
	});
});
