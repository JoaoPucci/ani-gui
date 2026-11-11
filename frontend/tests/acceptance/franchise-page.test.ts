// Acceptance: the franchise page, /anime/<id>/franchise.
//
// The page lists a whole franchise around the show it was opened
// from: a masthead named after the franchise's first TV series, with the
// number of titles it lists and a Resume button for the title being
// watched (or a way to start from the beginning), then the main story
// as a numbered list in the order the setting chooses, the show it was
// opened from marked, and the other versions, films and side stories
// on rails, the recaps too. A franchise that cannot be read
// says so and offers to try again.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { mount, unmount } from 'svelte';
import { get } from 'svelte/store';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
import { setLocale } from '../../src/lib/paraglide/runtime';

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

import { watchPageFetches } from './page-fetches';
import FranchisePage from '../../src/routes/anime/[id]/franchise/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { breadcrumb } from '../../src/lib/breadcrumb';

const KITSU_ID = '42';

const entry = (id: string, title: string, subtype: string, start: string, episodes = 12) => ({
	id,
	title,
	subtype,
	start,
	episodes,
	status: 'finished',
	poster: null,
	cover: null
});

// Link order: Origins → Saga → Second Arc → Final Arc. Origins came
// out after Second Arc, so release order puts it third.
const ORIGINS = entry('5', 'Saga: Origins (OVA)', 'OVA', '2013-04-01', 2);
const SAGA = entry('10', 'Saga', 'TV', '2010-01-01');
const SECOND = entry('42', 'Saga: Second Arc', 'TV', '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', 'TV', '2014-01-01');
const REMAKE = entry('70', 'Saga (2020)', 'TV', '2020-01-01', 24);
const FILM = entry('60', 'Saga: The Movie', 'movie', '2015-01-01', 1);
const SIDE = entry('80', 'Saga: Beach Episode', 'special', '2012-08-01', 1);
const RECAP = entry('90', 'Saga: Recap', 'special', '2013-01-01', 1);

const FRANCHISE = {
	anchor: KITSU_ID,
	entries: [ORIGINS, SAGA, SECOND, FINAL],
	links: [
		['10', '42'],
		['42', '50'],
		['5', '10']
	],
	versions: [REMAKE],
	films: [FILM],
	sides: [SIDE],
	recaps: [RECAP],
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
	setLocale('en', { reload: false });
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

interface Options {
	order?: string;
	franchise?: () => Response | Promise<Response>;
	history?: HistoryRowFixture[];
	/**
	 * The row the backend names for an id where it is not the row that
	 * recorded it (`history_claim::names`): a row whose recorded entry
	 * Kitsu deleted, named by its stored mapping or title match.
	 */
	named?: Record<string, HistoryRowFixture>;
}

function useFranchise(opts: Options = {}) {
	const calls = { franchise: 0 };
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), watch_order: opts.order ?? 'linked' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, () => {
			calls.franchise++;
			return opts.franchise ? opts.franchise() : HttpResponse.json(FRANCHISE);
		}),
		// The rows the franchise's entries resume from, as the resume
		// lookup names them.
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			const rows = opts.history ?? [];
			return HttpResponse.json(
				Object.fromEntries(
					ids.map((k) => [k, opts.named?.[k] ?? rows.find((r) => r.kitsu_id === k) ?? null])
				)
			);
		})
	);
	return calls;
}

const title = () => target.querySelector('h1')?.textContent?.trim() ?? '';
const mainStory = () =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2')?.textContent?.trim() === m.watchorder_franchise_main()
	) ?? null;
const rows = () => [...(mainStory()?.querySelectorAll('li a') ?? [])] as HTMLAnchorElement[];
const row = (id: string) => rows().find((a) => a.getAttribute('href') === `/anime/${id}`) ?? null;
const rail = (eyebrow: string) =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2 .eyebrow-key')?.textContent?.trim() === eyebrow
	) ?? null;
