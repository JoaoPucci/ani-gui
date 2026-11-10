// Acceptance: the show page's watch-order strip.
//
// A show that belongs to a run of seasons shows, under its masthead,
// what comes before it and what comes after, its place ("03 of 04"
// in the masthead eyebrow where the run is one line), and a card for
// the entry the viewer is part-way through elsewhere in the run. The
// order setting decides the neighbours. A franchise that cannot be
// read leaves the page as it was, with no strip and no error. Where
// the strip shows, the franchise it links to is left out of "Similar
// titles", and nothing else is.

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

type HistoryRowFixture = { ep_no: string; id: string; title: string; kitsu_id?: string };

interface ShowOptions {
	order?: string;
	settings?: () => Response;
	franchise?: () => Response;
	/** Null: the history read fails. */
	history?: HistoryRowFixture[] | null;
	/**
	 * The row the backend names for an id where it is not the row that
	 * recorded it (`history_claim::names`): a row whose recorded entry
	 * Kitsu deleted, named by its stored mapping or title match.
	 */
	named?: Record<string, HistoryRowFixture>;
	similar?: string[];
}

/** The page's reads, with the franchise answered by `franchise`. */
function useShow(opts: ShowOptions = {}) {
	const calls = { franchise: 0 };
	const similar = (opts.similar ?? []).map((id) => kitsuRef(id, `Similar ${id}`, 12));
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			opts.settings
				? opts.settings()
				: HttpResponse.json({ ...appConfig(), watch_order: opts.order ?? 'linked' })
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
		http.get(`${API_BASE}/api/history`, () =>
			opts.history === null
				? new HttpResponse(null, { status: 503 })
				: HttpResponse.json(opts.history ?? [])
		),
		// The rows the franchise's entries resume from, as the resume
		// lookup names them.
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			if (opts.history === null) return new HttpResponse(null, { status: 503 });
			const { ids } = (await request.json()) as { ids: string[] };
			const rows = opts.history ?? [];
			return HttpResponse.json(
				Object.fromEntries(
					ids.map((id) => [id, opts.named?.[id] ?? rows.find((r) => r.kitsu_id === id) ?? null])
				)
			);
		}),
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
/** What a screen reader reads for `el`: its text outside `aria-hidden` parts. */
function spoken(el: Element): string {
	const out: string[] = [];
	const walk = (n: Node) => {
		if (n instanceof Element && n.getAttribute('aria-hidden') === 'true') return;
		if (n.nodeType === Node.TEXT_NODE) out.push(n.textContent ?? '');
		n.childNodes.forEach(walk);
	};
	walk(el);
	return out.join(' ').replace(/\s+/g, ' ').trim();
}
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

	it("links to the whole franchise from the strip's footer", async () => {
		useShow();
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		const link = strip()!.querySelector(`a[href="/anime/${KITSU_ID}/franchise"]`);
		expect(link).not.toBeNull();
		expect(link!.textContent).toContain(m.watchorder_strip_franchise_stack_title());
		expect(link!.textContent).toContain(m.watchorder_strip_franchise_stack_main({ count: 4 }));
	});

	it('keeps the franchise link beside the card for the entry being watched', async () => {
		useShow({
			history: [{ id: 'hianime:saga-origins', title: 'Saga Origins', ep_no: '3', kitsu_id: '5' }]
		});
		app = mount(DetailPage, { target });
		await until(() => tile('5') !== null, 'the watching card');
		expect(strip()!.querySelector(`a[href="/anime/${KITSU_ID}/franchise"]`)).not.toBeNull();
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

	it("gives a row's progress to the entry the backend names it for, when Kitsu deleted the recorded one", async () => {
		// The row recorded entry 4, which Kitsu has since deleted; the
		// backend names it for Origins (5), the entry its Continue
		// Watching card was matched to, as the resume lookup does.
		const ROW = { id: 'hianime:saga-origins', title: 'Saga Origins', ep_no: '3', kitsu_id: '4' };
		useShow({ history: [ROW], named: { '5': ROW } });
		app = mount(DetailPage, { target });
		await until(() => tile('5') !== null, 'the watching card');
		expect(tile('5')!.textContent).toContain(m.watchorder_strip_watching_key());
		expect(tile('5')!.textContent).toContain(m.watchorder_meta_progress_open({ episode: 3 }));
	});

	it('leaves the whole franchise out of Similar titles, and only the franchise', async () => {
		// The strip links to the franchise page, which lists all of it:
		// Origins is in the run but not on the strip, and the film is
		// beside the run, yet neither is a similar title.
		const FILM = entry('60', 'Saga: The Movie', 'movie', '2015-01-01');
		useShow({
			similar: ['10', '50', '5', '60', '77'],
			franchise: () => HttpResponse.json({ ...FRANCHISE, films: [FILM] })
		});
		app = mount(DetailPage, { target });
		await until(() => strip() !== null && similarCard('77') !== null, 'the strip and Similar');
		expect(similarCard('10')).toBeNull();
		expect(similarCard('50')).toBeNull();
		expect(similarCard('5')).toBeNull();
		expect(similarCard('60')).toBeNull();
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

	it('names each tile and the watching card by the full title, once', async () => {
		useShow({
			history: [{ id: 'hianime:saga-origins', title: 'Saga Origins', ep_no: '3', kitsu_id: '5' }]
		});
		app = mount(DetailPage, { target });
		await until(() => tile('5') !== null, 'the watching card');
		expect(spoken(tile('50')!).startsWith('Saga: Final Arc')).toBe(true);
		expect(spoken(tile('50')!)).not.toMatch(/^Final Arc/);
		expect(spoken(tile('5')!)).toContain('Saga: Origins');
		expect(spoken(tile('5')!).split('Origins').length).toBe(2);
	});

	it('shows no Similar titles card that the strip then takes away', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		useShow({
			similar: ['50', '77'],
			franchise: () => HttpResponse.json(FRANCHISE)
		});
		server.use(
			http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, async () => {
				await held;
				return HttpResponse.json(FRANCHISE);
			})
		);
		app = mount(DetailPage, { target });
		let sawFifty = false;
		const deadline = Date.now() + 1500;
		while (Date.now() < deadline) {
			if (similarCard('50') !== null) sawFifty = true;
			await new Promise((r) => setTimeout(r, 10));
		}
		release();
		await until(() => strip() !== null && similarCard('77') !== null, 'the strip and Similar');
		expect(sawFifty).toBe(false);
		expect(similarCard('50')).toBeNull();
	});

	it('draws in link order when the settings cannot be read', async () => {
		useShow({ settings: () => new HttpResponse(null, { status: 503 }) });
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		expect(tile('50')).not.toBeNull();
		expect(eyebrow()).toContain(m.watchorder_strip_position({ position: '03', total: '04' }));
	});

	it('shows the strip without progress when the history cannot be read', async () => {
		useShow({ history: null });
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		expect(tile('10')).not.toBeNull();
		expect(strip()!.textContent).not.toContain(m.watchorder_strip_watching_key());
	});

	it('claims no place, total or end for a franchise the backend read only in part', async () => {
		// The walk stopped at its bound: Second Arc is the last entry it
		// reached, which says nothing about whether another follows.
		useShow({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, SECOND],
					links: [['10', '42']],
					complete: false
				})
		});
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		expect(tile('10')).not.toBeNull();
		const text = strip()!.textContent ?? '';
		// No number: Origins, the first entry, is missing from this read,
		// so Second Arc's "02" would be wrong (it is "03").
		expect(text).not.toMatch(/\b0\d\b/);
		expect(text).not.toContain(m.watchorder_strip_position_open({ position: '02' }));
		expect(text).not.toContain(m.watchorder_strip_last());
		expect(text).not.toContain(m.watchorder_strip_first());
		expect(eyebrow()).not.toMatch(/\b\d{2}\b/);
		// The way to the franchise page stays, without counts it may get wrong.
		expect(text).toContain(m.watchorder_strip_franchise_stack_title());
		expect(text).not.toContain(m.watchorder_strip_franchise_stack_main({ count: 2 }));
	});

	it("names a side story's parent stories, and nothing after it", async () => {
		useShow({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, FINAL],
					links: [['10', '50']],
					sides: [SECOND],
					parents: { [KITSU_ID]: ['10', '50'] }
				})
		});
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip');
		expect(tile('10')).not.toBeNull();
		expect(tile('50')).not.toBeNull();
		expect(strip()!.textContent).toContain(m.watchorder_strip_word_parents());
		expect(strip()!.textContent).toContain(m.watchorder_strip_last());
		expect(strip()!.textContent).not.toContain(m.watchorder_strip_join({ count: 2 }));
		expect(eyebrow()).not.toContain(m.watchorder_strip_position({ position: '03', total: '04' }));
	});

	it('keeps no strip or place from the show before, moving to one without a run', async () => {
		useShow();
		const OTHER = '99';
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/${OTHER}`, () =>
				HttpResponse.json({ ...kitsuRef(OTHER, 'Lone Film', 1), subtype: 'movie' })
			),
			http.get(`${API_BASE}/api/kitsu/franchise/${OTHER}`, () =>
				HttpResponse.json({ ...FRANCHISE, anchor: OTHER, entries: [], links: [] })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${OTHER}`, () =>
				HttpResponse.json({ aired: 1, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.get(`${API_BASE}/api/history/by-kitsu/${OTHER}`, () => HttpResponse.json(null))
		);
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, "the first show's strip");
		setParams({ id: OTHER });
		await until(() => title() === 'Lone Film', 'the second show');
		await new Promise((r) => setTimeout(r, 100));
		expect(strip()).toBeNull();
		expect(eyebrow()).not.toContain(m.watchorder_strip_position({ position: '03', total: '04' }));
	});

	it("shows no strip from a show's franchise that answers after leaving it", async () => {
		// From Second Arc to Final Arc, a member of the same run whose
		// own franchise read fails: Second Arc's answer, arriving late,
		// would place Final Arc at "04 of 04" if the page took it.
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let answered = false;
		useShow();
		const OTHER = '50';
		server.use(
			http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, async () => {
				await held;
				answered = true;
				return HttpResponse.json(FRANCHISE);
			}),
			http.get(`${API_BASE}/api/kitsu/anime/${OTHER}`, () =>
				HttpResponse.json({ ...kitsuRef(OTHER, 'Saga: Final Arc', 12), subtype: 'TV' })
			),
			http.get(
				`${API_BASE}/api/kitsu/franchise/${OTHER}`,
				() => new HttpResponse(null, { status: 503 })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${OTHER}`, () =>
				HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.get(`${API_BASE}/api/history/by-kitsu/${OTHER}`, () => HttpResponse.json(null))
		);
		app = mount(DetailPage, { target });
		await until(() => title() === 'Saga: Second Arc', 'the first show');
		setParams({ id: OTHER });
		await until(() => title() === 'Saga: Final Arc', 'the second show');
		release();
		await until(() => answered, "the first show's franchise answer");
		await new Promise((r) => setTimeout(r, 150));
		expect(strip()).toBeNull();
		expect(eyebrow()).not.toContain(m.watchorder_strip_position({ position: '04', total: '04' }));
	});
});
