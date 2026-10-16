// Acceptance: an episode missing from this audio suggests the other
// audio only when the availability cache says the other audio has it.
//
// The sentence used to tell the user to try the other audio every
// time. With the dub cached as unavailable, a user who followed it
// switched to dub and found the detail page's Play hidden, with
// nothing saying why. These cases mount the play and detail routes,
// fail a real play click through the typed SSE verdict, answer the
// cache lookup three ways, and read the rendered overlay. They also
// pin that the lookup is the cache-only batch route and that nothing
// asks the provider about the other audio.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '40684';
const TITLE = 'Ongoing Show';
const VERDICT = JSON.stringify({
	kind: 'episode_unavailable',
	key: 'error.play.episode_unavailable'
});

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	setParams({ id: KITSU_ID });
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

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

type CacheAnswer = {
	cached: Record<string, boolean>;
	playable_episode_counts: Record<string, number>;
};

/** What the routes asked for: every cache-only batch body, the mode
 *  of every availability probe, and every cache warm. */
interface Asked {
	batch: { kitsu_ids: string[]; mode: string }[];
	probeModes: string[];
	warms: number;
}

/** The availability routes. The cache answers `cache` for the other
 *  audio (dub) and holds nothing for the audio being played, so the
 *  routes' own lookups for sub are unaffected. */
function availabilityHandlers(cache: CacheAnswer | 'fails', asked: Asked) {
	return [
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			asked.probeModes.push(((await request.json()) as { mode: string }).mode);
			return HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			});
		}),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
			asked.batch.push(body);
			if (body.mode !== 'dub')
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			if (cache === 'fails') return new HttpResponse(null, { status: 500 });
			return HttpResponse.json(cache);
		}),
		http.post(`${API_BASE}/api/availability/warm`, () => {
			asked.warms += 1;
			return new HttpResponse(null, { status: 202 });
		})
	];
}

function expectCacheOnly(asked: Asked) {
	expect(asked.batch).toContainEqual({ kitsu_ids: [KITSU_ID], mode: 'dub' });
	expect(asked.probeModes, 'the provider is never asked about the other audio').not.toContain(
		'dub'
	);
	expect(asked.warms, 'nothing warms the cache on a failure').toBe(0);
}

type Handoff = 'external' | 'syncplay';

function useHandlers(cache: CacheAnswer | 'fails'): Asked {
	const asked: Asked = { batch: [], probeModes: [], warms: 0 };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig('sub'))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		...availabilityHandlers(cache, asked),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		// Both handoffs resolve the episode before they spawn anything;
		// the resolve answers with the verdict.
		...(['external', 'syncplay'] as Handoff[]).map((h) =>
			http.post(`${API_BASE}/api/play/${h}`, () =>
				HttpResponse.json(JSON.parse(VERDICT), { status: 404 })
			)
		)
	);
	return asked;
}

const tile = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;

/** Click episode `episode`'s tile and fail its interactive stream with
 *  the episode verdict. */
async function clickAndFail(episode: number) {
	await until(() => tile(episode) !== null, `the tile for episode ${episode}`);
	tile(episode)!.click();
	await until(
		() =>
			FakeEventSource.instances.some(
				(i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1')
			),
		"the click's interactive stream"
	);
	FakeEventSource.instances
		.filter((i) => i.url.includes(`episode=${episode}`) && !i.url.includes('prefetch=1'))
		.pop()!
		.dispatch('error', VERDICT);
}

const SUGGESTS = () => m.play_play_failure_episode_unavailable_try_other_audio();
const PLAIN = () => m.play_play_failure_episode_unavailable();

const CASES: [string, CacheAnswer | 'fails', () => string][] = [
	[
		'the cache has the episode in the other audio',
		{ cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 12 } },
		SUGGESTS
	],
	[
		'the cache knows the other audio has nothing',
		{ cached: { [KITSU_ID]: false }, playable_episode_counts: {} },
		PLAIN
	],
	[
		'the cache holds no answer for the other audio',
		{ cached: {}, playable_episode_counts: {} },
		PLAIN
	],
	['the cache lookup fails', 'fails', PLAIN]
];

/** The other audio has the show but stops at episode 1 — short of the
 *  episode each route fails, so a route that looked up the wrong
 *  episode, or none, would suggest it. */
const SHORT: [string, CacheAnswer, () => string] = [
	"the other audio's count stops short of the episode",
	{ cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 1 } },
	PLAIN
];

async function expectCopy(expected: () => string) {
	await until(
		() => (target.textContent ?? '').includes(expected()),
		'the episode-unavailable copy on the failure overlay'
	);
	if (expected === PLAIN) expect(target.textContent).not.toContain(SUGGESTS());
}

describe('detail — the episode verdict and the other audio', () => {
	for (const [when, cache, expected] of [...CASES, SHORT]) {
		it(`when ${when}`, async () => {
			const asked = useHandlers(cache);
			app = mount(DetailPage, { target });
			await clickAndFail(5);
			await expectCopy(expected);
			expectCacheOnly(asked);
		});
	}
});

