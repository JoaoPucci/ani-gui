// Acceptance: the player's "Up next" card at a finale.
//
// On the last episode of an entry that has finished airing, once the
// video ends, the Skip Ending pill's corner holds a card naming the
// entry that comes next in the run, in the order the setting chooses.
// Through the credits the pill behaves as on any episode. The card
// links to that entry's page and waits there: the end of the video
// plays nothing further, and the next-episode button stays within the
// entry. A playhead a moment short of the length counts as the end
// too. A seek back puts the card away, and the end brings it back. The
// card names the next entry without asking the provider anything about
// it. A fork lists the ways on; a next entry that has not aired shows
// when it airs and links nowhere. Esc dismisses the card while it has
// focus. It stays clear of the controls that hide in fullscreen, and
// under reduced motion it simply appears.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { watchPageFetches } from './page-fetches';
import { recordMotion, setReducedMotion } from './motion';
import { playerVideo, playerVideoInSlot } from './player-video';
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

import { goto } from '$app/navigation';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';
const TITLE = 'Saga: Second Arc';
const EPISODES = 12;
const LENGTH = 1420;
const OPENING = { skip_type: 'op', start_time: 60, end_time: 150 };
const ENDING = { skip_type: 'ed', start_time: 1300, end_time: 1390 };

const entry = (
	id: string,
	title: string,
	start: string | null,
	extra: Partial<{ status: string; episodes: number | null; subtype: string }> = {}
) => ({
	id,
	title,
	subtype: 'TV',
	start,
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null,
	...extra
});

// Link order: Saga → Second Arc → Final Arc, and Origins, a prequel
// story released after Second Arc, before Saga. Release order puts
// Origins straight after Second Arc.
const ORIGINS = entry('5', 'Saga: Origins', '2013-04-01', { subtype: 'OVA' });
const SAGA = entry('10', 'Saga', '2010-01-01');
const SECOND = entry(KITSU_ID, TITLE, '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', '2014-01-01');

function franchise(
	over: Partial<{ entries: unknown[]; links: string[][]; complete: boolean }> = {}
) {
	return {
		anchor: KITSU_ID,
		entries: [SAGA, SECOND, ORIGINS, FINAL],
		links: [
			['10', KITSU_ID],
			[KITSU_ID, '50'],
			['5', '10']
		],
		versions: [],
		films: [],
		sides: [],
		recaps: [],
		parents: {},
		complete: true,
		...over
	};
}

class SilentEventSource {
	addEventListener() {}
	close() {}
}
type GlobalLike = { EventSource?: unknown };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	vi.mocked(goto).mockClear();
	g.EventSource = SilentEventSource;
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	await setReducedMotion(false);
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

interface Options {
	episode?: number;
	order?: string;
	autoPlayNext?: boolean;
	nativeControls?: boolean;
	skips?: { skip_type: string; start_time: number; end_time: number }[];
	/** Null: the franchise cannot be read. */
	franchise?: ReturnType<typeof franchise> | null;
	/** How many episodes the source lists and has aired: Kitsu's 12 unless set. */
	sourceEpisodes?: number;
	/** Fractional extras the source lists ("12.5"). */
	extras?: string[];
	autoSkipOp?: boolean;
	/** The watch history: none unless set. */
	history?: HistoryRowFixture[];
	/**
	 * The row the backend names for an id where it is not the row that
	 * recorded it (`history_claim::names`): a row whose recorded entry
	 * Kitsu deleted, named by its stored mapping or title match.
	 */
	named?: Record<string, HistoryRowFixture>;
}

type HistoryRowFixture = { id: string; title: string; ep_no: string; kitsu_id?: string };

