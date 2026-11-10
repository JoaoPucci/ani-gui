// Acceptance: the franchise page on real franchises.
//
// Each page is served the franchise the backend's walk answers for it
// over the listings Kitsu served (tests/fixtures/franchise/served-*.json,
// kept equal to the walk by backend/src/commands/franchise_corpus_test.rs).
// The page is named after the actual main story (One Piece, also from
// its spin-off LEGO ONE PIECE), every row carries its whole title, a
// film sits under Films whatever role Kitsu gave it, a TV re-edit sits
// with the other versions, and the recaps show like any other section.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';

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

/** The walk's answer for `id`, as the endpoint serves it. */
function served(id: string): Record<string, unknown> {
	const path = resolvePath(__dirname, `../../../tests/fixtures/franchise/served-${id}.json`);
	return JSON.parse(readFileSync(path, 'utf8')) as Record<string, unknown>;
}

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
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** Open `id`'s franchise page with the walk's answer for it. */
function openFranchise(id: string) {
	setParams({ id });
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), watch_order: 'linked' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${id}`, () => HttpResponse.json(served(id))),
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			return HttpResponse.json(Object.fromEntries(ids.map((k) => [k, null])));
		})
	);
	app = mount(FranchisePage, { target });
}

const title = () => target.querySelector('h1')?.textContent?.trim() ?? '';
const mainStory = () =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2')?.textContent?.trim() === m.watchorder_franchise_main()
	) ?? null;
const rows = () => [...(mainStory()?.querySelectorAll('li a') ?? [])] as HTMLAnchorElement[];
const rowTitle = (id: string) =>
	rows()
		.find((a) => a.getAttribute('href') === `/anime/${id}`)
		?.querySelector('.row-title')?.textContent ?? null;
const rail = (eyebrow: string) =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2 .eyebrow-key')?.textContent?.trim() === eyebrow
	) ?? null;
const inRail = (eyebrow: string, id: string) =>
	rail(eyebrow)?.querySelector(`a[href="/anime/${id}"]`) != null;

describe('franchise page — real franchises', () => {
	it("is One Piece's, with LEGO ONE PIECE among its side stories", async () => {
		openFranchise('12');
		await until(() => title() !== '' && rows().length > 0, 'the page');
		expect(title()).toBe('One Piece');
		expect(rows().map((a) => a.getAttribute('href'))).toEqual(['/anime/12']);
		expect(inRail(m.watchorder_franchise_sides(), '50656')).toBe(true);
	});

	it("is One Piece's from LEGO ONE PIECE too, LEGO a side story", async () => {
		openFranchise('50656');
		await until(() => title() !== '' && rows().length > 0, 'the page');
		expect(title()).toBe('One Piece');
		expect(rows().map((a) => a.getAttribute('href'))).toEqual(['/anime/12']);
		expect(inRail(m.watchorder_franchise_sides(), '50656')).toBe(true);
	});

	it("lists Naruto's main story by whole titles, and Boruto's film under Films", async () => {
		openFranchise('11');
		await until(() => rows().length === 4, 'the main story');
		expect(title()).toBe('Naruto');
		expect(rowTitle('13051')).toBe('Boruto: Naruto Next Generations');
		expect(rowTitle('47181')).toBe('Boruto: Naruto Next Generations Part 2');
		expect(inRail(m.watchorder_franchise_films(), '10089')).toBe(true);
		expect(inRail(m.watchorder_franchise_versions(), '10089')).toBe(false);
	});

	it("lists Bleach's main story by whole titles", async () => {
		openFranchise('244');
		await until(() => rows().length === 5, 'the main story');
		expect(rowTitle('43078')).toBe('BLEACH: Sennen Kessen-hen');
		expect(rowTitle('46903')).toBe('BLEACH: Sennen Kessen-hen - Ketsubetsu-tan');
	});

	it("puts Dragon Ball's films under Films, Beerus beside Kai, and shows the recaps", async () => {
		openFranchise('199');
		await until(() => rows().length > 0, 'the main story');
		expect(title()).toBe('Dragon Ball');
		expect(inRail(m.watchorder_franchise_films(), '461')).toBe(true);
		expect(inRail(m.watchorder_franchise_films(), '7176')).toBe(true);
		expect(inRail(m.watchorder_franchise_versions(), '461')).toBe(false);
		expect(inRail(m.watchorder_franchise_versions(), '4394')).toBe(true);
		expect(inRail(m.watchorder_franchise_versions(), '50497')).toBe(true);
		expect(inRail(m.watchorder_franchise_recaps(), '50497')).toBe(false);
		// The recaps are on the page from the start.
		expect(inRail(m.watchorder_franchise_recaps(), '11468')).toBe(true);
	});
});
