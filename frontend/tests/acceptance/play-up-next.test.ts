// Acceptance: the player's "Up next" card at a finale.
//
// On the last episode of an entry that has finished airing, once the
// ending credits begin, the Skip Ending pill's corner holds a card
// naming the entry that comes next in the run, in the order the
// setting chooses. It links to that entry's page and waits there: the
// end of the video plays nothing further, and the next-episode button
// stays within the entry. The card names the next entry without
// asking the provider anything about it. A fork lists the ways on; a
// next entry that has not aired shows when it airs and links nowhere.
// Esc dismisses the card while it has focus. It stays clear of the
// controls that hide in fullscreen, and under reduced motion it simply
// appears.

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

function franchise(over: Partial<{ entries: unknown[]; links: string[][] }> = {}) {
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
}

/** Opens `options.episode` of Second Arc and gives its video a length. */
async function openEpisode(options: Options = {}) {
	const fetches = watchPageFetches();
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({
				...appConfig(),
				watch_order: options.order ?? 'linked',
				auto_play_next: options.autoPlayNext === true,
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
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: EPISODES, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(EPISODES))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
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

const card = () => target.querySelector<HTMLElement>('.up-next');
const cardLinks = () => [...(card()?.querySelectorAll<HTMLAnchorElement>('a') ?? [])];
const skipPill = () => target.querySelector('.player-skip-btn');

describe('the Up next card at a finale', () => {
	it("names the next entry once the credits begin, in the Skip Ending pill's place", async () => {
		const { video } = await openEpisode();
		await playTo(video, 1000);
		expect(card()).toBeNull();
		await playTo(video, 1300);
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

	it('stays through the credits, past them and at the end, and goes on a seek back', async () => {
		const { video } = await openEpisode();
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		await playTo(video, 1395);
		expect(card()).not.toBeNull();
		await playTo(video, LENGTH);
		video.dispatchEvent(new Event('ended'));
		await new Promise((r) => setTimeout(r, 0));
		expect(card()).not.toBeNull();
		await playTo(video, 600);
		await until(() => card() === null, 'the card to go on a seek back');
	});

	it('names the next entry without asking the provider about it', async () => {
		const { video, fetches } = await openEpisode();
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		await fetches.settled('anything the card set off');
		const asked = fetches.sent.filter(
			(f) =>
				f.path.includes('/50') ||
				JSON.stringify(f.body).includes('Final Arc') ||
				JSON.stringify(f.body).includes('"50"')
		);
		expect(asked).toEqual([]);
	});

	it('plays nothing further at the end, even with auto-play on', async () => {
		const { video, fetches } = await openEpisode({ autoPlayNext: true });
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		const before = fetches.sent.length;
		await playTo(video, LENGTH);
		video.dispatchEvent(new Event('ended'));
		await fetches.settled('anything the end set off');
		expect(fetches.sent.slice(before).filter((f) => f.path.startsWith('/api/play'))).toEqual([]);
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
		expect(card()).not.toBeNull();
	});

	it('leaves the next-episode button within the entry', async () => {
		const { video } = await openEpisode();
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		const next = target.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
		);
		expect(next).not.toBeNull();
		expect(next!.disabled).toBe(true);
	});

	it('is dismissed by Esc while it has focus, for the rest of the episode', async () => {
		const { video } = await openEpisode();
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		const link = cardLinks()[0];
		link.focus();
		link.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await until(() => card() === null, 'the card to be dismissed');
		await playTo(video, 1320);
		expect(card()).toBeNull();
	});

	it('is dismissed by its own button', async () => {
		const { video } = await openEpisode();
		await playTo(video, 1300);
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
	});

	it('without skip times, waits for the last minutes', async () => {
		const { video } = await openEpisode({ skips: [] });
		await playTo(video, 1300);
		expect(card()).toBeNull();
		await playTo(video, LENGTH - 60);
		await until(() => card() !== null, 'the card in the last minutes');
	});

	it('follows the order setting', async () => {
		const { video } = await openEpisode({ order: 'released' });
		await playTo(video, 1300);
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
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		expect(card()!.textContent).toContain(m.watchorder_upnext_choices());
		expect(cardLinks().map((a) => a.getAttribute('href'))).toEqual(['/anime/50', '/anime/60']);
	});

	it('shows a next entry that has not aired with its date, and links nowhere', async () => {
		const LATER = entry('50', 'Saga: Final Arc', '2099-01-01', { episodes: null });
		const { video } = await openEpisode({
			franchise: franchise({ entries: [SAGA, SECOND, LATER] })
		});
		await playTo(video, 1300);
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
		expect(card()).toBeNull();
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
		expect(card()).toBeNull();
	});

	it("shows with the browser's own controls too", async () => {
		const { video } = await openEpisode({ nativeControls: true });
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
	});

	it('is not among the controls fullscreen hides', async () => {
		const { video } = await openEpisode();
		await playTo(video, 1300);
		await until(() => card() !== null, 'the card');
		expect(card()!.closest('.player-controls')).toBeNull();
		expect(card()!.closest('section.player-frame')).not.toBeNull();
	});

	it.each([
		['appears without moving under reduced motion', true],
		['settles in, as the skip pill does, with motion on', false]
	] as const)('%s', async (_name, reduced) => {
		await setReducedMotion(reduced);
		const motion = recordMotion();
		try {
			const { video } = await openEpisode();
			await playTo(video, 1300);
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
});