const button = (text: string) =>
	[...target.querySelectorAll('a, button')].find((b) => b.textContent?.trim() === text) as
		| HTMLElement
		| undefined;
const resumeLink = () =>
	[...target.querySelectorAll('a')].find((a) =>
		[m.watchorder_franchise_resume(), m.watchorder_franchise_start()].some(
			(t) => t === a.textContent?.trim()
		)
	) ?? null;

describe('franchise page', () => {
	it('names the franchise after its first TV series and counts what it lists', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => title() !== '', 'the masthead');
		// The first title in link order is Origins, the prequel OVA; the
		// first TV series after it is Saga.
		expect(title()).toBe('Saga');
		expect(target.textContent).toContain(m.watchorder_franchise_eyebrow());
		expect(target.textContent).toContain(m.watchorder_franchise_count({ count: 8 }));
	});

	it('lists the main story in link order, numbered, with the show it came from marked', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(rows().map((a) => a.getAttribute('href'))).toEqual([
			'/anime/5',
			'/anime/10',
			'/anime/42',
			'/anime/50'
		]);
		expect(row('5')!.textContent).toContain('01');
		expect(row('50')!.textContent).toContain('04');
		// The row links to the show's page, not to this one: it marks
		// the current item, not the current page.
		expect(row('42')!.getAttribute('aria-current')).toBe('true');
		expect(row('42')!.textContent).toContain(m.watchorder_franchise_here());
		expect(row('10')!.getAttribute('aria-current')).toBeNull();
		// Each row by the title the rest of the app shows for it: the
		// words it shares with the others are not cut.
		expect(row('50')!.querySelector('.row-title')?.textContent).toBe('Saga: Final Arc');
		expect(row('50')!.getAttribute('title')).toBe('Saga: Final Arc');
		expect(target.textContent).toContain(m.watchorder_franchise_order_note_linked());
		expect(target.querySelector('a[href="/settings"]')).not.toBeNull();
	});

	it('follows the order setting', async () => {
		useFranchise({ order: 'released' });
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(rows().map((a) => a.getAttribute('href'))).toEqual([
			'/anime/10',
			'/anime/42',
			'/anime/5',
			'/anime/50'
		]);
		expect(row('5')!.textContent).toContain('03');
		expect(target.textContent).toContain(m.watchorder_franchise_order_note_released());
	});

	it('resumes the title being watched, and says where it is', async () => {
		useFranchise({
			history: [
				{ id: 'hianime:saga', title: 'Saga', ep_no: '12', kitsu_id: '10' },
				{ id: 'hianime:saga-final', title: 'Saga Final', ep_no: '3', kitsu_id: '50' }
			]
		});
		app = mount(FranchisePage, { target });
		await until(() => resumeLink() !== null && rows().length === 4, 'the masthead and list');
		expect(resumeLink()!.textContent?.trim()).toBe(m.watchorder_franchise_resume());
		expect(resumeLink()!.getAttribute('href')).toBe('/anime/50');
		const masthead = resumeLink()!.parentElement!;
		expect(masthead.textContent).toContain('04');
		expect(masthead.querySelector('.caption-title')?.textContent?.trim()).toBe('Saga: Final Arc');
		expect(masthead.textContent).toContain(m.watchorder_meta_progress_open({ episode: 3 }));
		// Saga is watched to the end, and its row says so.
		expect(row('10')!.textContent).toContain(m.watchorder_meta_watched());
	});

	it('resumes the title a row is named for when Kitsu deleted the entry the row recorded', async () => {
		// The row recorded entry 4, which Kitsu has since deleted; the
		// backend names it for Final Arc (50), the entry its Continue
		// Watching card was matched to, as the resume lookup does.
		const ROW = { id: 'hianime:saga-final', title: 'Saga Final', ep_no: '3', kitsu_id: '4' };
		useFranchise({ history: [ROW], named: { '50': ROW } });
		app = mount(FranchisePage, { target });
		await until(() => resumeLink() !== null && rows().length === 4, 'the masthead and list');
		expect(resumeLink()!.textContent?.trim()).toBe(m.watchorder_franchise_resume());
		expect(resumeLink()!.getAttribute('href')).toBe('/anime/50');
		expect(resumeLink()!.parentElement!.textContent).toContain(
			m.watchorder_meta_progress_open({ episode: 3 })
		);
	});

	it('offers to start from the first title when nothing is part-way', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => resumeLink() !== null, 'the masthead');
		expect(resumeLink()!.textContent?.trim()).toBe(m.watchorder_franchise_start());
		expect(resumeLink()!.getAttribute('href')).toBe('/anime/5');
	});

	it('puts other versions, films, side stories and recaps on rails', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(
			rail(m.watchorder_franchise_versions())?.querySelector('a[href="/anime/70"]')
		).not.toBeNull();
		expect(
			rail(m.watchorder_franchise_films())?.querySelector('a[href="/anime/60"]')
		).not.toBeNull();
		expect(
			rail(m.watchorder_franchise_sides())?.querySelector('a[href="/anime/80"]')
		).not.toBeNull();
		// Recaps show like every other section: nothing to open first.
		expect(
			rail(m.watchorder_franchise_recaps())?.querySelector('a[href="/anime/90"]')
		).not.toBeNull();
		expect(
			rail(m.watchorder_franchise_recaps())!.querySelector('button[aria-expanded]')
		).toBeNull();
	});

	it('leaves out the rails the franchise has nothing for', async () => {
		useFranchise({
			franchise: () =>
				HttpResponse.json({ ...FRANCHISE, versions: [], films: [], sides: [], recaps: [] })
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(rail(m.watchorder_franchise_versions())).toBeNull();
		expect(rail(m.watchorder_franchise_films())).toBeNull();
		expect(rail(m.watchorder_franchise_sides())).toBeNull();
		expect(rail(m.watchorder_franchise_recaps())).toBeNull();
		expect(target.textContent).toContain(m.watchorder_franchise_count({ count: 4 }));
	});

	it('sets the ways to continue side by side where the story branches', async () => {
		const ALT = entry('51', 'Saga: Final Arc Alternate', 'TV', '2014-06-01');
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [...FRANCHISE.entries, ALT],
					links: [...FRANCHISE.links, ['42', '51']]
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 5, 'the main story');
		const text = mainStory()!.textContent ?? '';
		expect(text).toContain(m.watchorder_strip_ways({ count: 2 }));
		// One column per way, under the one heading: no column of its own
		// says which way it is.
		const branches = [...mainStory()!.querySelectorAll('.branch')];
		expect(branches).toHaveLength(2);
		expect(branches.map((b) => b.querySelectorAll('li').length)).toEqual([1, 1]);
		expect(mainStory()!.querySelector('.branch-key')).toBeNull();
		expect(text).not.toMatch(/Way \d+ of \d+/);
	});

	it('says when there may be more titles than it shows', async () => {
		useFranchise({ franchise: () => HttpResponse.json({ ...FRANCHISE, complete: false }) });
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(target.textContent).toContain(m.watchorder_franchise_partial());
	});

	it('claims no places, total or beginning when there may be more titles', async () => {
		// The read may have missed titles before the first one it lists.
		useFranchise({ franchise: () => HttpResponse.json({ ...FRANCHISE, complete: false }) });
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		const text = target.textContent ?? '';
		expect(text).not.toMatch(/\b0\d\b/);
		expect(text).not.toContain(m.watchorder_franchise_count({ count: 4 }));
		expect(text).not.toContain(m.watchorder_franchise_start());
	});

	it('still resumes the title being watched when there may be more titles, with no place', async () => {
		useFranchise({
			franchise: () => HttpResponse.json({ ...FRANCHISE, complete: false }),
			history: [{ id: 'hianime:saga-final', title: 'Saga Final', ep_no: '3', kitsu_id: '50' }]
		});
		app = mount(FranchisePage, { target });
		await until(() => resumeLink() !== null && rows().length === 4, 'the masthead and list');
		expect(resumeLink()!.textContent?.trim()).toBe(m.watchorder_franchise_resume());
		expect(resumeLink()!.getAttribute('href')).toBe('/anime/50');
		expect(resumeLink()!.parentElement!.textContent).not.toMatch(/\b0\d\b/);
	});

	it('heads ways on without a count when there may be more titles', async () => {
		const ALT = entry('51', 'Saga: Final Arc Alternate', 'TV', '2014-06-01');
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [...FRANCHISE.entries, ALT],
					links: [...FRANCHISE.links, ['42', '51']],
					complete: false
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 5, 'the main story');
		const text = mainStory()!.textContent ?? '';
		expect(text).toContain(m.watchorder_upnext_choices());
		expect(text).not.toContain(m.watchorder_strip_ways({ count: 2 }));
	});

	it('counts the titles of a loop when the franchise was read whole', async () => {
		// Saga, Second Arc and Final Arc each list the next as a sequel,
		// and Final Arc lists Saga: a loop with no clear order.
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, SECOND, FINAL],
					links: [
						['10', '42'],
						['42', '50'],
						['50', '10']
					]
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 3, 'the main story');
		expect(mainStory()!.textContent).toContain(m.watchorder_franchise_loop_group({ count: 3 }));
	});

	it('heads a loop without a count when there may be more titles', async () => {
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, SECOND, FINAL],
					links: [
						['10', '42'],
						['42', '50'],
						['50', '10']
					],
					complete: false
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 3, 'the main story');
		const text = mainStory()!.textContent ?? '';
		expect(text).toContain(m.watchorder_franchise_loop_group_partial());
		expect(text).not.toContain(m.watchorder_franchise_loop_group({ count: 3 }));
	});

	it('counts the recaps only when the franchise was read whole', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => rail(m.watchorder_franchise_recaps()) !== null, 'the recaps');
		expect(target.querySelector('.recaps .eyebrow-value')?.textContent?.trim()).toBe('1');
	});

	it('gives the recaps no count when there may be more titles', async () => {
		useFranchise({ franchise: () => HttpResponse.json({ ...FRANCHISE, complete: false }) });
		app = mount(FranchisePage, { target });
		await until(() => rail(m.watchorder_franchise_recaps()) !== null, 'the recaps');
		expect(target.querySelector('.recaps h2')?.textContent?.trim()).toBe(
			m.watchorder_franchise_recaps()
		);
	});

	it('says the franchise could not be loaded, and loads it on Try again', async () => {
		let fail = true;
		const calls = useFranchise({
			franchise: () =>
				fail ? new HttpResponse(null, { status: 503 }) : HttpResponse.json(FRANCHISE)
		});
		app = mount(FranchisePage, { target });
		await until(
			() => target.textContent?.includes(m.watchorder_franchise_failed()) ?? false,
			'the failure'
		);
		expect(rows()).toHaveLength(0);
		fail = false;
		button(m.watchorder_franchise_retry())!.click();
		await until(() => rows().length === 4, 'the main story');
		expect(calls.franchise).toBe(2);
		expect(target.textContent).not.toContain(m.watchorder_franchise_failed());
	});

	it('shows the franchise of the show it moves to, not the one before', async () => {
		useFranchise();
		const OTHER = '50';
		server.use(
			http.get(`${API_BASE}/api/kitsu/franchise/${OTHER}`, () =>
				HttpResponse.json({ ...FRANCHISE, anchor: OTHER })
			)
		);
		app = mount(FranchisePage, { target });
		await until(() => row('42')?.getAttribute('aria-current') === 'true', 'the first page');
		setParams({ id: OTHER });
		await until(() => row('50')?.getAttribute('aria-current') === 'true', 'the second page');
		expect(row('42')!.getAttribute('aria-current')).toBeNull();
	});

	it('names the show it was opened from in the topbar, between Home and Franchise', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 4, 'the main story');
		expect(get(breadcrumb)).toEqual([
			{ label: m.breadcrumb_home(), href: '/' },
			{ label: 'Saga: Second Arc', href: `/anime/${KITSU_ID}` },
			{ label: m.watchorder_franchise_eyebrow() }
		]);
	});

	it('says the show has nothing around it when the franchise is the show alone', async () => {
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SECOND],
					links: [],
					versions: [],
					films: [],
					sides: [],
					recaps: []
				})
		});
		app = mount(FranchisePage, { target });
		await until(
			() => target.textContent?.includes(m.watchorder_franchise_none()) ?? false,
			'the lone show'
		);
		expect(rows()).toHaveLength(0);
		expect(target.textContent).not.toContain(m.watchorder_franchise_failed());
		expect(button(m.watchorder_franchise_retry())).toBeUndefined();
	});

	it('does not claim the show is alone when the answer lists titles around it', async () => {
		// An answer with no main story at all cannot be drawn: it is a
		// read that went wrong, not a show with nothing around it.
		useFranchise({ franchise: () => HttpResponse.json({ ...FRANCHISE, entries: [], links: [] }) });
		app = mount(FranchisePage, { target });
		await until(
			() => target.textContent?.includes(m.watchorder_franchise_failed()) ?? false,
			'the unreadable answer'
		);
		expect(target.textContent).not.toContain(m.watchorder_franchise_none());
	});

	it('keeps the latest read when an older one for the same show answers after it', async () => {
		// To Final Arc and back: the first read for Second Arc fails, late.
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let calls = 0;
		let lateAnswered = false;
		useFranchise();
		const OTHER = '50';
		server.use(
			http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, async () => {
				calls++;
				if (calls === 1) {
					await held;
					lateAnswered = true;
					return new HttpResponse(null, { status: 503 });
				}
				return HttpResponse.json(FRANCHISE);
			}),
			http.get(`${API_BASE}/api/kitsu/franchise/${OTHER}`, () =>
				HttpResponse.json({ ...FRANCHISE, anchor: OTHER })
			)
		);
		app = mount(FranchisePage, { target });
		await until(() => calls === 1, 'the first read');
		setParams({ id: OTHER });
		await until(() => row('50')?.getAttribute('aria-current') === 'true', 'the other show');
		setParams({ id: KITSU_ID });
		await until(() => row('42')?.getAttribute('aria-current') === 'true', 'the show again');
		release();
		await until(() => lateAnswered, 'the late answer');
		await new Promise((r) => setTimeout(r, 100));
		expect(target.textContent).not.toContain(m.watchorder_franchise_failed());
		expect(row('42')!.getAttribute('aria-current')).toBe('true');
	});

	it('closes the main story with the titles that have no clear place', async () => {
		// Saga → Second Arc → Final Arc, and Origins → Side Arc → Final
		// Arc: Side Arc has no number, and Origins only leads into it.
		const SIDE_ARC = entry('45', 'Saga: Side Arc', 'TV', '2013-06-01');
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, SECOND, ORIGINS, SIDE_ARC, FINAL],
					links: [
						['10', '42'],
						['42', '50'],
						['5', '45'],
						['45', '50']
					]
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 5, 'the main story');
		const story = mainStory()!;
		const heading = [...story.querySelectorAll('p')].find(
			(p) => p.textContent?.trim() === m.watchorder_franchise_unplaced()
		)!;
		expect(heading).toBeDefined();
		// Nothing numbered comes after the heading: only its own list.
		const after = [...story.querySelectorAll('li a')].filter(
			(a) => heading.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_FOLLOWING
		);
		expect(after.map((a) => a.getAttribute('href'))).toEqual(['/anime/5', '/anime/45']);
		expect(row('50')!.textContent).toContain('03');
	});

	it('keeps the titles with no clear place apart when there may be more titles', async () => {
		const SIDE_ARC = entry('45', 'Saga: Side Arc', 'TV', '2013-06-01');
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [SAGA, SECOND, ORIGINS, SIDE_ARC, FINAL],
					links: [
						['10', '42'],
						['42', '50'],
						['5', '45'],
						['45', '50']
					],
					complete: false
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 5, 'the main story');
		const story = mainStory()!;
		// Origins and Side Arc lead into Final Arc: under their own
		// heading, they never read as coming after it.
		const heading = [...story.querySelectorAll('p')].find(
			(p) => p.textContent?.trim() === m.watchorder_franchise_unplaced()
		);
		expect(heading).toBeDefined();
		const after = [...story.querySelectorAll('li a')].filter(
			(a) => heading!.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_FOLLOWING
		);
		expect(after.map((a) => a.getAttribute('href'))).toEqual(['/anime/5', '/anime/45']);
	});

	it('draws the page when the one title it lists is another show', async () => {
		useFranchise({
			franchise: () =>
				HttpResponse.json({
					...FRANCHISE,
					entries: [FINAL],
					links: [],
					versions: [],
					films: [],
					sides: [],
					recaps: []
				})
		});
		app = mount(FranchisePage, { target });
		await until(() => rows().length === 1, 'the one title');
		expect(target.textContent).not.toContain(m.watchorder_franchise_none());
	});
});

