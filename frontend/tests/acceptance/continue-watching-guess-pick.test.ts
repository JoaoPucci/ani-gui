// Acceptance: which show a guessed Continue card opens and plays.
//
// A Continue row with no recorded show is matched by a Kitsu search for
// its title. The hit the row itself names wins: one whose title is the
// row's, and before that one from the year the row names — a trailing
// "(2009)" in its title, or the year its show key carries, which the
// history route reads for it (`key_year`). The user's Black Clover card
// used to open Kitsu's one-episode Jump Festa special, whose slug the
// title spells, and play nothing; the JoJo card from hianime's 2000
// listing opened the 2012 series and played it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers } from './home-handlers';

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
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

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

/** A Kitsu search hit as recorded for the user's lines. */
function hit(
	id: string,
	canonical_title: string,
	slug: string,
	subtype: string,
	start_date: string,
	episode_count: number,
	titles: Record<string, string>
) {
	return {
		id,
		canonical_title,
		titles,
		abbreviated_titles: [],
		slug,
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count,
		subtype,
		status: 'finished',
		start_date,
		average_rating: null
	};
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
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

/** Mount Home over `row` and the search's `hits`, click the card that
 *  names `shown`, and return the play it asks for. */
async function playTheCard(
	row: { ep_no: string; id: string; title: string; key_year?: number },
	hits: ReturnType<typeof hit>[],
	shown: string
): Promise<URLSearchParams> {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json([row])),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json(hits)),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
		])
	);
	app = mount(HomePage, { target });
	const card = () =>
		Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(shown)) ??
		null;
	await until(() => card() !== null, `the card showing ${shown}`);
	card()!.click();
	await until(() => FakeEventSource.instances.length > 0, "the card's play");
	return new URL(FakeEventSource.instances[0].url).searchParams;
}

describe('a guessed Continue card for Black Clover', () => {
	it('opens and plays the TV series, not the special its title spells the slug of', async () => {
		const asked = await playTheCard(
			{ ep_no: '1', id: 'hianime:black-clover-949', title: 'Black Clover' },
			[
				hit('13209', 'Black Clover', 'black-clover-tv', 'TV', '2017-10-03', 170, {
					en: 'Black Clover',
					en_jp: 'Black Clover'
				}),
				hit(
					'12625',
					'Black Clover: Jump Festa 2016 Special',
					'black-clover',
					'special',
					'2016-11-27',
					1,
					{
						en_jp: 'Black Clover: Jump Festa 2016 Special'
					}
				)
			],
			'Black Clover'
		);
		expect(asked.get('title')).toBe('Black Clover');
		expect(asked.get('subtype')).toBe('TV');
		expect(asked.get('episode_count')).toBe('170');
		expect(asked.get('episode')).toBe('2');
	});
});

describe("a guessed Continue card for hianime's 2000 JoJo listing", () => {
	it('opens and plays the 2000 OVA its key names, not the 2012 series', async () => {
		const asked = await playTheCard(
			{
				ep_no: '1',
				id: 'hianime:jojos-bizarre-adventure-2000-5182',
				title: "JoJo's Bizarre Adventure",
				key_year: 2000
			},
			[
				hit(
					'7158',
					'JoJo no Kimyou na Bouken (TV)',
					'jojo-s-bizarre-adventure-2012',
					'TV',
					'2012-10-06',
					26,
					{
						en_us: "JoJo's Bizarre Adventure",
						en: "JoJo's Bizarre Adventure (2012)"
					}
				),
				hit(
					'7730',
					'JoJo no Kimyou na Bouken: Adventure',
					'jojo-s-bizarre-adventure-2000',
					'OVA',
					'2000-05-25',
					7,
					{
						en: "JoJo's Bizarre Adventure (2000)"
					}
				)
			],
			'JoJo no Kimyou na Bouken'
		);
		expect(asked.get('title')).toBe('JoJo no Kimyou na Bouken: Adventure');
		expect(asked.get('year')).toBe('2000');
	});
});

describe('two guessed Continue cards titled alike, from different years', () => {
	it('each keep the show its own year names, on the next visit too', async () => {
		// The backend keeps a row's stored guess under the year it names
		// (`year`), as `backend/src/api/title_match_year_test.rs` holds it
		// to; this fake keeps them the same way.
		const stored = new Map<string, string>();
		const keyOf = (title: unknown, provider: unknown, year: unknown) =>
			`${title}|${provider ?? ''}|${year ?? ''}`;
		const rows = [
			{
				ep_no: '1',
				id: 'hianime:jojos-bizarre-adventure-2012-9',
				title: "JoJo's Bizarre Adventure",
				key_year: 2012
			},
			{
				ep_no: '1',
				id: 'hianime:jojos-bizarre-adventure-2000-5182',
				title: "JoJo's Bizarre Adventure",
				key_year: 2000
			}
		];
		const hits = [
			hit(
				'7158',
				'JoJo no Kimyou na Bouken (TV)',
				'jojo-s-bizarre-adventure-2012',
				'TV',
				'2012-10-06',
				26,
				{
					en_us: "JoJo's Bizarre Adventure",
					en: "JoJo's Bizarre Adventure (2012)"
				}
			),
			hit(
				'7730',
				'JoJo no Kimyou na Bouken: Adventure',
				'jojo-s-bizarre-adventure-2000',
				'OVA',
				'2000-05-25',
				7,
				{
					en: "JoJo's Bizarre Adventure (2000)"
				}
			)
		];
		server.use(
			...homeHandlers({}, [
				http.get(`${API_BASE}/api/history`, () => HttpResponse.json(rows)),
				http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json(hits)),
				http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
					HttpResponse.json(hits.find((h) => h.id === params.id) ?? null)
				),
				http.get(`${API_BASE}/api/title-match`, ({ request }) => {
					const q = new URL(request.url).searchParams;
					return HttpResponse.json(
						stored.get(keyOf(q.get('title'), q.get('provider'), q.get('year'))) ?? null
					);
				}),
				http.put(`${API_BASE}/api/title-match`, async ({ request }) => {
					const b = (await request.json()) as Record<string, unknown>;
					stored.set(keyOf(String(b.title), b.provider, b.year), String(b.kitsu_id));
					return new HttpResponse(null, { status: 204 });
				})
			])
		);
		const shown = (title: string) =>
			Array.from(target.querySelectorAll('button')).some((b) => b.textContent?.includes(title));
		const both = () =>
			shown('JoJo no Kimyou na Bouken (TV)') && shown('JoJo no Kimyou na Bouken: Adventure');

		app = mount(HomePage, { target });
		await until(both, 'both cards, each its own show');
		await until(() => stored.size >= 1, 'the guesses to be stored');
		await new Promise((r) => setTimeout(r, 50));
		unmount(app);
		app = null;

		// The next visit reads the stored guesses.
		app = mount(HomePage, { target });
		await until(both, 'both cards again, each its own show');
	});
});
