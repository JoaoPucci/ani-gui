// Acceptance: a Continue card whose stored mapping names a later season
// Kitsu marks without a cour suffix on its slug.
//
// A play from a show's own page stores the provider→Kitsu mapping, and
// a later Home visit reads the card through it. The cour check reads a
// Kitsu slug without a suffix as the first season, and Gintama' is
// `gintama-2` — so a "Gintama Season 2" card bound to it was passed
// over and fell back to a guess. Kitsu titles that entry "Gintama
// Season 2", the provider's own title, and a card bound to an entry
// that carries its title is the show the user played. A card bound to
// another season's entry — Stone Ocean Part 2 to Part 1 — is still
// passed over, and so is Slime Season 2: Kitsu titles that entry "That
// Time I Got Reincarnated as a Slime 2" and "…2nd Season Part 1", never
// as the provider does.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef, type KitsuRefFixture } from './home-handlers';

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
import { goto } from '$app/navigation';
import { __resetApiBaseForTests } from '../../src/lib/api';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {}
	dispatch(name: string, data: string) {
		const ev = { data, type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
	);
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

/** A Kitsu entry as Kitsu serves it: its slug and every title it carries. */
function entry(
	id: string,
	slug: string,
	canonical: string,
	titles: Record<string, string>,
	abbreviated: string[],
	episodes: number
): KitsuRefFixture {
	return {
		...kitsuRef(id, canonical, episodes),
		slug,
		titles,
		abbreviated_titles: abbreviated
	};
}

const GINTAMA_2 = entry(
	'5971',
	'gintama-2',
	"Gintama'",
	{ en_jp: "Gintama'", en_us: 'Gintama Season 2', ja_jp: "銀魂'" },
	['Gintama (2011)'],
	51
);
const STONE_OCEAN_1 = entry(
	'44294',
	'jojo-no-kimyou-na-bouken-stone-ocean',
	"JoJo's Bizarre Adventure: Stone Ocean",
	{
		en: "JoJo's Bizarre Adventure: Stone Ocean",
		en_jp: 'JoJo no Kimyou na Bouken: Stone Ocean'
	},
	['Jojo part 6'],
	12
);
const SLIME_2 = entry(
	'42196',
	'tensei-shitara-slime-datta-ken-2',
	'Tensei shitara Slime Datta Ken 2',
	{
		en: 'That Time I Got Reincarnated as a Slime 2',
		en_jp: 'Tensei shitara Slime Datta Ken 2'
	},
	['TenSura 2', 'Tensei Shitara Slime Datta Ken 2nd Season Part 1'],
	12
);

/** Home with one Continue row whose mapping a play stored as `ref`.
 *  Kitsu's search answers `ref` too, so a card that passes the mapping
 *  over still resolves — as a guess. */
function mountWithPlayedMapping(showId: string, title: string, ref: KitsuRefFixture) {
	server.use(
		...homeHandlers({ history: [{ ep_no: '3', id: showId, title }] }, [
			http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId/played`, () =>
				HttpResponse.json(ref.id)
			),
			http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(ref.id)),
			http.get(`${API_BASE}/api/kitsu/anime/${ref.id}`, () => HttpResponse.json(ref)),
			// The slug fallback a multi-cour row tries before searching
			// finds no entry under the provider's title.
			http.get(`${API_BASE}/api/kitsu/anime-by-slug/:slug`, () => HttpResponse.json(null)),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([ref]))
		])
	);
	app = mount(HomePage, { target });
}

/** The card renders the Kitsu entry's title once resolved. */
const card = (title: string) =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(title)) ?? null;

/** Click the resolved card; the Kitsu id its play asks to record (null
 *  when none) and whether the play page opened as a guess. */
async function play(title: string): Promise<{ recorded: string | null; guess: boolean }> {
	await until(() => card(title) !== null, `the resolved Continue card for ${title}`);
	card(title)!.click();
	await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
	const stream = FakeEventSource.instances[0];
	const recorded = new URL(stream.url).searchParams.get('kitsu_id');
	stream.dispatch(
		'done',
		JSON.stringify({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4'
		})
	);
	await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the play page');
	const url = String(vi.mocked(goto).mock.calls.at(-1)?.[0] ?? '');
	return { recorded, guess: new URL(url, 'http://app.test').searchParams.get('guess') === '1' };
}

describe('a Continue card bound to a later season by a play', () => {
	it("is linked when Kitsu titles the entry as the provider does (Gintama Season 2 → Gintama')", async () => {
		mountWithPlayedMapping('hianime:gintama-season-2-1153', 'Gintama Season 2', GINTAMA_2);

		expect(await play(GINTAMA_2.canonical_title)).toEqual({ recorded: '5971', guess: false });
	});

	it('stays a guess when the entry is another season (Stone Ocean Part 2 → Part 1)', async () => {
		const title = "JoJo's Bizarre Adventure Part 6: Stone Ocean Part 2";
		mountWithPlayedMapping(
			'hianime:jojos-bizarre-adventure-stone-ocean-part-2-1464',
			title,
			STONE_OCEAN_1
		);

		expect(await play(STONE_OCEAN_1.canonical_title)).toEqual({ recorded: null, guess: true });
	});

	it('stays a guess for Slime Season 2, which no Kitsu title names as the provider does', async () => {
		const title = 'That Time I Got Reincarnated as a Slime Season 2';
		mountWithPlayedMapping(
			'hianime:that-time-i-got-reincarnated-as-a-slime-season-2-1',
			title,
			SLIME_2
		);

		expect(await play(SLIME_2.canonical_title)).toEqual({ recorded: null, guess: true });
	});
});

// The same check guards the title match a Continue row remembered from
// an earlier resolve. A remembered Gintama Season 2 → Gintama' match was
// passed over, and with nothing else answering the card fell back to a
// search link; it now resolves to the entry.
describe('a remembered title match to a later season', () => {
	it('resolves the card when Kitsu titles the entry as the provider does', async () => {
		server.use(
			...homeHandlers(
				{
					history: [{ ep_no: '3', id: 'hianime:gintama-season-2-1153', title: 'Gintama Season 2' }]
				},
				[
					http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(GINTAMA_2.id)),
					http.get(`${API_BASE}/api/kitsu/anime/${GINTAMA_2.id}`, () =>
						HttpResponse.json(GINTAMA_2)
					),
					http.get(`${API_BASE}/api/kitsu/anime-by-slug/:slug`, () => HttpResponse.json(null))
				]
			)
		);
		app = mount(HomePage, { target });

		await until(() => card(GINTAMA_2.canonical_title) !== null, "the card resolved to Gintama'");
		const searchLink = Array.from(target.querySelectorAll('a')).find((a) =>
			a.getAttribute('href')?.includes('/search')
		);
		expect(searchLink).toBeUndefined();
	});
});
