// Acceptance: the detail page's Play button over a row an earlier
// version numbered otherwise.
//
// A stitched recap an earlier version numbered "24.5" is "12.5" now. A
// history row from then reads "24.5" until a listing of the show puts
// its numbering right, and the page's own availability lookup pays for
// that listing. Once it has answered, Play goes on from the row as it
// reads now — 13, not 25 — and back to the recap when it was left
// part-way under the number it had then.

import { describe, it, vi, beforeEach, afterEach } from 'vitest';
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
import { savePosition } from '../../src/lib/play/watch-position';

const KITSU_ID = '42';
const TITLE = 'Stitched Show';
const ROW = 'hianime:stitched-show-4';

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

function useShowHandlers() {
	let repaired = false;
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 36), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 36, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () => {
			repaired = true;
			return HttpResponse.json({
				available: true,
				episode_count: 36,
				extra_episodes: ['12.5'],
				episode_count_approximate: false
			});
		}),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json({ id: ROW, ep_no: '24.5', title: TITLE })
		),
		http.get(`${API_BASE}/api/history/:id/number`, () =>
			HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
		)
	);
}

/** The page's primary action, found by what it says. */
const playButton = (label: string) =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(label)
	) ?? null;

describe('detail route — a row an earlier version numbered otherwise', () => {
	it('goes on from the episode the row reads as once the lookup has answered', async () => {
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '13' });
		await until(() => playButton(label) !== null, 'the Play button naming episode 13');
	});

	it('goes back to the recap left part-way under the number it had then', async () => {
		savePosition(KITSU_ID, 24.5, 600, 1420);
		useShowHandlers();
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '12.5' });
		await until(() => playButton(label) !== null, 'the Play button naming the recap');
	});
});
