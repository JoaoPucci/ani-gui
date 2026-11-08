// Acceptance: the show page's watch-order strip.
//
// A show that belongs to a run of seasons shows, under its masthead,
// what comes before it and what comes after, its place ("03 of 04"
// in the masthead eyebrow where the run is one line), and a card for
// the entry the viewer is part-way through elsewhere in the run. The
// order setting decides the neighbours. A franchise that cannot be
// read leaves the page as it was, with no strip and no error. Entries
// the strip shows are left out of "Similar titles", and only those.

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

const KITSU_ID = '42';

const entry = (id: string, title: string, subtype: string, start: string) => ({
	id,
	title,
	subtype,
	start,
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});

// Link order: Origins → Saga → Second Arc → Final Arc. Origins is a
// prequel story released after Second Arc, so release order puts it
// between Second Arc and Final Arc.
const ORIGINS = entry('5', 'Saga: Origins', 'OVA', '2013-04-01');
const SAGA = entry('10', 'Saga', 'TV', '2010-01-01');
const SECOND = entry('42', 'Saga: Second Arc', 'TV', '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', 'TV', '2014-01-01');

const FRANCHISE = {
	anchor: KITSU_ID,
	entries: [SAGA, SECOND, ORIGINS, FINAL],
	links: [
		['10', '42'],
		['42', '50'],
		['5', '10']
	],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
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
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

interface ShowOptions {
	order?: string;
	franchise?: () => Response;
	history?: { ep_no: string; id: string; title: string; kitsu_id?: string }[];
	similar?: string[];
}

/** The page's reads, with the franchise answered by `franchise`. */
function useShow(opts: ShowOptions = {}) {
	const calls = { franchise: 0 };
	const similar = (opts.similar ?? []).map((id) => kitsuRef(id, `Similar ${id}`, 12));
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), watch_order: opts.order ?? 'linked' })
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Saga: Second Arc', 12), subtype: 'TV' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, () => {
			calls.franchise++;
			return opts.franchise ? opts.franchise() : HttpResponse.json(FRANCHISE);
		}),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json(similar)),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json({ cached: Object.fromEntries(similar.map((s) => [s.id, true])) })
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json(opts.history ?? [])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	return calls;
}

/** The strip: the section headed "Watch order". */
const strip = (): HTMLElement | null =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2 .eyebrow-key')?.textContent === m.watchorder_strip_eyebrow()
	) ?? null;
const tile = (id: string) => strip()?.querySelector(`a[href="/anime/${id}"]`) ?? null;
const eyebrow = () => target.querySelector('.masthead-text .eyebrow')?.textContent ?? '';
const title = () => target.querySelector('h1.title')?.textContent ?? '';
const similarCard = (id: string) =>
	target.querySelector(`section.similar a[href="/anime/${id}"]`) ?? null;

describe('detail route — the watch-order strip', () => {
	it('shows what comes before and after, and the place in the run', async () => {
		useShow();
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		expect(tile('10')).not.toBeNull();
		expect(tile('50')).not.toBeNull();
		// Short titles on the tiles, the full title as the tooltip.
		expect(tile('50')!.textContent).toContain('Final Arc');
		expect(tile('50')!.getAttribute('title')).toBe('Saga: Final Arc');
		expect(strip()!.textContent).toContain(m.watchorder_strip_word_prequel());
		expect(strip()!.textContent).toContain(m.watchorder_strip_word_sequel());
		// The page's own entry is marked, not linked.
		expect(tile(KITSU_ID)).toBeNull();
		expect(eyebrow()).toContain(m.watchorder_strip_position({ position: '03', total: '04' }));
	});

	it('shows no strip and no error when the franchise cannot be read', async () => {
		const calls = useShow({ franchise: () => new HttpResponse(null, { status: 503 }) });
		app = mount(DetailPage, { target });
		await until(() => title() !== '' && calls.franchise > 0, 'the page and the franchise read');
		await new Promise((r) => setTimeout(r, 100));
		expect(strip()).toBeNull();
		expect(eyebrow()).not.toContain(m.watchorder_strip_position({ position: '03', total: '04' }));
		expect(target.querySelector('[role="alert"]')).toBeNull();
		expect(title()).toBe('Saga: Second Arc');
	});

	it('takes its neighbours and its place from the order setting', async () => {
		useShow({ order: 'released' });
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		// By release date, Origins (2013) follows Second Arc (2012).
		expect(tile('10')).not.toBeNull();
		expect(tile('5')).not.toBeNull();
		expect(tile('50')).toBeNull();
		expect(strip()!.textContent).toContain(m.watchorder_strip_word_released_after());
		expect(eyebrow()).toContain(m.watchorder_strip_position({ position: '02', total: '04' }));
	});

	it('names the entry being watched elsewhere in the run, as a link to its page', async () => {
		useShow({
			history: [{ id: 'hianime:saga-origins', title: 'Saga Origins', ep_no: '3', kitsu_id: '5' }]
		});
		app = mount(DetailPage, { target });
		await until(() => tile('5') !== null, 'the watching card');
		expect(tile('5')!.textContent).toContain(m.watchorder_strip_watching_key());
		expect(tile('5')!.textContent).toContain(m.watchorder_meta_progress_open({ episode: 3 }));
	});

	it('leaves the entries the strip shows out of Similar titles, and only those', async () => {
		useShow({ similar: ['10', '50', '5', '77'] });
		app = mount(DetailPage, { target });
		await until(() => strip() !== null && similarCard('77') !== null, 'the strip and Similar');
		expect(similarCard('10')).toBeNull();
		expect(similarCard('50')).toBeNull();
		// Origins is in the run but not on the strip: it stays.
		expect(similarCard('5')).not.toBeNull();
	});

	it("leaves the watching card's entry out of Similar titles too", async () => {
		useShow({
			similar: ['5', '77'],
			history: [{ id: 'hianime:saga-origins', title: 'Saga Origins', ep_no: '3', kitsu_id: '5' }]
		});
		app = mount(DetailPage, { target });
		await until(() => tile('5') !== null && similarCard('77') !== null, 'the card and Similar');
		expect(similarCard('5')).toBeNull();
	});
});
