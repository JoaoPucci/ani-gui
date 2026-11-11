// Acceptance: the franchise page on real franchises.
//
// Each page is served the franchise the backend's walk answers for it
// over the listings Kitsu served (tests/fixtures/franchise/served-*.json,
// kept equal to the walk by backend/src/commands/franchise_corpus_test.rs).
// The page is named as the walk names the franchise: after the main
// story's first series by start (One Piece, also from its spin-off LEGO
// ONE PIECE; Lupin III, not its pilot film; FLCL, its 2000 OVA), or its
// first title where it holds no series.
// Every row carries its whole title, a film sits under Films whatever
// role Kitsu gave it, a TV re-edit sits with the other versions, and
// the recaps show like any other section.

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

/**
 * Every corpus anchor's franchise page and the name it carries: the
 * name the backend's walk answers with (`name`), from the main story's
 * first series by start, skipping the films, pilots, specials and
 * one-episode OVAs before it, else from its first title.
 */
const HEADINGS: readonly (readonly [string, string, string])[] = [
	['12', 'One Piece', 'One Piece'],
	['50656', 'One Piece', 'LEGO ONE PIECE'],
	['50409', 'One Piece', "CHOPPER's"],
	['45513', 'One Piece', 'One Piece Film: Red'],
	['428', 'One Piece', 'the 1998 OVA'],
	['11', 'Naruto', 'Naruto'],
	['1555', 'Naruto', 'Naruto: Shippuuden'],
	['13051', 'Naruto', 'Boruto'],
	['10089', 'Naruto', 'Boruto: Naruto the Movie'],
	['7543', 'Naruto', 'The Last: Naruto the Movie'],
	['244', 'BLEACH', 'BLEACH'],
	['43078', 'BLEACH', 'Sennen Kessen-hen'],
	['1515', 'BLEACH', 'Memories of Nobody'],
	['199', 'Dragon Ball', 'Dragon Ball'],
	['720', 'Dragon Ball', 'Dragon Ball Z'],
	['10879', 'Dragon Ball', 'Dragon Ball Super'],
	['44544', 'Dragon Ball', 'Super Hero, a film in the main story'],
	['210', 'Detective Conan', 'Detective Conan'],
	['696', 'Detective Conan', 'its first film'],
	['818', 'Gintama', 'Gintama'],
	['7241', 'Gintama', 'its second film, in the main story'],
	['486', 'Pocket Monsters', 'Pokémon'],
	['1265', 'Lupin III', 'Lupin III, after its 1969 pilot film'],
	['323', 'Fate/stay night', 'Fate/stay night'],
	['6028', 'Fate/stay night', 'Fate/Zero'],
	['6928', 'Fate/stay night', 'a Fate/Zero special'],
	['3919', 'Bakemonogatari', 'Bakemonogatari'],
	['7158', 'JoJo no Kimyou na Bouken (TV)', 'the 2012 series'],
	['7730', 'JoJo no Kimyou na Bouken', 'the 2000 OVA'],
	['7442', 'Attack on Titan', 'Attack on Titan'],
	['46874', 'SPY×FAMILY', 'SPY×FAMILY CODE: White'],
	['184', 'R.O.D OVA', 'the 2001 OVA'],
	['185', 'R.O.D OVA', 'the 2003 series'],
	['202', 'FLCL', 'the 2000 OVA'],
	['11961', 'FLCL', 'FLCL Progressive'],
	['6084', 'Berserk', 'the first Golden Age film'],
	['11655', 'Kenpuu Denki Berserk', 'the 2016 series, after the 1997 one'],
	['115', 'Hunter x Hunter', 'the 1999 series'],
	['1046', 'Sakura Taisen: Ouka Kenran', 'the 1997 OVA; the TV series is a version'],
	['1162', 'Kidou Keisatsu Patlabor', 'the 1988 OVA; the TV series is a version'],
	['2357', 'Kara no Kyoukai 1: Fukan Fuukei', 'seven films'],
	['7124', 'Persona 3 the Movie 1: Spring of Birth', 'four films'],
	['841', 'Top wo Nerae! Gunbuster', 'two OVAs'],
	['14084', 'Boku no Hero Academia', 'its first film, a side story of the seasons'],
	['11469', 'Boku no Hero Academia', 'its first season'],
	['13881', 'Boku no Hero Academia', 'its third season'],
	['42205', 'Boku no Hero Academia', 'Heroes:Rising, which names no parent of its own'],
	['47844', 'Boku no Hero Academia', 'the fourth film']
];

describe('franchise page — the name of every corpus franchise', () => {
	for (const [id, heading, what] of HEADINGS) {
		it(`is named ${heading} from ${what} (${id})`, async () => {
			openFranchise(id);
			await until(() => title() !== '', 'the page');
			expect(title()).toBe(heading);
		});
	}
});

describe('franchise page — real franchises', () => {
	// Films that name a series their parent story sit under Films, as
	// their strips call them films.
	for (const id of ['1515', '45513', '46874', '696', '7543']) {
		it(`lists the film ${id} under Films`, async () => {
			openFranchise(id);
			await until(() => rail(m.watchorder_franchise_films()) !== null, 'the films');
			expect(inRail(m.watchorder_franchise_films(), id)).toBe(true);
			expect(inRail(m.watchorder_franchise_sides(), id)).toBe(false);
		});
	}

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
