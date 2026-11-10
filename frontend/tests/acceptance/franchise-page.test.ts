// Acceptance: the franchise page, /anime/<id>/franchise.
//
// The page lists a whole franchise around the show it was opened
// from: a masthead named after the franchise's first title, with the
// number of titles it lists and a Resume button for the title being
// watched (or a way to start from the beginning), then the main story
// as a numbered list in the order the setting chooses, the show it was
// opened from marked, and the other versions, films and side stories
// on rails, the recaps folded away. A franchise that cannot be read
// says so and offers to try again.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { flushSync, mount, unmount } from 'svelte';
import { get } from 'svelte/store';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig } from './home-handlers';
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
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

interface Options {
	order?: string;
	franchise?: () => Response;
	history?: { ep_no: string; id: string; title: string; kitsu_id?: string }[];
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
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json(opts.history ?? []))
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
	it('names the franchise after its first title and counts what it lists', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => title() !== '', 'the masthead');
		// The first title in link order is Origins, the prequel. Its
		// "(OVA)" goes: no other title reads "Saga: Origins".
		expect(title()).toBe('Saga: Origins');
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
		// Short titles on the rows, the full title as the tooltip.
		expect(row('50')!.textContent).toContain('Final Arc');
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
		expect(masthead.textContent).toContain('Final Arc');
		expect(masthead.textContent).toContain(m.watchorder_meta_progress_open({ episode: 3 }));
		// Saga is watched to the end, and its row says so.
		expect(row('10')!.textContent).toContain(m.watchorder_meta_watched());
	});

	it('offers to start from the first title when nothing is part-way', async () => {
		useFranchise();
		app = mount(FranchisePage, { target });
		await until(() => resumeLink() !== null, 'the masthead');
		expect(resumeLink()!.textContent?.trim()).toBe(m.watchorder_franchise_start());
		expect(resumeLink()!.getAttribute('href')).toBe('/anime/5');
	});

	it('puts other versions, films and side stories on rails, and folds the recaps away', async () => {
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
		const show = button(m.watchorder_franchise_recaps_show())!;
		expect(show.getAttribute('aria-expanded')).toBe('false');
		expect(target.querySelector('a[href="/anime/90"]')).toBeNull();
		show.click();
		flushSync();
		expect(target.querySelector('a[href="/anime/90"]')).not.toBeNull();
		const hide = button(m.watchorder_franchise_recaps_hide())!;
		expect(hide.getAttribute('aria-expanded')).toBe('true');
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
		expect(button(m.watchorder_franchise_recaps_show())).toBeUndefined();
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
