// Acceptance: the detail page's Play button over a row an earlier
// version numbered otherwise.
//
// A stitched recap an earlier version numbered "24.5" is "12.5" now. A
// history row from then reads "24.5" until a listing of the show puts
// its numbering right, and the page's own availability lookup pays for
// that listing. Once it has answered, Play goes on from the row as it
// reads now — 13, not 25 — and back to the recap when it was left
// part-way under the number it had then.

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
import { savePosition } from '../../src/lib/play/watch-position';
import { goto } from '$app/navigation';
import { forgetNavigationListeners, startNavigation } from './navigation';

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
	vi.mocked(goto).mockClear();
	forgetNavigationListeners();
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

function useShowHandlers(rowDelayMs = 0) {
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
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, async () => {
			// Read before the lookup put the row right, answered after it.
			const row = { id: ROW, ep_no: '24.5', title: TITLE };
			if (rowDelayMs > 0) await new Promise((r) => setTimeout(r, rowDelayMs));
			return HttpResponse.json(row);
		}),
		http.get(`${API_BASE}/api/history/:id/number`, () =>
			HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
		)
	);
}

/** The show with settings that land late, in dub: the page's first
 *  lookup goes out as sub, the second as dub. `reads` is what the row
 *  reads as after each, by how many lookups have answered. */
function useTwoLookups(reads: string[]) {
	let answered = 0;
	server.use(
		http.get(`${API_BASE}/api/settings`, async () => {
			await new Promise((r) => setTimeout(r, 300));
			return HttpResponse.json(appConfig('dub'));
		}),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			const body = (await request.json()) as { mode?: string };
			answered = body.mode === 'dub' ? 2 : Math.max(answered, 1);
			return HttpResponse.json({
				available: true,
				episode_count: 36,
				extra_episodes: ['12.5'],
				episode_count_approximate: false
			});
		}),
		http.get(`${API_BASE}/api/history/:id/number`, () =>
			HttpResponse.json({ ep_no: reads[answered] ?? reads.at(-1) })
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

	it('reads the row again when it arrives after the lookup has answered', async () => {
		useShowHandlers(300);
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '13' });
		await until(() => playButton(label) !== null, 'the Play button naming episode 13');
	});

	it('reads the row again when a later lookup answers', async () => {
		// Only the dub lookup, the second, puts the row right.
		useShowHandlers();
		useTwoLookups(['24.5', '24.5', '12.5']);
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '13' });
		await until(() => playButton(label) !== null, 'the Play button naming episode 13');
	});

	it('takes what each lookup leaves the row reading as, not only the first', async () => {
		// The sub lookup puts the row at 12.5; the dub lookup leaves it
		// at 13. Play goes on from the row as the last one left it.
		useShowHandlers();
		useTwoLookups(['24.5', '12.5', '13']);
		app = mount(DetailPage, { target });
		const label = m.detail_play_button_resume({ episode: '14' });
		await until(() => playButton(label) !== null, 'the Play button naming episode 14');
	});

	it('reads the row again after a warm of the show puts it right', async () => {
		// The lookup answers with a search-hit count: no listing behind
		// it. The page's own warm of the hero episode pays for one.
		let repaired = false;
		useShowHandlers();
		server.use(
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 36,
					extra_episodes: ['12.5'],
					episode_count_approximate: true
				})
			),
			http.get(`${API_BASE}/api/history/:id/number`, () =>
				HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
			)
		);
		app = mount(DetailPage, { target });
		await until(
			() => playButton(m.detail_play_button_resume({ episode: '25' })) !== null,
			'the Play button naming what the page read'
		);
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('prefetch=1')),
			'the hero warm'
		);
		repaired = true;
		for (const warm of FakeEventSource.instances.filter((i) => i.url.includes('prefetch=1'))) {
			warm.dispatch(
				'done',
				JSON.stringify({ session_id: 'w', media_url: 'x', media_kind: 'hls', episode: '25' })
			);
		}
		await until(
			() => playButton(m.detail_play_button_resume({ episode: '13' })) !== null,
			'the Play button naming episode 13'
		);
	});

	it('goes on from the row read again when Play is clicked while it is', async () => {
		// The lookup fails, so the refresh warms the row's own episode
		// first; Play is clicked while that warm is out.
		let repaired = false;
		useShowHandlers();
		server.use(
			http.post(`${API_BASE}/api/availability`, () => new HttpResponse(null, { status: 503 })),
			http.get(`${API_BASE}/api/history/:id/number`, () =>
				HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
			)
		);
		app = mount(DetailPage, { target });
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=24.5')),
			'the warm of the row'
		);
		playButton(m.detail_play_button_resume({ episode: '25' }))?.click();
		repaired = true;
		for (const warm of FakeEventSource.instances.filter((i) => i.url.includes('episode=24.5'))) {
			warm.dispatch(
				'done',
				JSON.stringify({ session_id: 'w', media_url: 'x', media_kind: 'hls', episode: '12.5' })
			);
		}
		await until(
			() =>
				FakeEventSource.instances.some(
					(i) => i.url.includes('episode=13') && !i.url.includes('prefetch=1')
				),
			'the click to resolve episode 13'
		);
		expect(
			FakeEventSource.instances.some(
				(i) => i.url.includes('episode=25') && !i.url.includes('prefetch=1')
			)
		).toBe(false);
	});

	it('reads a row put right before the visit a bounded number of times', async () => {
		// The stamp keeps naming its former number for good. Warms that
		// answer after the row was read must not read it again forever.
		let reads = 0;
		useShowHandlers();
		server.use(
			http.get(`${API_BASE}/api/history/:id/number`, () => {
				reads += 1;
				return HttpResponse.json({ ep_no: '12.5', former_ep_no: '24.5' });
			})
		);
		app = mount(DetailPage, { target });
		await until(
			() => playButton(m.detail_play_button_resume({ episode: '13' })) !== null,
			'the Play button naming episode 13'
		);
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('prefetch=1')),
			'the warms'
		);
		for (const warm of FakeEventSource.instances.filter((i) => i.url.includes('prefetch=1'))) {
			warm.dispatch(
				'done',
				JSON.stringify({ session_id: 'w', media_url: 'x', media_kind: 'hls', episode: '13' })
			);
		}
		await new Promise((r) => setTimeout(r, 500));
		expect(reads).toBeLessThan(10);
	});
});

