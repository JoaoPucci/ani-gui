// Acceptance: the episode strip follows prev/next navigation across
// its page boundary.
//
// The strip shows five tiles per page and opens on the page holding
// the current episode. Prev/next and auto-play swap episodes with a
// same-route `goto`, so the page never remounts — the strip has to
// follow the URL's episode on its own. When the user has paginated
// away to browse, it must NOT follow: the strip is theirs then.
//
// The decision is a pure function with its own unit tests; this
// mounts the real page and checks what the user sees in the DOM.
// `goto` is stubbed in this tier, so the episode switch is driven the
// way the page itself would land it: by moving the URL stub.
//
// The stream is an mp4 session — happy-dom has no MediaSource, so an
// HLS session would land on the error overlay instead of the player.

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

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

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

function useShowHandlers(episodeCount = 12) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, episodeCount), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({
				aired: episodeCount,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(episodeCount))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: episodeCount,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

async function mountAtEpisode(episode: number) {
	setUrl(`/play/${KITSU_ID}`, { episode: String(episode) });
	app = mount(PlayPage, { target });
	await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
	FakeEventSource.instances[0].dispatch(
		'done',
		JSON.stringify({
			id: 'session-1',
			kind: 'mp4',
			has_subtitles: false,
			quality: '1080',
			mode: 'sub'
		})
	);
	// The page carries the session in the URL and gets there by
	// `goto`, which this tier stubs out — so the stub is moved by
	// hand to where the real navigation would have landed.
	setUrl(`/play/${KITSU_ID}`, {
		session: 'session-1',
		episode: String(episode),
		kind: 'mp4'
	});
}

/** The episode switch as the page lands it: same route, new episode
 *  in the query — the real `goto` is a stub in this tier. */
function landOnEpisode(episode: number) {
	setUrl(`/play/${KITSU_ID}`, {
		session: 'session-1',
		episode: String(episode),
		kind: 'mp4'
	});
}

const tile = (n: number) => target.querySelector(`li[data-ep-num="${n}"]`);
const tileNumbers = () =>
	Array.from(target.querySelectorAll('li[data-ep-num]')).map((li) =>
		Number(li.getAttribute('data-ep-num'))
	);
const currentCardIn = (n: number) => tile(n)?.querySelector('.ep-card-current');
const pagerNext = () =>
	target.querySelector(
		`button[aria-label="${m.play_episodes_pager_next_aria_label()}"]`
	) as HTMLButtonElement | null;

describe('play route — the episode strip follows navigation', () => {
	it('paginates to the next page when the episode walks off the visible one', async () => {
		useShowHandlers();
		await mountAtEpisode(5);

		// The strip opens on the page holding ep 5: tiles 1–5.
		await until(() => currentCardIn(5) != null, 'the strip on page 1 with ep 5 current');
		expect(tileNumbers()).toEqual([1, 2, 3, 4, 5]);

		// Next (or auto-play) lands on ep 6 — the first tile of page 2.
		landOnEpisode(6);
		await until(() => currentCardIn(6) != null, 'the strip to follow onto page 2');
		expect(tileNumbers()).toEqual([6, 7, 8, 9, 10]);
		expect(tile(5)).toBeNull();

		// And back: Prev from the first tile of page 2 returns to page 1.
		landOnEpisode(5);
		await until(() => currentCardIn(5) != null, 'the strip to follow back onto page 1');
		expect(tileNumbers()).toEqual([1, 2, 3, 4, 5]);
	});

	it('leaves a strip the user paginated away alone', async () => {
		useShowHandlers();
		await mountAtEpisode(1);

		await until(() => currentCardIn(1) != null, 'the strip on page 1 with ep 1 current');

		// The user browses ahead to page 2 while ep 1 keeps playing.
		await until(() => pagerNext() != null && !pagerNext()!.disabled, 'the pager to be ready');
		pagerNext()!.click();
		await until(() => tile(6) != null, 'the browsed page 2 to render');
		expect(tileNumbers()).toEqual([6, 7, 8, 9, 10]);

		// Auto-play advances 1 → 2. The strip is the user's now — it
		// must not snap back to page 1 under them.
		landOnEpisode(2);
		await until(
			() =>
				target.textContent?.includes(m.play_episode_nav_current_label({ episode: '2' })) === true,
			'the episode change to land in the nav cluster'
		);
		expect(tileNumbers()).toEqual([6, 7, 8, 9, 10]);
		expect(tile(1)).toBeNull();
	});

	it('discards a stale mount-time fetch that resolves after a follow', async () => {
		// The mount-time strip request and a follow triggered while it
		// is still in flight race each other: both assign the strip's
		// window on completion. The slower initial response must not
		// overwrite the follow that already landed — that would strand
		// the strip on the stale page with the current episode
		// highlighted nowhere.
		let releaseFirst!: () => void;
		const firstBlocked = new Promise<void>((resolve) => {
			releaseFirst = resolve;
		});
		let episodesRequests = 0;
		useShowHandlers();
		server.use(
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, async () => {
				episodesRequests += 1;
				if (episodesRequests === 1) await firstBlocked;
				return HttpResponse.json(kitsuEpisodes(12));
			})
		);

		await mountAtEpisode(5);

		// The initial page-1 request is held open; the follow to page 2
		// fires its own request, which resolves immediately.
		landOnEpisode(6);
		await until(() => currentCardIn(6) != null, 'the follow onto page 2 to land');
		expect(tileNumbers()).toEqual([6, 7, 8, 9, 10]);
		expect(episodesRequests).toBe(2);

		// Now the stale mount-time response arrives. Nothing observable
		// may change, so give its completion a beat to (wrongly) land
		// before asserting the strip still shows the followed page.
		releaseFirst();
		await new Promise((resolve) => setTimeout(resolve, 200));
		expect(tileNumbers()).toEqual([6, 7, 8, 9, 10]);
		expect(currentCardIn(6)).not.toBeNull();
	});

	it('supersedes an in-flight follow when navigation returns to the rendered page', async () => {
		// Follow 20 → 21 starts fetching page 5; Prev returns to 20
		// before it resolves. The strip still RENDERS page 4, so a
		// decision fed the rendered page sees nothing to do and arms no
		// superseding fetch — and the generation guard is idle because
		// no new generation exists. The page-5 response must not land
		// under episode 20. (Ep 21 crosses a backing-API page boundary,
		// which is what makes the follow's fetch actually leave the
		// cache and stay in flight.)
		let releasePageTwo!: () => void;
		const pageTwoBlocked = new Promise<void>((resolve) => {
			releasePageTwo = resolve;
		});
		useShowHandlers(30);
		server.use(
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, async ({ request }) => {
				if (new URL(request.url).searchParams.get('page') === '2') await pageTwoBlocked;
				return HttpResponse.json(kitsuEpisodes(30));
			})
		);

		await mountAtEpisode(20);
		await until(() => currentCardIn(20) != null, 'the strip on page 4 with ep 20 current');
		expect(tileNumbers()).toEqual([16, 17, 18, 19, 20]);

		// Next: the follow to page 5 hangs on the blocked request while
		// the strip keeps rendering page 4.
		landOnEpisode(21);
		await until(
			() =>
				target.textContent?.includes(m.play_episode_nav_current_label({ episode: '21' })) === true,
			'the switch to ep 21 to land'
		);
		expect(tileNumbers()).toEqual([16, 17, 18, 19, 20]);

		// Prev, back to the page the strip never stopped rendering.
		landOnEpisode(20);
		await until(
			() =>
				target.textContent?.includes(m.play_episode_nav_current_label({ episode: '20' })) === true,
			'the switch back to ep 20 to land'
		);

		// The stale page-5 response arrives. Nothing observable may
		// change, so give its completion a beat to (wrongly) land.
		releasePageTwo();
		await new Promise((resolve) => setTimeout(resolve, 200));
		expect(tileNumbers()).toEqual([16, 17, 18, 19, 20]);
		expect(currentCardIn(20)).not.toBeNull();
	});

	it('retries a follow that failed transiently on the next episode change', async () => {
		// The follow 20 → 21 fails (its backing-API page 500s). The
		// strip was still TRACKING playback — the failure must not
		// reclassify it as browsed-away, or 21 → 22 would decline the
		// retry and the strip would strand on page 4 for the rest of
		// the session.
		let failPageTwo = true;
		let pageTwoRequests = 0;
		useShowHandlers(30);
		server.use(
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
				if (new URL(request.url).searchParams.get('page') === '2') {
					pageTwoRequests += 1;
					if (failPageTwo) return new HttpResponse(null, { status: 500 });
				}
				return HttpResponse.json(kitsuEpisodes(30));
			})
		);

		await mountAtEpisode(20);
		await until(() => currentCardIn(20) != null, 'the strip on page 4 with ep 20 current');
		const requestsBeforeFollow = pageTwoRequests;

		// Next: the follow to page 5 hits the failing request.
		landOnEpisode(21);
		await until(() => pageTwoRequests > requestsBeforeFollow, 'the follow fetch to be attempted');
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(tileNumbers()).toEqual([16, 17, 18, 19, 20]);

		// The transient failure clears; the next episode change must
		// retry the follow.
		failPageTwo = false;
		landOnEpisode(22);
		await until(() => currentCardIn(22) != null, 'the retried follow onto page 5');
		expect(tileNumbers()).toEqual([21, 22, 23, 24, 25]);
	});

	it('follows onto a page whose manual browse failed once playback reaches it', async () => {
		// From ep 20 / page 4 the user clicks the pager toward page 5
		// and the fetch fails. The strip never stopped showing page 4 —
		// the failed click must not leave browsed-away intent behind,
		// or auto-play onto 21 (page 5) would decline to follow and the
		// current episode would go missing from the strip.
		let failPageTwo = true;
		let pageTwoRequests = 0;
		useShowHandlers(30);
		server.use(
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
				if (new URL(request.url).searchParams.get('page') === '2') {
					pageTwoRequests += 1;
					if (failPageTwo) return new HttpResponse(null, { status: 500 });
				}
				return HttpResponse.json(kitsuEpisodes(30));
			})
		);

		await mountAtEpisode(20);
		await until(() => currentCardIn(20) != null, 'the strip on page 4 with ep 20 current');
		const requestsBeforeBrowse = pageTwoRequests;

		// The user pages toward 5; the fetch fails and the strip keeps
		// rendering page 4.
		await until(() => pagerNext() != null && !pagerNext()!.disabled, 'the pager to be ready');
		pagerNext()!.click();
		await until(() => pageTwoRequests > requestsBeforeBrowse, 'the browse fetch to be attempted');
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(tileNumbers()).toEqual([16, 17, 18, 19, 20]);

		// The failure clears; auto-play advances onto the page the
		// browse never reached. The strip follows playback there.
		failPageTwo = false;
		landOnEpisode(21);
		await until(() => currentCardIn(21) != null, 'the follow onto page 5');
		expect(tileNumbers()).toEqual([21, 22, 23, 24, 25]);
	});
});
