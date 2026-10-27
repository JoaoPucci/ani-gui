// Acceptance: the notice that replaces Play names the other audio
// when the availability cache has the title in it.
//
// Kitsu 40684 is in the catalogue with subtitles and has nothing
// dubbed. In dub, the detail page hid Play behind "Not in the
// streaming catalogue", which is false: the title is there, in the
// other audio. These cases mount the detail page in dub with the
// probe answering unavailable, answer the cache's question about the
// other audio several ways, and read the notice. They also pin that
// the other audio is learned from the cache-only batch route, with no
// probe and no warm.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { afterRead, watchPageFetches, type PageFetch } from './page-fetches';
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

const KITSU_ID = '40684';
const TITLE = 'Ongoing Show';

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

type CacheAnswer = {
	cached: Record<string, boolean>;
	playable_episode_counts: Record<string, number>;
};

interface Asked {
	batch: { kitsu_ids: string[]; mode: string }[];
	probeModes: string[];
	warms: number;
}

/** The page in `mode`; the probe answers unavailable for it, and the
 *  cache answers `other` for the other audio. */
function useHandlers(
	mode: 'sub' | 'dub',
	other: CacheAnswer | 'fails',
	probeAvailable = false,
	subtype: string | null = null,
	unsettled?: string
): Asked {
	const asked: Asked = { batch: [], probeModes: [], warms: 0 };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(mode))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished', subtype })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			asked.probeModes.push(((await request.json()) as { mode: string }).mode);
			return HttpResponse.json(
				probeAvailable
					? { available: true, episode_count: 12, extra_episodes: [] }
					: { available: false, episode_count: null, extra_episodes: [], unsettled }
			);
		}),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
			asked.batch.push(body);
			if (body.mode === mode) return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			if (other === 'fails') return new HttpResponse(null, { status: 500 });
			return HttpResponse.json(other);
		}),
		http.post(`${API_BASE}/api/availability/warm`, () => {
			asked.warms += 1;
			return new HttpResponse(null, { status: 202 });
		}),
		http.put(`${API_BASE}/api/settings`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	return asked;
}

const notice = () => target.querySelector('p.unavailable[role="status"]')?.textContent ?? null;

/** The page's probe about the audio it plays. */
const isProbe = (r: PageFetch) => r.path === '/api/availability';
/** The page's cache question about `mode`. */
const isBatch = (mode: string) => (r: PageFetch) =>
	r.path === '/api/availability/batch' && r.body.mode === mode;

const HAS = { cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 4 } };

const CASES: [string, 'sub' | 'dub', CacheAnswer | 'fails', () => string][] = [
	['dub, and the cache has it subtitled', 'dub', HAS, () => m.detail_unavailable_in_dub_message()],
	['sub, and the cache has it dubbed', 'sub', HAS, () => m.detail_unavailable_in_sub_message()],
	[
		'dub, and the cache knows the other audio has nothing',
		'dub',
		{ cached: { [KITSU_ID]: false }, playable_episode_counts: {} },
		() => m.detail_unavailable_message()
	],
	[
		'dub, and the cache holds no answer for the other audio',
		'dub',
		{ cached: {}, playable_episode_counts: {} },
		() => m.detail_unavailable_message()
	],
	[
		'dub, and the cache has a count-less row for the other audio',
		'dub',
		{ cached: { [KITSU_ID]: true }, playable_episode_counts: {} },
		() => m.detail_unavailable_in_dub_message()
	],
	['dub, and the cache lookup fails', 'dub', 'fails', () => m.detail_unavailable_message()]
];

describe('detail — the unavailable notice and the other audio', () => {
	for (const [when, mode, other, expected] of CASES) {
		it(`in ${when}`, async () => {
			const asked = useHandlers(mode, other);
			const otherMode = mode === 'dub' ? 'sub' : 'dub';
			app = mount(DetailPage, { target });
			await until(() => notice() !== null, 'the unavailable notice');
			await until(
				() => asked.batch.some((b) => b.mode === otherMode),
				'the cache lookup for the other audio'
			);
			await until(() => (notice() ?? '').includes(expected()), 'the notice copy');
			expect(asked.batch).toContainEqual({ kitsu_ids: [KITSU_ID], mode: otherMode });
			expect(asked.probeModes, 'the provider is never asked about the other audio').not.toContain(
				otherMode
			);
			expect(asked.warms, 'nothing warms the cache for the notice').toBe(0);
		});
	}
});

