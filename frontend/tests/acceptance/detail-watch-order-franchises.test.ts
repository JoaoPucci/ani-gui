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
// whole title. A page outside the main story walks its own chain: its
// own titles numbered among themselves, the main-story title it meets
// shown beside them under its own key, and a note in the heading.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { availabilityAnswer } from '../answers';
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
import {
	__resetApiBaseForTests,
	type AiringStatus,
	type AvailabilityBatchResponse,
	type AvailabilityResponse,
	type Config,
	type Franchise,
	type HistoryEntry,
	type HistoryRowByKitsu,
	type KitsuAnimeRef,
	type KitsuEpisode
} from '../../src/lib/api';

/** The walk's answer for `id`, as the endpoint serves it (the
 *  backend records these files from its own answers, and its corpus
 *  test checks each against the walk). */
function served(id: string): Franchise {
	const path = resolvePath(__dirname, `../../../tests/fixtures/franchise/served-${id}.json`);
	return JSON.parse(readFileSync(path, 'utf8')) as Franchise;
}

/** The title the walk knows for `id`, from its own payload. */
function titleOf(f: Franchise, id: string): string {
	return [...f.entries, ...f.sides, ...f.films, ...f.versions].find((e) => e.id === id)!.title;
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
function openShow(id: string, order: 'linked' | 'released' = 'linked') {
	const f = served(id);
	setParams({ id });
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json<Config>({ ...appConfig(), watch_order: order })
		),
		http.get(`${API_BASE}/api/kitsu/anime/${id}`, () =>
			HttpResponse.json<KitsuAnimeRef>({ ...kitsuRef(id, titleOf(f, id), 12), subtype: 'TV' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${id}`, () => HttpResponse.json<Franchise>(f)),
		http.get(`${API_BASE}/api/kitsu/airing/${id}`, () =>
			HttpResponse.json<AiringStatus>({
				aired: 12,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		// One similar title, which the page shows only once the strip
		// (or its absence) is decided.
		http.post(`${API_BASE}/api/kitsu/search`, () =>
			HttpResponse.json<KitsuAnimeRef[]>([kitsuRef('77', 'Similar 77', 12)])
		),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json<AvailabilityBatchResponse>({
				cached: { '77': true },
				playable_episode_counts: {}
			})
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({
					available: true,
					episode_count: 12,
					extra_episodes: [],
					episode_count_approximate: false
				})
			)
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${id}`, () =>
			HttpResponse.json<HistoryRowByKitsu | null>(null)
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json<string | null>('/dl')),
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			return HttpResponse.json<Record<string, HistoryEntry | null>>(
				Object.fromEntries(ids.map((i) => [i, null]))
			);
		}),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json<Record<string, number>>({}))
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
const heading = () => strip()?.querySelector('h2')?.textContent ?? '';
const columnKey = (side: 'before' | 'after') =>
	strip()?.querySelector(`.col-key.key-${side}`)?.textContent?.trim() ?? null;

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
		['7543', '1555'], // The Last: Naruto the Movie
		// My Hero Academia's films are linked to each other, not to the
		// seasons; each still reads as a film under a season.
		['14084', '13881'], // Futari no Hero, under the third season
		['42205', '13881'], // Heroes:Rising, which names no parent of its own
		['47844', '47232'] // YOU'RE NEXT, under the seventh season
	] as const) {
		it(`calls the film ${id} a film, under its parent story ${parent}`, async () => {
			openShow(id);
			await until(() => strip() !== null, 'the strip');
			expect(strip()!.textContent).toContain(m.watchorder_strip_film());
			expect(strip()!.textContent).not.toContain(m.watchorder_strip_side_story());
			expect(tile(parent)).not.toBeNull();
		});
	}

	it("does not line My Hero Academia's films up as a story of their own", async () => {
		openShow('14084');
		await until(() => strip() !== null, 'the strip');
		expect(tile('42205')).toBeNull();
		expect(tile('44209')).toBeNull();
	});

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

	it("numbers Berserk's first Golden Age film among the films, outside the main story", async () => {
		openShow('6084');
		await until(() => strip() !== null, 'the strip');
		expect(heading()).toContain(m.watchorder_strip_outside());
		expect(heading()).toContain(m.watchorder_strip_position({ position: '01', total: '03' }));
		expect(tile('6664')).not.toBeNull();
		expect(tile('24')).toBeNull();
	});

	it('shows the JoJo series the 2000 OVA follows, keyed as the main story it comes from', async () => {
		openShow('7730');
		await until(() => strip() !== null, 'the strip');
		expect(heading()).toContain(m.watchorder_strip_position({ position: '01', total: '02' }));
		expect(tile('7158')).not.toBeNull();
		expect(columnKey('before')).toBe(m.watchorder_strip_seam_from());
		expect(tile('610')).not.toBeNull();
	});

	it('in release order, shows the JoJo series after the 2000 OVA, keyed as the main story it leads into', async () => {
		openShow('7730', 'released');
		await until(() => strip() !== null, 'the strip');
		expect(heading()).toContain(m.watchorder_strip_position({ position: '02', total: '02' }));
		expect(columnKey('after')).toBe(m.watchorder_strip_seam_into());
	});

	it('calls Boruto: Naruto the Movie a film, after Shippuden from the main story', async () => {
		openShow('10089');
		await until(() => strip() !== null, 'the strip');
		expect(heading()).toContain(m.watchorder_strip_outside());
		expect(strip()!.textContent).toContain(m.watchorder_strip_film());
		expect(tile('1555')).not.toBeNull();
		expect(columnKey('before')).toBe(m.watchorder_strip_seam_from());
	});

	it("says nothing of a chain on the main story's pages", async () => {
		openShow('11655');
		await until(() => strip() !== null, 'the strip');
		expect(heading()).not.toContain(m.watchorder_strip_outside());
		expect(columnKey('before')).not.toBe(m.watchorder_strip_seam_from());
	});
});
