// Acceptance: the rewired play-failure surfaces render the shared
// copy — including the one per-surface override.
//
// The unification retired the home and detail pages' inline mappers
// onto the shared one; these cases mount those routes, fail a real
// play through the typed SSE payload, and assert the rendered
// message, so a dropped call or a wrong option at either Svelte
// adapter cannot hide behind green unit tests.

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

import HomePage from '../../src/routes/+page.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

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
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
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

const UPSTREAM_503 = JSON.stringify({
	kind: 'upstream',
	status: 503,
	key: 'error.network.upstream'
});

describe('home — a resume failure renders the shared copy', () => {
	it('a provider 503 names the source as down', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/history`, () =>
				HttpResponse.json([{ ep_no: '3', id: 'provider-1', title: 'Cowboy Bebop' }])
			),
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([{ ...kitsuRef('1', 'Cowboy Bebop', 26), status: 'finished' }])
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 26,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
			http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(null)),
			http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([]))
		);

		app = mount(HomePage, { target });
		const resumeButton = () =>
			Array.from(target.querySelectorAll('button')).find((b) =>
				b.textContent?.includes('Cowboy Bebop')
			) ?? null;
		await until(() => resumeButton() !== null, 'the resolved Continue card');

		const streamsBefore = FakeEventSource.instances.length;
		resumeButton()!.click();
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the resume resolve stream'
		);
		FakeEventSource.instances[FakeEventSource.instances.length - 1].dispatch('error', UPSTREAM_503);
		await until(
			() => (target.textContent ?? '').includes(m.play_play_failure_source_down()),
			'the source-down copy on the resume failure overlay'
		);
	});
});

describe('detail — play failures render the shared copy with the override', () => {
	const KITSU_ID = '42';
	const TITLE = 'Ongoing Show';

	function useDetailHandlers() {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
				HttpResponse.json(
					Array.from({ length: 12 }, (_, i) => ({
						id: `ep-${i + 1}`,
						number: i + 1,
						relative_number: i + 1,
						canonical_title: `Episode ${i + 1}`,
						titles: {},
						synopsis: null,
						thumbnail: null,
						length_minutes: 24,
						air_date: '2019-01-01'
					}))
				)
			),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 12,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
		);
	}

	const tile = (n: number) =>
		(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;

	async function mountAndFailEpisode(episode: number, payload: string) {
		setParams({ id: KITSU_ID });
		app = mount(DetailPage, { target });
		await until(() => tile(episode) !== null, `the tile for episode ${episode}`);
		tile(episode)!.click();
		await until(
			() =>
				FakeEventSource.instances.some(
					(i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1')
				),
			"the click's interactive stream"
		);
		const interactive = FakeEventSource.instances
			.filter((i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1'))
			.pop()!;
		interactive.dispatch('error', payload);
	}

	it('a provider 503 names the source as down', async () => {
		useDetailHandlers();
		await mountAndFailEpisode(3, UPSTREAM_503);
		await until(
			() => (target.textContent ?? '').includes(m.play_play_failure_source_down()),
			'the source-down copy on the play failure overlay'
		);
	});

	it("a catalogue miss keeps this surface's definitive phrasing", async () => {
		// The one per-surface override the unification preserved: the
		// detail page states the miss definitively instead of the
		// other surfaces' hedge.
		useDetailHandlers();
		await mountAndFailEpisode(
			4,
			JSON.stringify({ kind: 'no_results', key: 'error.search.no_results' })
		);
		await until(
			() => (target.textContent ?? '').includes(m.detail_error_play_no_results()),
			'the definitive catalogue-miss copy'
		);
	});
});