describe('play — the episode verdict and the other audio', () => {
	for (const [when, cache, expected] of [...CASES, SHORT]) {
		it(`when ${when}`, async () => {
			const asked = useHandlers(cache);
			setUrl(`/play/${KITSU_ID}`, { session: 'session-other-audio', episode: '1', kind: 'mp4' });
			app = mount(PlayPage, { target });
			await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
			await clickAndFail(2);
			await expectCopy(expected);
			expectCacheOnly(asked);
		});
	}
});

const moreButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_more_aria_label()}"]`
	) as HTMLButtonElement | null;

const menuItem = (label: string) =>
	Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined;

describe('play — a handoff that meets the episode verdict and the other audio', () => {
	const labels: [Handoff, () => string][] = [
		['external', () => m.play_external_label()],
		['syncplay', () => m.play_hamburger_syncplay_label()]
	];
	for (const [handoff, label] of labels) {
		for (const [when, cache, expected] of [...CASES, SHORT]) {
			it(`${handoff}, when ${when}`, async () => {
				const asked = useHandlers(cache);
				setUrl(`/play/${KITSU_ID}`, { session: 'session-handoff', episode: '3', kind: 'mp4' });
				app = mount(PlayPage, { target });
				await until(() => moreButton() !== null, 'the More actions button under the player');
				moreButton()!.click();
				await until(() => menuItem(label()) !== undefined, `the "${label()}" menu item`);
				menuItem(label())!.click();
				await expectCopy(expected);
				expectCacheOnly(asked);
			});
		}
	}
});

function useHomeHandlers(cache: CacheAnswer | 'fails', asked: Asked) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig('sub'))),
		http.get(`${API_BASE}/api/history`, () =>
			HttpResponse.json([{ ep_no: '3', id: 'provider-1', title: TITLE }])
		),
		http.post(`${API_BASE}/api/kitsu/search`, () =>
			HttpResponse.json([{ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' }])
		),
		...availabilityHandlers(cache, asked),
		http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(null)),
		http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([]))
	);
}

const resumeButton = () =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(TITLE)) ?? null;

/** Mount home, click the Continue card, and fail its resolve with the
 *  episode verdict. */
async function resumeAndFail() {
	app = mount(HomePage, { target });
	await until(() => resumeButton() !== null, 'the resolved Continue card');
	const streamsBefore = FakeEventSource.instances.length;
	resumeButton()!.click();
	await until(() => FakeEventSource.instances.length > streamsBefore, 'the resume resolve stream');
	FakeEventSource.instances[FakeEventSource.instances.length - 1].dispatch('error', VERDICT);
}

describe('home — a Continue Watching verdict and the other audio', () => {
	for (const [when, cache, expected] of [...CASES, SHORT]) {
		it(`when ${when}`, async () => {
			const asked: Asked = { batch: [], probeModes: [], warms: 0 };
			useHomeHandlers(cache, asked);

			await resumeAndFail();
			await expectCopy(expected);
			expectCacheOnly(asked);
		});
	}
});

describe('detail — the page stays busy while the failure is worded', () => {
	it('keeps Play disabled until the other-audio answer lands', async () => {
		// The overlay waits on the cache lookup. Releasing the page's
		// busy state first left a window with neither the loading
		// overlay nor the failure, in which a second click could start
		// a play the late message then covered.
		useHandlers({ cached: {}, playable_episode_counts: {} });
		let release!: () => void;
		const answered = new Promise<void>((r) => (release = r));
		let asked = false;
		server.use(
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { mode: string };
				if (body.mode === 'dub') {
					asked = true;
					await answered;
				}
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			})
		);
		const play = () =>
			target.querySelector('div.actions button.btn-glass') as HTMLButtonElement | null;
		app = mount(DetailPage, { target });
		await until(() => play() !== null && !play()!.disabled, 'the Play button');
		await clickAndFail(5);
		await until(() => asked, 'the other-audio lookup');
		expect(play()!.disabled, 'Play stays disabled while the message is worded').toBe(true);
		expect(target.textContent).not.toContain(PLAIN());
		release();
		await expectCopy(PLAIN);
		await until(() => !play()!.disabled, 'Play to come back');
	});
});

describe('home — the Continue card stays busy while the failure is worded', () => {
	it('keeps the card resuming until the other-audio answer lands', async () => {
		const asked: Asked = { batch: [], probeModes: [], warms: 0 };
		useHomeHandlers({ cached: {}, playable_episode_counts: {} }, asked);
		let release!: () => void;
		const answered = new Promise<void>((r) => (release = r));
		let held = false;
		server.use(
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { mode: string };
				if (body.mode === 'dub') {
					held = true;
					await answered;
				}
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			})
		);
		await resumeAndFail();
		await until(() => held, 'the other-audio lookup');
		expect(
			resumeButton()!.classList.contains('resume-card-busy'),
			'the card stays resuming while the message is worded'
		).toBe(true);
		expect(target.textContent).not.toContain(PLAIN());
		release();
		await expectCopy(PLAIN);
		await until(
			() => !resumeButton()!.classList.contains('resume-card-busy'),
			'the card to settle'
		);
	});
});
