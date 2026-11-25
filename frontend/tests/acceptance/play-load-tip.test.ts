// Acceptance: the player's load tip — a feature tip shown while a
// stream loads, before playback starts (the harness is the start
// veil's, play-start-veil.test.ts).
//
// It shows once a start has lasted a second, is announced, and never
// takes focus, so the player's keys keep working; a faster start never
// shows it. When the first frame comes it stays, folded to a plate,
// until Got it or Escape close it, seen. An open player menu takes
// Escape first. An error takes it away, still pending.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

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
import { playerVideo, playerVideoInSlot } from './player-video';
import { readPosition } from '../../src/lib/play/watch-position';
import { m } from '../../src/lib/paraglide/messages';
import { __resetOnboardingForTests } from '../../src/lib/onboarding/tips.svelte';

import type {
	OnboardingView,
	AiringStatus,
	AvailabilityResponse,
	Config,
	KitsuAnimeRef,
	KitsuEpisode,
	SkipInterval
} from '../../src/lib/api';
import { availabilityAnswer, episodeAnswer } from '../answers';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

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
	__resetPlayCacheForTests();
	window.localStorage.clear();
	vi.mocked(goto).mockClear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	setParams({ id: KITSU_ID });
	__resetOnboardingForTests();
	useTip();
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
	throw new Error(`timed out waiting for ${what}
--- DOM ---
${target.textContent}`);
}

function kitsuEpisodes(count: number): KitsuEpisode[] {
	return Array.from({ length: count }, (_, i) =>
		episodeAnswer({
			id: `ep-${i + 1}`,
			number: i + 1,
			relative_number: i + 1,
			canonical_title: `Episode ${i + 1}`,
			thumbnail: { original: `https://media.kitsu.app/episodes/thumbnails/${i + 1}/original.jpg` },
			length: 24,
			airdate: '2019-01-01'
		})
	);
}