// Progress on the rows: a caption in the row's right column, with a
// line under it only where the line measures something. A finished
// title says so in words, with no line; a title with no known total
// gives its episode alone. The meta line keeps the episode count.
describe('franchise page progress', () => {
	// Saga watched to the end, Second Arc part-way, Final Arc still
	// airing with no known total, Origins not started.
	const AIRING = { ...FINAL, episodes: null, status: 'current' };
	const progressFranchise = () =>
		useFranchise({
			franchise: () =>
				HttpResponse.json({ ...FRANCHISE, entries: [ORIGINS, SAGA, SECOND, AIRING] }),
			history: [
				{ id: 'hianime:saga', title: 'Saga', ep_no: '12', kitsu_id: '10' },
				{ id: 'hianime:saga-2', title: 'Saga 2', ep_no: '6', kitsu_id: '42' },
				{ id: 'hianime:saga-final', title: 'Saga Final', ep_no: '5', kitsu_id: '50' }
			]
		});
	const line = (a: HTMLElement) => a.querySelector('.row-line:not(.slot)');
	const count = (text: string, part: string) => text.split(part).length - 1;

	for (const locale of ['en', 'pt-BR'] as const) {
		it(`captions each row by how far it is watched, in ${locale}`, async () => {
			setLocale(locale, { reload: false });
			progressFranchise();
			app = mount(FranchisePage, { target });
			await until(
				() => rows().length === 4 && (row('42')?.textContent ?? '').includes('6'),
				'the main story with its progress'
			);

			const partway = row('42')!;
			const caption = m.watchorder_meta_progress({ episode: 6, total: 12 });
			// Said once, in the right column, not again in the meta line.
			expect(count(partway.textContent!, caption)).toBe(1);
			expect(partway.querySelector('.row-caption')!.textContent).toBe(caption);
			expect(partway.querySelector('.meta')!.textContent).not.toContain(caption);
			const fill = line(partway)!.querySelector('.row-line-fill') as HTMLElement;
			expect(fill.style.inlineSize).toBe('50%');
			expect(line(partway)!.getAttribute('aria-hidden')).toBe('true');

			const watched = row('10')!;
			expect(watched.querySelector('.row-caption')!.textContent).toBe(m.watchorder_meta_watched());
			expect(watched.querySelector('.row-caption svg')!.getAttribute('aria-hidden')).toBe('true');
			expect(line(watched)).toBeNull();

			const airing = row('50')!;
			expect(airing.querySelector('.row-caption')!.textContent).toBe(
				m.watchorder_meta_progress_open({ episode: 5 })
			);
			expect(line(airing)).toBeNull();

			const fresh = row('5')!;
			expect(fresh.querySelector('.row-caption')).toBeNull();
			expect(fresh.querySelector('.row-line')).toBeNull();

			// The meta line gives the episode count again, progress or not.
			for (const a of [partway, watched, fresh]) {
				expect(a.querySelector('.meta')!.textContent).toContain(m.strip_card_episodes_suffix());
			}
		});
	}

	it('draws every row’s caption on one baseline, holding the line’s place where there is none', async () => {
		progressFranchise();
		app = mount(FranchisePage, { target });
		await until(
			() => (row('10')?.textContent ?? '').includes(m.watchorder_meta_watched()),
			'the progress'
		);
		for (const id of ['10', '50']) {
			const slot = row(id)!.querySelector('.row-line.slot');
			expect(slot, id).not.toBeNull();
			expect(slot!.getAttribute('aria-hidden')).toBe('true');
		}
	});
});

