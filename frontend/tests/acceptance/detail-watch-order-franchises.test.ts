// Acceptance: the show page's watch-order strip on real franchises.
//
// Each page is served the franchise the backend's walk answers for it
// over the listings Kitsu served (tests/fixtures/franchise/served-*.json,
// kept equal to the walk by backend/src/commands/franchise_corpus_test.rs).
// One Piece is the main story: Kitsu lists its two-episode spin-off LEGO
// ONE PIECE as its parent story, and the page must not call One Piece a
// side story of it. LEGO ONE PIECE's page is the side story, under One
// Piece. A film that names a series its parent story reads "Film",
// any other side story "Side story". Every tile names its entry by its
// whole title.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';

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

interface Served {
	anchor: string;
	entries: { id: string; title: string }[];
	sides: { id: string; title: string }[];
	films: { id: string; title: string }[];
}

/** The walk's answer for `id`, as the endpoint serves it. */
function served(id: string): Served {
	const path = resolvePath(__dirname, `../../../tests/fixtures/franchise/served-${id}.json`);
	return JSON.parse(readFileSync(path, 'utf8')) as Served;
}

/** The title the walk knows for `id`, from its own payload. */
function titleOf(f: Served, id: string): string {
	return [...f.entries, ...f.sides, ...f.films].find((e) => e.id === id)!.title;
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

/** Open `id`'s show page with the walk's answer for it. */
function openShow(id: string) {
	const f = served(id);
	setParams({ id });
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), watch_order: 'linked' })
		),
		http.get(`${API_BASE}/api/kitsu/anime/${id}`, () =>
			HttpResponse.json({ ...kitsuRef(id, titleOf(f, id), 12), subtype: 'TV' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${id}`, () => HttpResponse.json(f)),
		http.get(`${API_BASE}/api/kitsu/airing/${id}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		// One similar title, which the page shows only once the strip
		// (or its absence) is decided.
		http.post(`${API_BASE}/api/kitsu/search`, () =>
			HttpResponse.json([kitsuRef('77', 'Similar 77', 12)])
		),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json({ cached: { '77': true } })
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${id}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			return HttpResponse.json(Object.fromEntries(ids.map((i) => [i, null])));
		}),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	app = mount(DetailPage, { target });
}

/** The strip: the section headed "Watch order". */
const strip = (): HTMLElement | null =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2 .eyebrow-key')?.textContent === m.watchorder_strip_eyebrow()
	) ?? null;
const tile = (id: string) => strip()?.querySelector(`a[href="/anime/${id}"]`) ?? null;
const tileTitle = (id: string) => tile(id)?.querySelector('.tile-title')?.textContent ?? null;

describe('detail route — the watch-order strip on real franchises', () => {
	it('never calls One Piece a side story of its spin-off LEGO ONE PIECE', async () => {
		openShow('12');
		// "Similar titles" waits for the franchise read and what the page
		// draws from it, so once it shows, the page has decided.
		await until(
			() => target.querySelector('section.similar a[href="/anime/77"]') !== null,
			'Similar titles'
		);
		expect(target.textContent).not.toContain(m.watchorder_strip_side_story());
		expect(target.querySelector('a[href="/anime/50656"]')).toBeNull();
	});

	it("leads from One Piece's page to One Piece's franchise", async () => {
		openShow('12');
		await until(
			() => target.querySelector('a[href="/anime/12/franchise"]') !== null,
			'the franchise link'
		);
		expect(target.textContent).not.toContain(m.watchorder_strip_side_story());
	});

	it('shows LEGO ONE PIECE as a side story of One Piece', async () => {
		openShow('50656');
		await until(() => strip() !== null, 'the strip');
		expect(strip()!.textContent).toContain(m.watchorder_strip_side_story());
		expect(tileTitle('12')).toBe('One Piece');
	});

	// Films that name a series their parent story: the walk lists each
	// under films, and the strip says "Film" where a side story reads
	// "Side story". Each anchor's tiles show its parent stories.
	for (const [id, parent] of [
		['1515', '244'], // Bleach Movie 1: Memories of Nobody
		['45513', '12'], // One Piece Film: Red
		['46874', '45619'], // SPY×FAMILY CODE: White
		['696', '210'], // Detective Conan Movie 01: The Timed Skyscraper
		['7543', '1555'] // The Last: Naruto the Movie
	] as const) {
		it(`calls the film ${id} a film, under its parent story ${parent}`, async () => {
			openShow(id);
			await until(() => strip() !== null, 'the strip');
			expect(strip()!.textContent).toContain(m.watchorder_strip_film());
			expect(strip()!.textContent).not.toContain(m.watchorder_strip_side_story());
			expect(tile(parent)).not.toBeNull();
		});
	}

	// Side stories that are not films keep "Side story".
	for (const [id, what] of [
		['428', 'the 1998 One Piece OVA'],
		['50409', "CHOPPER's TV series"],
		['50656', 'LEGO ONE PIECE'],
		['6928', 'the Fate/Zero special']
	] as const) {
		it(`calls ${what} (${id}) a side story, not a film`, async () => {
			openShow(id);
			await until(() => strip() !== null, 'the strip');
			expect(strip()!.textContent).toContain(m.watchorder_strip_side_story());
			expect(strip()!.textContent).not.toContain(m.watchorder_strip_film());
		});
	}

	it("names Naruto's next entry by its whole title", async () => {
		openShow('11');
		await until(() => strip() !== null, 'the strip');
		expect(tileTitle('1555')).toBe('Naruto: Shippuuden');
	});

	it("names Bleach's next entry by its whole title, prefix and all", async () => {
		openShow('244');
		await until(() => strip() !== null, 'the strip');
		expect(tileTitle('43078')).toBe('BLEACH: Sennen Kessen-hen');
	});

	it("names Dragon Ball's next entry by its whole title", async () => {
		openShow('199');
		await until(() => strip() !== null, 'the strip');
		expect(tileTitle('720')).toBe('Dragon Ball Z');
	});
});