describe('detail — a title the provider refused by name', () => {
	it('keeps "not in the streaming catalogue", and never asks or names the other audio', async () => {
		// A refusal by name is about the title in every audio: the
		// provider lists nothing named for it. Even with the cache
		// holding the other audio, the notice must not send the viewer
		// there, and the page must not ask.
		const asked = useHandlers('dub', HAS, false, null, 'refused');
		const { sent, read } = watchPageFetches();
		app = mount(DetailPage, { target });
		await until(() => notice() !== null, 'the unavailable notice');
		// The page has read the refusal, and what it set off has run to
		// its next request: the question about the other audio would be
		// asked by now.
		await until(() => read.some(isProbe), 'the refusal to be read');
		await afterRead();
		expect(sent.filter(isBatch('sub')), 'the other audio is never asked').toEqual([]);
		expect(notice()).toContain(m.detail_unavailable_message());
		expect(notice()).not.toContain(m.detail_unavailable_in_dub_message());
		expect(
			asked.batch.filter((b) => b.mode === 'sub'),
			'the other audio is never asked'
		).toEqual([]);
	});
});

describe('detail — the notice asks about the other audio only while it is up', () => {
	it('asks nothing about the other audio when the chosen audio has the title', async () => {
		const asked = useHandlers('dub', HAS, true);
		const { sent, read } = watchPageFetches();
		const play = () => target.querySelector('div.actions button.btn-glass');
		app = mount(DetailPage, { target });
		await until(() => play() !== null, 'the Play button');
		// Play shows once the probe's answer is applied. That answer has
		// been read, and what it set off has run to its next request:
		// a question about the other audio would be asked by now.
		await until(() => read.some(isProbe), 'the probe answer to be read');
		await afterRead();
		expect(notice()).toBeNull();
		expect(sent.filter(isBatch('sub'))).toEqual([]);
		expect(asked.batch.filter((b) => b.mode === 'sub')).toEqual([]);
	});

	it('does not show an old answer after the audio changes', async () => {
		// In dub, the question "does sub have it?" is out and held. The
		// viewer switches to sub; the held answer (yes) lands after.
		// It answers a question nobody asks any more, and naming sub
		// as the audio that has the title would be wrong in sub.
		useHandlers('dub', { cached: {}, playable_episode_counts: {} });
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const { read } = watchPageFetches();
		let askedSub = false;
		server.use(
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { mode: string };
				if (body.mode === 'sub') {
					askedSub = true;
					await held;
					return HttpResponse.json(HAS);
				}
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			})
		);
		app = mount(DetailPage, { target });
		await until(() => askedSub, 'the lookup for sub');
		const subButton = Array.from(target.querySelectorAll('button.seg-btn')).find(
			(b) => b.textContent?.trim() === 'SUB'
		) as HTMLButtonElement;
		subButton.click();
		await until(
			() => target.querySelector('button.seg-btn.active')?.textContent?.trim() === 'SUB',
			'the audio switch to land'
		);
		await until(() => notice() !== null, 'the notice in sub');
		release();
		// The page has read the old answer, and what it set off has run
		// to its next write.
		await until(() => read.some(isBatch('sub')), 'the old answer to be read');
		await afterRead();
		expect(notice()).toContain(m.detail_unavailable_message());
		expect(notice()).not.toContain(m.detail_unavailable_in_dub_message());
		expect(notice()).not.toContain(m.detail_unavailable_in_sub_message());
	});

	it("keeps the catalogue sentence when the other audio's cached count is zero", async () => {
		useHandlers('dub', {
			cached: { [KITSU_ID]: true },
			playable_episode_counts: { [KITSU_ID]: 0 }
		});
		const { read } = watchPageFetches();
		app = mount(DetailPage, { target });
		await until(() => notice() !== null, 'the unavailable notice');
		// The notice shows before the cache answers about the other
		// audio. The page has read that answer, and applied it.
		await until(() => read.some(isBatch('sub')), "the other audio's answer to be read");
		await afterRead();
		expect(notice()).toContain(m.detail_unavailable_message());
	});
});

