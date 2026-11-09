// Acceptance: the skip times across an episode switch in the player.
//
// A switch — the Next or Previous button, a click in the episode
// strip, auto-play at the end of an episode — replaces the stream on
// screen with the new episode's. The skip times of the episode that
// was playing go with it: no Skip pill shows at its times, and with
// automatic skipping on nothing moves the new episode's playhead by
// them. The new episode's own times are asked for once its stream has
// learned its length, with that length, since aniskip matches its
// submissions to it. A switch that fails leaves the old stream on
// screen with its times.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { playerVideo, playerVideoInSlot } from './player-video';
import { m } from '../../src/lib/paraglide/messages';
import { savePosition } from '../../src/lib/play/watch-position';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
// A landed switch navigates to the episode it played; following it
// puts the page on that episode as the real router would.
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async (href: string) => {
		const next = new URL(href, 'http://127.0.0.1:31337');
		setUrl(next.pathname, Object.fromEntries(next.searchParams));
	}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';
const TITLE = 'Long Show';
const EPISODES = 12;
/** Each episode's own length: no two the same, so a length asked with
 *  names the stream it came from. */
const lengthOf = (episode: number) => 1400 + episode * 10;

type Skip = { skip_type: string; start_time: number; end_time: number };
const op = (start: number, end: number): Skip => ({
	skip_type: 'op',
	start_time: start,
	end_time: end
});

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	landed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
/** Every skip-times request, as the backend received it. */
let asks: { episode: string; length: string | null }[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	asks = [];
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

/** Lets every effect a dispatched event set off run. */
const settle = () => new Promise((r) => setTimeout(r, 30));

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
	episode: number;
	autoSkipOp?: boolean;
	autoPlayNext?: boolean;
	extras?: string[];
	/** The skip times per episode; a function answers the request
	 *  itself (late, failing). Episodes not named answer none. */
	skips: Record<string, Skip[] | (() => Promise<Response> | Response)>;
}

function serve(options: Options) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({
				...appConfig(),
				auto_skip_op: options.autoSkipOp === true,
				auto_play_next: options.autoPlayNext === true,
				use_custom_player_controls: true
			})
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
		),
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
				extra_episodes: options.extras ?? [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, ({ request, params }) => {
			const episode = String(params.episode);
			asks.push({
				episode,
				length: new URL(request.url).searchParams.get('episode_length')
			});
			const answer = options.skips[episode];
			if (typeof answer === 'function') return answer();
			return HttpResponse.json(answer ?? []);
		})
	);
}

/** The element as a fresh load leaves it, then as it learns the
 *  stream's length. */
function startStream(video: HTMLVideoElement) {
	video.currentTime = 0;
	Object.defineProperty(video, 'duration', { configurable: true, get: () => Number.NaN });
	video.dispatchEvent(new Event('timeupdate'));
}
function learnLength(video: HTMLVideoElement, length: number) {
	Object.defineProperty(video, 'duration', { configurable: true, get: () => length });
	video.dispatchEvent(new Event('loadedmetadata'));
	video.dispatchEvent(new Event('durationchange'));
}

/** Moves the playhead to `at`, as playback reaching it would. */
async function playTo(video: HTMLVideoElement, at: number) {
	video.currentTime = at;
	video.dispatchEvent(new Event('timeupdate'));
	await settle();
}

const skipPill = () => target.querySelector('.player-skip-btn');
const timeLabel = () => target.querySelector('.pc-time')?.textContent?.replace(/\s+/g, ' ').trim();
const nextButton = () =>
	target.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
	);
const prevButton = () =>
	target.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_episode_nav_previous_aria_label()}"]`
	);
const stripCard = (n: number | string) =>
	target.querySelector<HTMLButtonElement>(`li[data-ep-num="${n}"] button`);

/** Opens `options.episode` and lets its stream learn its length and
 *  its skip times arrive. */