// The line's colours: one app colour for list progress, named once,
// whatever show the row is. The "You are here" rule keeps the show's
// accent, which means "this show, here". happy-dom has no cascade to
// measure, so these are syntactic constraints on the style blocks.
describe('franchise page progress colours', () => {
	const view = readFileSync(
		resolve(process.cwd(), 'src/lib/watch-order/FranchiseView.svelte'),
		'utf8'
	);
	const style = view.slice(view.indexOf('<style>'));
	const rule = (selector: string) => {
		const at = style.indexOf(`\n\t${selector} {`);
		expect(at, selector).toBeGreaterThan(-1);
		return style.slice(at, style.indexOf('}', at));
	};
	const tokens = readFileSync(resolve(process.cwd(), 'src/lib/design/tokens.css'), 'utf8');

	it('names the list progress colours once, as tokens', () => {
		expect(tokens).toContain('--progress-fill: var(--bone-200);');
		expect(tokens).toContain('--progress-track: var(--ink-200);');
	});

	it('fills the line in the progress colour, never thinner than 3px', () => {
		expect(rule('.row-line')).toContain('background: var(--progress-track);');
		const fill = rule('.row-line-fill');
		expect(fill).toContain('background: var(--progress-fill);');
		expect(fill).toContain('min-inline-size: 3px;');
		expect(style).not.toMatch(/\.row-line[^{]*\{[^}]*--row-accent/);
	});

	it('gives the unnumbered rows’ caption column the numbered rows’ width', () => {
		// "ЭП 1000 ИЗ 1100" in uppercase tracking needs the wider column.
		expect(rule('.story-row')).toContain(
			'grid-template-columns: 4.5rem 3.5rem minmax(0, 1fr) 9rem;'
		);
		expect(rule('.story-row.compact')).toContain(
			'grid-template-columns: 3rem minmax(0, 1fr) 9rem;'
		);
	});

	it('keeps the show’s accent on the "You are here" rule', () => {
		expect(rule('.here .story-row')).toContain('border-inline-start-color: var(--row-accent);');
	});
});

describe('franchise page — leaving it while Kitsu is slow', () => {
	it('cancels the franchise read still out, so the walk holds no connection', async () => {
		const fetches = watchPageFetches();
		let release: (() => void) | null = null;
		useFranchise({
			franchise: () =>
				new Promise<Response>((resolve) => {
					release = () => resolve(new HttpResponse(null, { status: 503 }));
				})
		});
		app = mount(FranchisePage, { target });
		const franchiseRead = (f: { path: string }) => f.path === `/api/kitsu/franchise/${KITSU_ID}`;
		await until(() => fetches.sent.some(franchiseRead), 'the franchise read');
		unmount(app);
		app = null;
		try {
			await until(() => fetches.failed.some(franchiseRead), 'the franchise read cancelled', 2000);
		} finally {
			(release as (() => void) | null)?.();
		}
	});
});