/** Opens `options.episode` of Second Arc and gives its video a length. */
async function openEpisode(options: Options = {}) {
	const fetches = watchPageFetches();
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({
				...appConfig(),
				watch_order: options.order ?? 'linked',
				auto_play_next: options.autoPlayNext === true,
				auto_skip_op: options.autoSkipOp === true,
				use_custom_player_controls: options.nativeControls !== true
			})
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, () =>
			options.franchise === null
				? HttpResponse.json({ error: 'error.kitsu.unreachable' }, { status: 502 })
				: HttpResponse.json(options.franchise ?? franchise())
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json(options.history ?? [])),
		// The rows the franchise's entries resume from, as the resume
		// lookup names them.
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			const rows = options.history ?? [];
			return HttpResponse.json(
				Object.fromEntries(
					ids.map((k) => [k, options.named?.[k] ?? rows.find((r) => r.kitsu_id === k) ?? null])
				)
			);
		}),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({
				aired: options.sourceEpisodes ?? EPISODES,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(EPISODES))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: options.sourceEpisodes ?? EPISODES,
				extra_episodes: options.extras ?? [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () =>
			HttpResponse.json(options.skips ?? [OPENING, ENDING])
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	setUrl(`/play/${KITSU_ID}`, {
		session: 'session-1',
		episode: String(options.episode ?? EPISODES),
		kind: 'mp4'
	});
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	const video = playerVideo();
	Object.defineProperty(video, 'duration', { configurable: true, get: () => LENGTH });
	video.dispatchEvent(new Event('loadedmetadata'));
	video.dispatchEvent(new Event('durationchange'));
	await fetches.settled('the franchise, the settings and the skip times');
	return { video, fetches };
}

/** Moves the playhead to `at`, as playback reaching it would. */
async function playTo(video: HTMLVideoElement, at: number) {
	video.currentTime = at;
	video.dispatchEvent(new Event('timeupdate'));
	await new Promise((r) => setTimeout(r, 0));
}

/** Plays the video to its end: the playhead reaches the length and the
 *  element fires `ended`. The DOM here holds the element's own `ended`
 *  property at false; the cases that need it set it themselves. */
async function playToEnd(video: HTMLVideoElement) {
	await playTo(video, LENGTH);
	video.dispatchEvent(new Event('ended'));
	await new Promise((r) => setTimeout(r, 0));
}

const card = () => target.querySelector<HTMLElement>('.up-next');
const cardLinks = () => [...(card()?.querySelectorAll<HTMLAnchorElement>('a') ?? [])];
const skipPill = () => target.querySelector('.player-skip-btn');
const announcement = () => card()?.querySelector('[role="status"]')?.textContent ?? '';

/** Moves the page to `episode` in a new session, as a switch lands:
 *  the page attaches the session's stream, and the element, loaded
 *  afresh, starts at zero and learns its length. */
async function switchTo(video: HTMLVideoElement, episode: number, session: string) {
	setUrl(`/play/${KITSU_ID}`, { session, episode: String(episode), kind: 'mp4' });
	await until(() => (video.getAttribute('src') ?? '').includes(session), `${session} attached`);
	video.currentTime = 0;
	video.dispatchEvent(new Event('loadedmetadata'));
	video.dispatchEvent(new Event('durationchange'));
	video.dispatchEvent(new Event('timeupdate'));
}

/** Counts every time the card is put on the page. */
function countMounts(): { readonly count: number; stop(): void } {
	let count = 0;
	const observer = new MutationObserver((records) => {
		for (const r of records)
			for (const n of r.addedNodes)
				if (n instanceof HTMLElement && (n.matches('.up-next') || n.querySelector('.up-next')))
					count++;
	});
	observer.observe(target, { childList: true, subtree: true });
	return {
		get count() {
			return count;
		},
		stop: () => observer.disconnect()
	};
}

/** Puts `el` in fullscreen, as the browser reports it, or leaves it with null. */
function setFullscreen(el: Element | null) {
	Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => el });
	document.dispatchEvent(new Event('fullscreenchange'));
}