async function openEpisode(options: Options) {
	serve(options);
	setUrl(`/play/${KITSU_ID}`, {
		session: 'session-1',
		episode: String(options.episode),
		kind: 'mp4'
	});
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	const video = playerVideo();
	video.play = (() => Promise.resolve()) as unknown as HTMLVideoElement['play'];
	learnLength(video, lengthOf(options.episode));
	await until(
		() => asks.some((a) => a.episode === String(options.episode)),
		`episode ${options.episode}'s skip times asked for`
	);
	await settle();
	return video;
}

/** The page's stream for `episode` not yet landed, if it has one: a
 *  switch may take over the stream warmed for its episode. */
function openStreamFor(episode: number | string): FakeEventSource | undefined {
	return FakeEventSource.instances
		.filter((i) => !i.landed && new URL(i.url).searchParams.get('episode') === `${episode}`)
		.at(-1);
}

let sessions = 1;
/** Lands the switch's stream for `episode`, and waits for the page to
 *  attach it. The element, loaded afresh, is at zero and has not
 *  learned its length yet. The page may have warmed a stream for the
 *  episode beside the switch's own, so every one open is landed. */
async function landSwitch(video: HTMLVideoElement, episode: number | string) {
	const landed: string[] = [];
	await until(() => {
		const es = openStreamFor(episode);
		if (es) {
			const session = `session-${++sessions}`;
			landed.push(session);
			es.landed = true;
			es.dispatch(
				'done',
				JSON.stringify({
					session_id: session,
					media_url: `${API_BASE}/s/${session}/video.mp4`,
					media_kind: 'mp4',
					episode: `${episode}`
				})
			);
		}
		const src = video.getAttribute('src') ?? '';
		return landed.some((s) => src.includes(`/${s}/`));
	}, `the stream for episode ${episode} attached`);
	startStream(video);
	await settle();
}

type Way = 'next' | 'prev' | 'strip' | 'auto-play';

/** Switches from the episode on screen by `way`. */
async function switchBy(way: Way, video: HTMLVideoElement, to: number | string) {
	if (way === 'next') {
		await until(() => nextButton()?.disabled === false, 'Next enabled');
		nextButton()!.click();
	} else if (way === 'prev') {
		await until(() => prevButton()?.disabled === false, 'Previous enabled');
		prevButton()!.click();
	} else if (way === 'strip') {
		await until(() => stripCard(to) !== null, `the strip card for episode ${to}`);
		stripCard(to)!.click();
	} else {
		await until(() => nextButton()?.disabled === false, 'the next episode reachable');
		video.dispatchEvent(new Event('ended'));
	}
	return landSwitch(video, to);
}

const WAYS: { way: Way; from: number; to: number }[] = [
	{ way: 'next', from: 11, to: 12 },
	{ way: 'auto-play', from: 11, to: 12 },
	{ way: 'strip', from: 11, to: 12 },
	{ way: 'prev', from: 12, to: 11 }
];

