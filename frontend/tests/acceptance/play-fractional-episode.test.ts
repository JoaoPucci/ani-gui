// Acceptance: the play page on a fractional extra is about that
// extra.
//
// A recap the provider numbers 7.5 is a playable tile, and its play
// URL carries `episode=7.5`. The page read that as a whole number, so
// on the recap it said "Episode 7", handed episode 7 to the external
// player, and stepped to 6 and 8 from the wrong place. These cases
// mount the page on 7.5 and read what it shows and sends.
//
// The stream is an mp4 session: an HLS one would ask hls.js for a
// MediaSource happy-dom does not have.

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
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
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

type Posted = Record<string, unknown>;

interface Seen {
	external: Posted[];
	aniskip: string[];
}

function useHandlers(extras: string[] = ['7.5']): Seen {
	const seen: Seen = { external: [], aniskip: [] };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: extras,
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, ({ params }) => {
			seen.aniskip.push(String(params.episode));
			return HttpResponse.json([]);
		}),
		http.post(`${API_BASE}/api/play/external`, async ({ request }) => {
			seen.external.push((await request.json()) as Posted);
			return new HttpResponse(null, { status: 202 });
		})
	);
	return seen;
}

async function mountOnRecap() {
	setUrl(`/play/${KITSU_ID}`, { session: 'session-recap', episode: '7.5', kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
}

const navButton = (label: string) =>
	Array.from(target.querySelectorAll('button.ep-btn')).find(
		(b) => b.getAttribute('aria-label') === label
	) as HTMLButtonElement | undefined;

const moreButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_more_aria_label()}"]`
	) as HTMLButtonElement | null;

const menuItem = (label: string) =>
	Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined;

describe('play route — on a fractional extra', () => {
	it('names the recap, and steps to the whole episodes either side', async () => {
		useHandlers();
		await mountOnRecap();
		expect(target.querySelector('span.np-ep')?.textContent?.trim()).toBe(
			m.play_now_playing_episode_label({ episode: '7.5' })
		);
		const prev = navButton(m.play_episode_nav_previous_aria_label());
		const next = navButton(m.play_episode_nav_next_aria_label());
		expect(prev?.textContent).toContain(m.play_episode_nav_prev_label({ episode: '7' }));
		expect(next?.textContent).toContain(m.play_episode_nav_next_label({ episode: '8' }));

		const before = FakeEventSource.instances.length;
		next!.click();
		await until(
			() =>
				FakeEventSource.instances
					.slice(before)
					.some((i) => i.url.includes('episode=8') && !i.url.includes('prefetch=1')),
			'the next stream, for episode 8'
		);
		expect(FakeEventSource.instances.slice(before).some((i) => i.url.includes('episode=8.5'))).toBe(
			false
		);
	});

	it('steps back from the recap to episode 7', async () => {
		useHandlers();
		await mountOnRecap();
		const before = FakeEventSource.instances.length;
		navButton(m.play_episode_nav_previous_aria_label())!.click();
		await until(
			() =>
				FakeEventSource.instances
					.slice(before)
					.some((i) => /[?&]episode=7(&|$)/.test(i.url) && !i.url.includes('prefetch=1')),
			'the previous stream, for episode 7'
		);
	});

	it('hands the recap, not episode 7, to the external player', async () => {
		const seen = useHandlers();
		await mountOnRecap();
		await until(() => moreButton() !== null, 'the More actions button');
		moreButton()!.click();
		await until(() => menuItem(m.play_external_label()) !== undefined, 'the external item');
		menuItem(m.play_external_label())!.click();
		await until(() => seen.external.length === 1, 'the external-player request');
		expect(seen.external[0].episode).toBe('7.5');
	});
});

describe('play route — recaps at the edges of a strip page', () => {
	it('shows a recap before episode 1 and one after the page’s last episode on page 1', async () => {
		// Pages of five: 0.5 comes before 1, and 5.5 follows 5, so both
		// belong on page 1 — neither was shown on any page.
		useHandlers(['0.5', '5.5']);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-edge', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => target.querySelector('li[data-ep-num="1"]') !== null, 'the strip');
		await until(
			() => target.querySelector('li[data-ep-num="5.5"]') !== null,
			'the 5.5 tile on page 1'
		);
		expect(target.querySelector('li[data-ep-num="0.5"]')).not.toBeNull();
	});
});