describe('the Up next card at a finale', () => {
	it("names the next entry once the video ends, in the Skip Ending pill's place", async () => {
		const { video } = await openEpisode();
		await playTo(video, 1000);
		expect(card()).toBeNull();
		// Through the credits the pill shows as on any episode, and the
		// card waits for the end.
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		expect(card()).toBeNull();
		await playTo(video, 1395);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		expect(card()!.textContent).toContain(m.watchorder_upnext_eyebrow());
		expect(card()!.textContent).toContain('Saga: Final Arc');
		const links = cardLinks();
		expect(links.map((a) => a.getAttribute('href'))).toEqual(['/anime/50']);
		expect(links[0].getAttribute('aria-label')).toBe('Saga: Final Arc');
		expect(links[0].getAttribute('title')).toBe('Saga: Final Arc');
		expect(skipPill()).toBeNull();
		// It is announced once, politely, and never takes focus itself.
		await until(
			() =>
				card()!.querySelector('[role="status"]')?.textContent ===
				m.watchorder_upnext_announce({ title: 'Saga: Final Arc' }),
			'the announcement'
		);
		expect(card()!.contains(document.activeElement)).toBe(false);
	});

	it('stays at the end, goes on a seek back, and comes back at the end', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).not.toBeNull();
		await playTo(video, 1300);
		await until(() => card() === null, 'the card to go on a seek back into the credits');
		await playTo(video, 1395);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		await playToEnd(video);
		await until(() => card() !== null, 'the card back at the end');
	});

	it('goes as soon as a seek back starts, before the stream there has loaded', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		// The seek is under way: Chromium reports the new point on
		// `seeking`, and its `timeupdate` waits for the media there.
		video.currentTime = 600;
		video.dispatchEvent(new Event('seeking'));
		await until(() => card() === null, 'the card to go as the seek starts');
	});

	it('shows when the stream stops a moment short of its length without ending', async () => {
		const { video } = await openEpisode();
		// A precaution: where a stream has less media than the length it
		// declared, the playhead can stop just short of it, and the
		// element would never say it has ended.
		await playTo(video, LENGTH - 2);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		await playTo(video, LENGTH - 0.3);
		await until(() => card() !== null, 'the card at the stalled end');
	});

	it('shows when the element says it has ended, with a length it never learned', async () => {
		const { video } = await openEpisode();
		// The length is unknown, so the playhead alone decides nothing:
		// only the element's own end can bring the card.
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		video.dispatchEvent(new Event('durationchange'));
		await playTo(video, 1300);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		video.dispatchEvent(new Event('ended'));
		await until(() => card() !== null, 'the card on the ended event');
	});

	it("reads the element's ended state as it plays, and leaves it on a seek back", async () => {
		const { video } = await openEpisode();
		let ended = false;
		Object.defineProperty(video, 'ended', { configurable: true, get: () => ended });
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		video.dispatchEvent(new Event('durationchange'));
		ended = true;
		await playTo(video, 1400);
		await until(() => card() !== null, 'the card from the ended state');
		ended = false;
		await playTo(video, 600);
		await until(() => card() === null, 'the card to go on a seek back');
	});

	it("drops the last episode's end with its stream on a switch", async () => {
		const { video } = await openEpisode({ episode: 11 });
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		video.dispatchEvent(new Event('durationchange'));
		await playTo(video, 1400);
		video.dispatchEvent(new Event('ended'));
		await new Promise((r) => setTimeout(r, 0));
		const mounts = countMounts();
		try {
			setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '12', kind: 'mp4' });
			await until(
				() => (video.getAttribute('src') ?? '').includes('session-2'),
				'session-2 attached'
			);
			await new Promise((r) => setTimeout(r, 100));
			expect(mounts.count).toBe(0);
		} finally {
			mounts.stop();
		}
	});

	it('shows in the frame fullscreen the player controls use', async () => {
		const { video } = await openEpisode();
		const frame = target.querySelector('section.player-frame')!;
		try {
			setFullscreen(frame);
			await playToEnd(video);
			await until(() => card() !== null, 'the card in fullscreen');
			expect(frame.contains(card())).toBe(true);
		} finally {
			setFullscreen(null);
		}
	});

	it('names the next entry without asking the provider about it', async () => {
		const { video, fetches } = await openEpisode();
		const sentBefore = fetches.sent.length;
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		await fetches.settled('anything the card set off');
		const asked = fetches.sent.filter(
			(f) =>
				f.path.includes('/50') ||
				JSON.stringify(f.body).includes('Final Arc') ||
				JSON.stringify(f.body).includes('"50"')
		);
		expect(asked).toEqual([]);
		const after = fetches.sent.slice(sentBefore);
		expect(after.filter((f) => /^\/api\/(availability|play|kitsu\/search)/.test(f.path))).toEqual(
			[]
		);
	});

	it('plays nothing further at the end, even with auto-play on', async () => {
		const { video, fetches } = await openEpisode({ autoPlayNext: true });
		await playTo(video, 1395);
		const before = fetches.sent.length;
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		await fetches.settled('anything the end set off');
		expect(fetches.sent.slice(before).filter((f) => f.path.startsWith('/api/play'))).toEqual([]);
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
		expect(card()).not.toBeNull();
	});

	it('leaves the next-episode button within the entry', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const next = target.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
		);
		expect(next).not.toBeNull();
		expect(next!.disabled).toBe(true);
	});

	it('is dismissed by Esc while it has focus, for the rest of the episode', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const link = cardLinks()[0];
		link.focus();
		link.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await until(() => card() === null, 'the card to be dismissed');
		// Back into the credits and to the end again: still dismissed.
		await playTo(video, 1320);
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
	});

	it('is dismissed by its own button', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		card()!
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.watchorder_upnext_dismiss()}"]`)!
			.click();
		await until(() => card() === null, 'the card to be dismissed');
	});

	it("shows no card before the entry's last episode, where the pill shows as before", async () => {
		const { video } = await openEpisode({ episode: 11 });
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		expect(card()).toBeNull();
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
	});

	it('without skip times, shows at the end, not in the last minutes', async () => {
		const { video } = await openEpisode({ skips: [] });
		await playTo(video, LENGTH - 60);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		await playToEnd(video);
		await until(() => card() !== null, 'the card at the end');
	});

	it('follows the order setting', async () => {
		const { video } = await openEpisode({ order: 'released' });
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		expect(cardLinks().map((a) => a.getAttribute('href'))).toEqual(['/anime/5']);
		expect(card()!.textContent).toContain('Saga: Origins');
	});

	it('lists the ways on at a fork', async () => {
		const SPIN = entry('60', 'Saga: Other Path', '2015-01-01');
		const { video } = await openEpisode({
			franchise: franchise({
				entries: [SAGA, SECOND, FINAL, SPIN],
				links: [
					['10', KITSU_ID],
					[KITSU_ID, '50'],
					[KITSU_ID, '60']
				]
			})
		});
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		expect(card()!.textContent).toContain(m.watchorder_upnext_choices());
		expect(cardLinks().map((a) => a.getAttribute('href'))).toEqual(['/anime/50', '/anime/60']);
	});

	it('shows a next entry that has not aired with its date, and links nowhere', async () => {
		const LATER = entry('50', 'Saga: Final Arc', '2099-01-01', { episodes: null });
		const { video } = await openEpisode({
			franchise: franchise({ entries: [SAGA, SECOND, LATER] })
		});
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		expect(card()!.textContent).toContain(m.watchorder_upnext_coming());
		expect(card()!.textContent).toContain('Saga: Final Arc');
		expect(card()!.textContent).toContain('2099');
		expect(cardLinks()).toEqual([]);
	});

	it('shows no card where the franchise cannot be read, and the pill shows as before', async () => {
		const { video } = await openEpisode({ franchise: null });
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
	});

	it('shows no card where the franchise was read only in part, and the pill shows as before', async () => {
		const { video } = await openEpisode({ franchise: franchise({ complete: false }) });
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
	});

	it("gives the next entry's meta line the viewer's progress in it", async () => {
		const { video } = await openEpisode({
			history: [{ id: 'hianime:saga-final', title: 'Saga Final Arc', ep_no: '3', kitsu_id: '50' }]
		});
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		await until(
			() => (card()!.textContent ?? '').includes(m.watchorder_meta_progress_open({ episode: 3 })),
			'the progress'
		);
	});

	it("gives the next entry a row's progress when Kitsu deleted the entry the row recorded", async () => {
		// The row recorded entry 4, which Kitsu has since deleted; the
		// backend names it for Final Arc (50), the entry its Continue
		// Watching card was matched to, as the resume lookup does.
		const ROW = { id: 'hianime:saga-final', title: 'Saga Final Arc', ep_no: '3', kitsu_id: '4' };
		const { video } = await openEpisode({ history: [ROW], named: { '50': ROW } });
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		await until(
			() => (card()!.textContent ?? '').includes(m.watchorder_meta_progress_open({ episode: 3 })),
			"the replacement's progress"
		);
	});

	it('shows no card at the end of the run', async () => {
		const { video } = await openEpisode({
			franchise: franchise({
				links: [
					['10', KITSU_ID],
					['5', '10']
				],
				entries: [SAGA, SECOND, ORIGINS]
			})
		});
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
	});

	it("shows with the browser's own controls too", async () => {
		const { video } = await openEpisode({ nativeControls: true });
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
	});

	it('stays when the frame is fullscreen and its controls hide', async () => {
		const { video } = await openEpisode();
		Object.defineProperty(video, 'paused', { configurable: true, get: () => false });
		video.dispatchEvent(new Event('play'));
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const frame = target.querySelector('section.player-frame')!;
		try {
			setFullscreen(frame);
			await until(() => frame.classList.contains('fs-controls-hidden'), 'the controls to hide');
			expect(card()).not.toBeNull();
			expect(card()!.closest('.player-controls')).toBeNull();
		} finally {
			setFullscreen(null);
		}
	});

	it('is not put on the page while only the video is fullscreen', async () => {
		const { video } = await openEpisode();
		try {
			setFullscreen(video);
			await playToEnd(video);
			await new Promise((r) => setTimeout(r, 50));
			expect(card()).toBeNull();
		} finally {
			setFullscreen(null);
		}
		await until(() => card() !== null, 'the card once the video leaves fullscreen');
	});

	it.each([
		['appears without moving under reduced motion', true],
		['settles in, as the skip pill does, with motion on', false]
	] as const)('%s', async (_name, reduced) => {
		await setReducedMotion(reduced);
		const motion = recordMotion();
		try {
			const { video } = await openEpisode();
			await playToEnd(video);
			await until(() => card() !== null, 'the card');
			// A transition starts on the next frame.
			await new Promise((r) => setTimeout(r, 100));
			const moved = motion.animations.filter((a) => a.includes('up-next'));
			if (reduced) expect(moved).toEqual([]);
			else expect(moved.length).toBeGreaterThan(0);
		} finally {
			motion.stop();
		}
	});

	it("waits for the last episode the player reaches when the source lists more than Kitsu's count", async () => {
		const { video } = await openEpisode({ sourceEpisodes: 13 });
		await playTo(video, 1300);
		await until(() => skipPill() !== null, 'the Skip Ending pill');
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		const next = target.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
		);
		expect(next!.disabled).toBe(false);
	});

	it('shows on the episode past the count once nothing in the entry follows', async () => {
		const { video } = await openEpisode({ sourceEpisodes: 13, episode: 13 });
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
	});

	it('does not flash on a finale opened from the previous episode’s end', async () => {
		const { video } = await openEpisode({ episode: 11 });
		await playToEnd(video);
		const mounts = countMounts();
		try {
			await switchTo(video, 12, 'session-2');
			await new Promise((r) => setTimeout(r, 100));
			expect(mounts.count).toBe(0);
		} finally {
			mounts.stop();
		}
		// The finale's own end brings it.
		await playToEnd(video);
		await until(() => card() !== null, 'the card at the finale’s end');
	});

	it('waits for the resume hold, even at the end', async () => {
		const { video } = await openEpisode();
		// A point kept to resume from, as playback leaves one.
		await playTo(video, 1000);
		video.dispatchEvent(new Event('pause'));
		await switchTo(video, 12, 'session-2');
		const frame = target.querySelector('section.player-frame')!;
		await until(() => frame.getAttribute('aria-busy') === 'true', 'the resume hold');
		// The playhead reads the end while the hold is still on.
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		// The resume lands at its point, and playback goes on to the end.
		await playTo(video, 1000);
		video.dispatchEvent(new Event('seeked'));
		video.dispatchEvent(new Event('playing'));
		await until(() => frame.getAttribute('aria-busy') !== 'true', 'the resume to land');
		await playToEnd(video);
		await until(() => card() !== null, 'the card once the hold is over');
	});

	it('comes back when the episode is played again after a dismissal', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		card()!
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.watchorder_upnext_dismiss()}"]`)!
			.click();
		await until(() => card() === null, 'the card to be dismissed');
		await switchTo(video, 11, 'session-2');
		// A fresh start, not a resume: the resume hold keeps the card
		// back until playback is under way again.
		window.localStorage.clear();
		await switchTo(video, 12, 'session-3');
		await playToEnd(video);
		await until(() => card() !== null, 'the card on the new viewing');
	});

	it('is announced once per viewing, not again on its way back', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const said = m.watchorder_upnext_announce({ title: 'Saga: Final Arc' });
		await until(() => announcement() === said, 'the announcement');
		await playTo(video, 600);
		await until(() => card() === null, 'the card to go');
		await playToEnd(video);
		await until(() => card() !== null, 'the card again');
		await new Promise((r) => setTimeout(r, 50));
		expect(announcement()).toBe('');
	});

	it('gives focus back where it came from when Esc dismisses it', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const before = target.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
		)!;
		before.disabled = false;
		before.focus();
		cardLinks()[0].focus();
		cardLinks()[0].dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await until(() => card() === null, 'the card to be dismissed');
		expect(document.activeElement).toBe(before);
	});

	it('shows on an extra after the last episode, which has no skip times to ask for', async () => {
		const { video, fetches } = await openEpisode({ episode: 12.5, extras: ['12.5'] });
		expect(fetches.sent.some((f) => f.path.startsWith('/api/aniskip/'))).toBe(false);
		await playTo(video, LENGTH - 60);
		await new Promise((r) => setTimeout(r, 50));
		expect(card()).toBeNull();
		await playToEnd(video);
		await until(() => card() !== null, 'the card at the extra’s end');
	});

	it('leaves focus alone when the close button is clicked with the mouse', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		const control = target.querySelector<HTMLButtonElement>('.player-controls button')!;
		control.focus();
		const close = card()!.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.watchorder_upnext_dismiss()}"]`
		)!;
		close.focus();
		close.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
		await until(() => card() === null, 'the card to be dismissed');
		expect(target.querySelector('.player-controls')!.contains(document.activeElement)).toBe(false);
	});

	it('stays dismissed, and unannounced, when the same episode is opened again by a recovery', async () => {
		const { video } = await openEpisode();
		await playToEnd(video);
		await until(() => card() !== null, 'the card');
		card()!
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.watchorder_upnext_dismiss()}"]`)!
			.click();
		await until(() => card() === null, 'the card to be dismissed');
		await switchTo(video, 12, 'session-2');
		// A recovery resumes where playback was: let that land.
		video.dispatchEvent(new Event('seeked'));
		video.dispatchEvent(new Event('playing'));
		const frame = target.querySelector('section.player-frame')!;
		await until(() => frame.getAttribute('aria-busy') !== 'true', 'the resume to land');
		await playToEnd(video);
		await new Promise((r) => setTimeout(r, 50));
		expect(frame.classList.contains('player-resuming')).toBe(false);
		expect(card()).toBeNull();
	});

	it("asks for the next episode's skip times with that episode's own length", async () => {
		const { video, fetches } = await openEpisode({ episode: 11 });
		const before = fetches.sent.length;
		Object.defineProperty(video, 'duration', { configurable: true, get: () => 1500 });
		await switchTo(video, 12, 'session-2');
		await fetches.settled('the skip times for episode 12');
		const asked = fetches.sent
			.slice(before)
			.filter((f) => f.path.startsWith(`/api/aniskip/${KITSU_ID}/12`));
		expect(asked.map((f) => f.query.get('episode_length'))).toEqual(['1500']);
	});

	it("does not carry the last episode's skip times into the next before its own arrive", async () => {
		const { video } = await openEpisode({
			episode: 11,
			autoSkipOp: true,
			skips: [{ skip_type: 'op', start_time: 0, end_time: 90 }]
		});
		await playTo(video, 600);
		// Episode 12 opens with no opening, and its stream has not
		// learned its length yet.
		server.use(http.get(`${API_BASE}/api/aniskip/:id/12`, () => HttpResponse.json([])));
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '12', kind: 'mp4' });
		await until(
			() => (video.getAttribute('src') ?? '').includes('session-2'),
			'session-2 attached'
		);
		// The new stream starts at zero and has not learned its length.
		video.currentTime = 0;
		video.dispatchEvent(new Event('timeupdate'));
		await new Promise((r) => setTimeout(r, 50));
		expect(video.currentTime).toBe(0);
		expect(skipPill()).toBeNull();
	});

	it("asks for an episode's skip times again on coming back to it before the next one learned its length", async () => {
		const { video, fetches } = await openEpisode({ episode: 11 });
		await playTo(video, 62);
		await until(() => skipPill() !== null, 'the Skip Intro pill on episode 11');
		// Episode 12 opens, and leaves before its stream learns its length.
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		await switchTo(video, 12, 'session-2');
		await new Promise((r) => setTimeout(r, 20));
		// Back on episode 11, whose length its stream then learns.
		Object.defineProperty(video, 'duration', { configurable: true, get: () => LENGTH });
		await switchTo(video, 11, 'session-3');
		await fetches.settled('the skip times for episode 11 again');
		// The episode resumes where it was left: let that land.
		video.dispatchEvent(new Event('seeked'));
		video.dispatchEvent(new Event('playing'));
		await playTo(video, 62);
		await until(() => skipPill() !== null, 'the Skip Intro pill on episode 11 again');
	});

	it("keeps an episode's skip times when an earlier request for them fails late", async () => {
		const { video } = await openEpisode({ episode: 10 });
		// Episode 11's first request is held, and fails once released; its
		// second answers.
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let asked = 0;
		server.use(
			http.get(`${API_BASE}/api/aniskip/${KITSU_ID}/11`, async () => {
				asked++;
				if (asked === 1) {
					await held;
					return new HttpResponse(null, { status: 503 });
				}
				return HttpResponse.json([OPENING, ENDING]);
			})
		);
		await switchTo(video, 11, 'session-2');
		await until(() => asked === 1, "episode 11's first request");
		// Episode 12 opens and is left before its stream learns its length.
		Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
		await switchTo(video, 12, 'session-3');
		await new Promise((r) => setTimeout(r, 20));
		Object.defineProperty(video, 'duration', { configurable: true, get: () => LENGTH });
		await switchTo(video, 11, 'session-4');
		await until(() => asked === 2, "episode 11's second request");
		video.dispatchEvent(new Event('seeked'));
		video.dispatchEvent(new Event('playing'));
		await playTo(video, 62);
		await until(() => skipPill() !== null, 'the Skip Intro pill from the second answer');
		release();
		await new Promise((r) => setTimeout(r, 50));
		await playTo(video, 63);
		expect(skipPill()).not.toBeNull();
	});
});