function useShowHandlers(opts: { nativeControls?: boolean; skips?: SkipInterval[] } = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json<Config>({
				...appConfig(),
				use_custom_player_controls: !opts.nativeControls
			})
		),
		http.post(`${API_BASE}/api/play/external`, () => new HttpResponse(null, { status: 202 })),
		http.post(`${API_BASE}/api/play/syncplay`, () => new HttpResponse(null, { status: 202 })),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json<KitsuAnimeRef>({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json<AiringStatus>({
				aired: 12,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json<KitsuEpisode[]>(kitsuEpisodes(12))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
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
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () =>
			HttpResponse.json<SkipInterval[]>(opts.skips ?? [])
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json<string | null>('/dl'))
	);
}

const frame = () => target.querySelector('section.player-frame') as HTMLElement;
/** The custom controls' play / pause button, whatever it is labelled. */
const playPause = () => target.querySelector('.pc-buttons-row .pc-btn') as HTMLButtonElement | null;
const settle = () => new Promise((r) => setTimeout(r, 20));

/** The page's video with its media state under the scenario's hand:
 *  `paused` and `readyState` as the scenario sets them, `pause()`
 *  counted, and events fired as Chromium fires them. */
function controlled(video: HTMLVideoElement) {
	let paused = true;
	let readyState = 0;
	Object.defineProperty(video, 'paused', { configurable: true, get: () => paused });
	Object.defineProperty(video, 'readyState', { configurable: true, get: () => readyState });
	const pause = vi.fn(() => {
		const was = paused;
		paused = true;
		if (!was) video.dispatchEvent(new Event('pause'));
	});
	const play = vi.fn(() => {
		paused = false;
		video.dispatchEvent(new Event('play'));
		return Promise.resolve();
	});
	video.pause = pause;
	video.play = play as unknown as HTMLVideoElement['play'];
	return {
		pause,
		play,
		/** The first frame plays: autoplay sets paused false, then `play`, `playing`. */
		firstFrame() {
			paused = false;
			readyState = 4;
			video.dispatchEvent(new Event('play'));
			video.dispatchEvent(new Event('playing'));
		},
		fire(name: string) {
			video.dispatchEvent(new Event(name));
		},
		set(next: { paused?: boolean; readyState?: number }) {
			paused = next.paused ?? paused;
			readyState = next.readyState ?? readyState;
		}
	};
}

async function openEpisode(sessionId = 'session-1', episode = '1') {
	useShowHandlers();
	return openMounted(sessionId, episode);
}

async function openMounted(sessionId: string, episode: string) {
	setUrl(`/play/${KITSU_ID}`, { session: sessionId, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => playPause() !== null, 'the custom controls');
	const media = controlled(playerVideo());
	// The source has attached once it marked the episode started.
	await until(() => readPosition(KITSU_ID, 1) === 0, 'the stream to attach');
	await settle();
	return media;
}

let seen: string[] = [];
function useTip(pending = true) {
	seen = [];
	server.use(
		http.get(`${API_BASE}/api/onboarding`, () =>
			HttpResponse.json<OnboardingView>({
				version: '0.19.0',
				tips: [
					{
						id: 'player-up-next',
						kind: 'single',
						surface: 'play',
						moment: 'stream-load',
						since: '0.19.0',
						places: ['player-frame'],
						how_to: null,
						pending
					}
				]
			})
		),
		http.post(`${API_BASE}/api/onboarding/seen`, async ({ request }) => {
			seen.push(((await request.json()) as { id: string }).id);
			return new HttpResponse(null, { status: 204 });
		})
	);
}

const loadTip = () => target.querySelector('[data-testid="onboarding-load-tip"]');
const phase = () => loadTip()?.querySelector('.note')?.getAttribute('data-phase') ?? null;
const said = () =>
	target.querySelector('.load-tip-host [role="status"]')?.textContent?.trim() ?? '';
const gotIt = () =>
	[...(loadTip()?.querySelectorAll('button') ?? [])].find(
		(b) => b.textContent?.trim() === m.onboarding_got_it()
	) as HTMLButtonElement | undefined;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const escape = () =>
	(document.activeElement ?? document.body).dispatchEvent(
		new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
	);

describe('play route — the load tip', () => {
	it('shows once a start lasts a second, is announced, and leaves focus where it was', async () => {
		await openEpisode();
		const before = document.activeElement;
		await wait(500);
		expect(loadTip()).toBeNull();
		await until(() => loadTip() !== null, 'the load tip', 3000);
		expect(phase()).toBe('loading');
		expect(loadTip()!.textContent).toContain(m.onboarding_player_up_next_1_title());
		expect(said()).toBe(
			m.onboarding_announce({ version: '0.19.0', title: m.onboarding_player_up_next_1_title() })
		);
		expect(document.activeElement).toBe(before);
	});

	it('a start that lands sooner never shows it', async () => {
		const media = await openEpisode();
		media.firstFrame();
		await wait(1500);
		expect(loadTip()).toBeNull();
		expect(seen).toEqual([]);
	});

	it('stays as a plate once the picture plays, until Got it closes it seen', async () => {
		const media = await openEpisode();
		await until(() => loadTip() !== null, 'the load tip', 3000);
		media.firstFrame();
		await until(() => phase() === 'playing', 'the plate');
		await wait(300);
		expect(phase()).toBe('playing');
		gotIt()!.click();
		await until(() => loadTip() === null, 'closed');
		await until(() => seen.length === 1, 'the close sent');
		expect(seen).toEqual(['player-up-next']);
	});

	it('Escape closes it, but an open menu takes Escape first', async () => {
		await openEpisode();
		await until(() => loadTip() !== null, 'the load tip', 3000);
		const menu = document.createElement('button');
		menu.setAttribute('aria-expanded', 'true');
		document.body.appendChild(menu);
		escape();
		await wait(50);
		expect(loadTip()).not.toBeNull();
		menu.remove();
		escape();
		await until(() => loadTip() === null, 'closed by Escape');
		await until(() => seen.length === 1, 'the close sent');
	});

	it('focus on the tip as it goes moves to the player, not the page', async () => {
		await openEpisode();
		await until(() => loadTip() !== null, 'the load tip', 3000);
		gotIt()!.focus();
		gotIt()!.click();
		await until(() => loadTip() === null, 'closed');
		expect(frame().contains(document.activeElement)).toBe(true);
	});

	it('a tip already seen never shows', async () => {
		useTip(false);
		await openEpisode();
		await wait(1600);
		expect(loadTip()).toBeNull();
	});
});