describe('detail — a new question starts from no answer', () => {
	it('drops the other-audio sentence while the next question is out', async () => {
		// In dub the cache says sub has the title, and the notice says
		// so. In sub the question becomes "does dub have it?", and until
		// it is answered the notice must not keep naming sub.
		useHandlers('dub', HAS);
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let askedDub = false;
		server.use(
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { mode: string };
				if (body.mode === 'dub') {
					askedDub = true;
					await held;
					return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
				}
				return HttpResponse.json(HAS);
			})
		);
		app = mount(DetailPage, { target });
		await until(
			() => (notice() ?? '').includes(m.detail_unavailable_in_dub_message()),
			'the notice naming sub'
		);
		const subButton = Array.from(target.querySelectorAll('button.seg-btn')).find(
			(b) => b.textContent?.trim() === 'SUB'
		) as HTMLButtonElement;
		subButton.click();
		await until(() => askedDub, 'the lookup for dub');
		expect(notice()).toContain(m.detail_unavailable_message());
		release();
	});
});

describe('detail — a music title never names the other audio', () => {
	it('keeps the catalogue sentence and asks nothing, whatever the cache holds', async () => {
		// A music title is unavailable in both audios by the page's own
		// rule, so "switch the audio" would lead to the same notice.
		// A positive row for its id is plausible — a probe can match an
		// unrelated show for it — and must not be believed.
		const asked = useHandlers('dub', HAS, false, 'music');
		const { sent } = watchPageFetches();
		app = mount(DetailPage, { target });
		await until(() => notice() !== null, 'the unavailable notice');
		// The notice is up, which is the state the question would be
		// asked from, and what it sets off has run to its next request.
		await afterRead();
		expect(sent.filter(isBatch('sub'))).toEqual([]);
		expect(notice()).toContain(m.detail_unavailable_message());
		expect(asked.batch.filter((b) => b.mode === 'sub')).toEqual([]);
	});
});

describe('detail — an answer for a show the page has left', () => {
	it('is not shown on the next show', async () => {
		// The page moves to another show (a Similar-title link reuses
		// the component) while the question about the first show's
		// other audio is out. The first show's answer must not name an
		// audio for the second.
		const OTHER_ID = '777';
		useHandlers('dub', { cached: {}, playable_episode_counts: {} });
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const { read, failed } = watchPageFetches();
		let askedFirst = false;
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/${OTHER_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(OTHER_ID, 'Another Show', 12), status: 'finished' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${OTHER_ID}`, () =>
				HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
				if (body.mode === 'sub' && body.kitsu_ids[0] === KITSU_ID) {
					askedFirst = true;
					await held;
					// The first show has it subtitled — and answers with
					// the id of the show the page is now on as well, so a
					// late answer applied to the wrong question would show.
					return HttpResponse.json({
						cached: { [KITSU_ID]: true, [OTHER_ID]: true },
						playable_episode_counts: {}
					});
				}
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			})
		);
		app = mount(DetailPage, { target });
		await until(() => askedFirst, "the lookup for the first show's other audio");
		setParams({ id: OTHER_ID });
		await until(
			() => (target.textContent ?? '').includes('Another Show'),
			'the second show to load'
		);
		await until(() => notice() !== null, "the second show's notice");
		release();
		// The page is done with the first show's question: its answer
		// has been read, or the move stopped it. What either set off has
		// run to its next write.
		const firstShows = (r: PageFetch) =>
			isBatch('sub')(r) && (r.body.kitsu_ids as string[])[0] === KITSU_ID;
		await until(
			() => read.some(firstShows) || failed.some(firstShows),
			"the first show's question to be answered or stopped"
		);
		await afterRead();
		expect(notice()).toContain(m.detail_unavailable_message());
	});
});