describe('play route — skip times across an episode switch', () => {
	for (const { way, from, to } of WAYS) {
		describe(`a switch by ${way}`, () => {
			it("asks for the new episode's times once with its own length", async () => {
				const video = await openEpisode({
					episode: from,
					autoPlayNext: way === 'auto-play',
					skips: { [from]: [op(0, 90)], [to]: [op(30, 120)] }
				});
				expect(asks).toEqual([{ episode: String(from), length: String(lengthOf(from)) }]);
				await switchBy(way, video, to);
				// Not before the new stream knows its length.
				expect(asks.filter((a) => a.episode === String(to))).toEqual([]);
				learnLength(video, lengthOf(to));
				await until(() => asks.some((a) => a.episode === String(to)), 'the new times asked');
				await settle();
				expect(asks.filter((a) => a.episode === String(to))).toEqual([
					{ episode: String(to), length: String(lengthOf(to)) }
				]);
			});

			for (const autoSkip of [true, false]) {
				it(`does not act on the last episode's times before its own arrive (auto-skip ${autoSkip ? 'on' : 'off'})`, async () => {
					const video = await openEpisode({
						episode: from,
						autoSkipOp: autoSkip,
						autoPlayNext: way === 'auto-play',
						skips: { [from]: [op(0, 90)], [to]: [op(30, 120)] }
					});
					await playTo(video, 600);
					await switchBy(way, video, to);
					// The new stream at its start, inside the last episode's opening.
					await playTo(video, 1);
					expect(video.currentTime).toBe(1);
					expect(skipPill()).toBeNull();
				});

				it(`acts on the new episode's own times once they arrive (auto-skip ${autoSkip ? 'on' : 'off'})`, async () => {
					const video = await openEpisode({
						episode: from,
						autoSkipOp: autoSkip,
						autoPlayNext: way === 'auto-play',
						skips: { [from]: [op(0, 90)], [to]: [op(30, 120)] }
					});
					await playTo(video, 600);
					await switchBy(way, video, to);
					learnLength(video, lengthOf(to));
					await until(() => asks.some((a) => a.episode === String(to)), 'the new times asked');
					await settle();
					// Inside the last episode's opening only: nothing.
					await playTo(video, 10);
					expect(video.currentTime).toBe(10);
					expect(skipPill()).toBeNull();
					// Inside the new episode's opening.
					await playTo(video, 31);
					if (autoSkip) {
						expect(video.currentTime).toBeCloseTo(120.05);
					} else {
						expect(video.currentTime).toBe(31);
						expect(skipPill()).not.toBeNull();
					}
				});
			}
		});
	}

	it("shows no times while the new episode's are late, then its own", async () => {
		let release: () => void = () => {};
		const late = new Promise<void>((r) => (release = r));
		const video = await openEpisode({
			episode: 11,
			autoSkipOp: true,
			skips: {
				11: [op(0, 90)],
				12: async () => {
					await late;
					return HttpResponse.json([op(30, 120)]);
				}
			}
		});
		await switchBy('next', video, 12);
		learnLength(video, lengthOf(12));
		await until(() => asks.some((a) => a.episode === '12'), 'the new times asked');
		await playTo(video, 5);
		expect(video.currentTime).toBe(5);
		expect(skipPill()).toBeNull();
		release();
		await settle();
		await playTo(video, 31);
		expect(video.currentTime).toBeCloseTo(120.05);
	});

	it("shows no times while the new episode's never arrive", async () => {
		// Held for the whole case, and answered only as it ends, so the
		// request does not outlive the page.
		let release: () => void = () => {};
		const never = new Promise<void>((r) => (release = r));
		const video = await openEpisode({
			episode: 11,
			autoSkipOp: true,
			skips: {
				11: [op(0, 90)],
				12: async () => {
					await never;
					return HttpResponse.json([]);
				}
			}
		});
		await switchBy('next', video, 12);
		learnLength(video, lengthOf(12));
		await until(() => asks.some((a) => a.episode === '12'), 'the new times asked');
		await playTo(video, 5);
		expect(video.currentTime).toBe(5);
		expect(skipPill()).toBeNull();
		release();
		await settle();
	});

	it("shows no times when the new episode's request fails", async () => {
		const video = await openEpisode({
			episode: 11,
			autoSkipOp: true,
			skips: {
				11: [op(0, 90)],
				12: () => HttpResponse.json({ error: 'error.aniskip.unreachable' }, { status: 502 })
			}
		});
		await switchBy('next', video, 12);
		learnLength(video, lengthOf(12));
		await until(() => asks.some((a) => a.episode === '12'), 'the new times asked');
		await settle();
		await playTo(video, 5);
		expect(video.currentTime).toBe(5);
		expect(skipPill()).toBeNull();
	});

	it('shows no times on an extra, which has none to ask for', async () => {
		const video = await openEpisode({
			episode: 12,
			autoSkipOp: true,
			extras: ['12.5'],
			skips: { 12: [op(0, 90)] }
		});
		await switchBy('strip', video, 12.5);
		learnLength(video, 600);
		await settle();
		expect(asks.map((a) => a.episode)).toEqual(['12']);
		await playTo(video, 5);
		expect(video.currentTime).toBe(5);
		expect(skipPill()).toBeNull();
	});

	it("leaves a resumed episode at its point, not moved by the last episode's opening", async () => {
		savePosition(KITSU_ID, 12, 50, lengthOf(12));
		const video = await openEpisode({
			episode: 11,
			autoSkipOp: true,
			skips: { 11: [op(0, 90)], 12: [] }
		});
		await switchBy('next', video, 12);
		learnLength(video, lengthOf(12));
		expect(video.currentTime).toBe(50);
		video.dispatchEvent(new Event('seeked'));
		video.dispatchEvent(new Event('playing'));
		const frame = target.querySelector('section.player-frame')!;
		await until(() => frame.getAttribute('aria-busy') !== 'true', 'the resume to land');
		await playTo(video, 50);
		expect(video.currentTime).toBe(50);
		expect(skipPill()).toBeNull();
	});

	it('asks again, with its own length, for an episode switched back to', async () => {
		const video = await openEpisode({
			episode: 11,
			skips: { 11: [op(0, 90)], 12: [op(30, 120)] }
		});
		await switchBy('next', video, 12);
		learnLength(video, lengthOf(12));
		await until(() => asks.some((a) => a.episode === '12'), "episode 12's times asked");
		await switchBy('prev', video, 11);
		// Episode 12's times are gone with its stream.
		await playTo(video, 31);
		expect(skipPill()).toBeNull();
		learnLength(video, 1395);
		await until(() => asks.filter((a) => a.episode === '11').length === 2, "11's asked again");
		expect(asks.at(-1)).toEqual({ episode: '11', length: '1395' });
		await settle();
		await playTo(video, 1);
		expect(skipPill()).not.toBeNull();
	});

	it('keeps the times of an episode a recovery opens again, without asking again', async () => {
		const video = await openEpisode({ episode: 11, skips: { 11: [op(30, 120)] } });
		// A recovery lands the same episode on a new session.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-r', episode: '11', kind: 'mp4' });
		await until(() => (video.getAttribute('src') ?? '').includes('session-r'), 'recovered');
		startStream(video);
		learnLength(video, lengthOf(11));
		await settle();
		expect(asks).toHaveLength(1);
		await playTo(video, 31);
		expect(skipPill()).not.toBeNull();
	});

	it('keeps the old stream and its times when the switch fails', async () => {
		const video = await openEpisode({
			episode: 11,
			skips: { 11: [op(30, 120)], 12: [op(300, 400)] }
		});
		await until(() => nextButton()?.disabled === false, 'Next enabled');
		nextButton()!.click();
		await until(() => nextButton()?.disabled === true, 'the switch under way');
		// Every stream the switch could take fails.
		await until(() => {
			const es = openStreamFor(12);
			if (es) {
				es.landed = true;
				es.dispatch(
					'error',
					JSON.stringify({ kind: 'upstream', status: 503, key: 'error.network.upstream' })
				);
			}
			return nextButton()?.disabled === false;
		}, 'the switch to fail');
		await settle();
		// The play failure is on screen; the old stream plays under it.
		expect(video.getAttribute('src')).toContain('/session-1/');
		expect(page.url.searchParams.get('session')).toBe('session-1');
		expect(asks.map((a) => a.episode)).toEqual(['11']);
		await playTo(video, 31);
		expect(skipPill()).not.toBeNull();
	});

	it("shows the new episode's playhead and no length, not the last one's, before it learns its own", async () => {
		const video = await openEpisode({ episode: 11, skips: { 11: [] } });
		await playTo(video, 600);
		expect(timeLabel()).toBe('10:00 / 25:10');
		await switchBy('next', video, 12);
		expect(timeLabel()).toBe('0:00 / 0:00');
		learnLength(video, lengthOf(12));
		await settle();
		expect(timeLabel()).toBe('0:00 / 25:20');
	});
});