// Play waits, busy, for the row being read again before it goes on.
// The page stays on screen until a navigation the viewer started
// lands; a click that came out of that wait and played would cancel
// it.
describe('detail route — a navigation started while Play waits for the row', () => {
	it('plays nothing and goes nowhere', async () => {
		let repaired = false;
		useShowHandlers();
		server.use(
			http.post(`${API_BASE}/api/availability`, () => new HttpResponse(null, { status: 503 })),
			http.get(`${API_BASE}/api/history/:id/number`, () =>
				HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
			),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
		);
		app = mount(DetailPage, { target });
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=24.5')),
			'the warm of the row'
		);
		playButton(m.detail_play_button_resume({ episode: '25' }))?.click();
		startNavigation();
		repaired = true;
		for (const warm of FakeEventSource.instances.filter((i) => i.url.includes('episode=24.5'))) {
			warm.dispatch(
				'done',
				JSON.stringify({ session_id: 'w', media_url: 'x', media_kind: 'hls', episode: '12.5' })
			);
		}
		await new Promise((r) => setTimeout(r, 300));

		const clicked = FakeEventSource.instances.filter(
			(i) => /episode=(13|25)\b/.test(i.url) && !i.url.includes('prefetch=1')
		);
		expect(clicked).toEqual([]);
		expect(goto).not.toHaveBeenCalled();
	});
});
