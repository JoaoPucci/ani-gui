// Acceptance: the player's links up to the show page carry the
// guessed Continue card's row a guessed session was opened from
// (`?guessed=<row>`), so that page can say which history title it was
// matched from. A session opened any other way carries nothing.

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
// A landed switch navigates to the episode it played; following it
// keeps the page's own episode where the real router would put it, so
// a click on the episode already playing does nothing here either.
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
import { breadcrumb } from '../../src/lib/breadcrumb';
import { get } from 'svelte/store';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { Provider } from '../../src/lib/account/types';

const KITSU_ID = '42';
const TITLE = 'Finished Show';
const TRACKERS: Provider[] = ['mal', 'anilist'];

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	landed = false;
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

interface TrackerWrite {
	provider: string;
	body: Record<string, unknown>;
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let writes: TrackerWrite[];
let marked: string[];
let stamped: { episode?: string; kitsu_id?: string }[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	writes = [];
	marked = [];
	stamped = [];
	setParams({ id: KITSU_ID });
	// The write-back fans out to every connected tracker. `hydrate()`
	// reads the Electron keychain from the layout, which a page-level
	// mount never reaches — so seed the store the way hydrate would.
	for (const provider of TRACKERS) {
		accountStore.setConnected(provider, {
			access_token: `${provider}-bearer`,
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-1',
			username: 'tester',
			avatar_url: null
		});
	}
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	for (const provider of TRACKERS) accountStore.setDisconnected(provider);
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

interface Show {
	/** Kitsu's episode count for the show. */
	kitsuCount: number;
	/** Kitsu's status: `finished` is the only one that completes. */
	status: string;
	/** How many episodes the provider lists for the audio in use. */
	providerCount: number;
	mode?: 'sub' | 'dub';
}

function serve(show: Show) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(show.mode ?? 'sub'))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, show.kitsuCount), status: show.status })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({
				aired: show.kitsuCount,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(show.kitsuCount))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: show.providerCount,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
			const body = (await request.json()) as { episode?: string; kitsu_id?: string };
			marked.push(String(body.episode));
			stamped.push(body);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/account/update/:provider`, async ({ request, params }) => {
			writes.push({
				provider: String(params.provider),
				body: (await request.json()) as Record<string, unknown>
			});
			return HttpResponse.json(null);
		})
	);
}

const ROW = 'hianime:finished-show-7';

/** The now-playing link up to the show page, once it renders. */
async function showLink(query: Record<string, string>): Promise<string> {
	setUrl(`/play/${KITSU_ID}`, { episode: '3', ...query });
	serve({ kitsuCount: 12, status: 'finished', providerCount: 12 });
	app = mount(PlayPage, { target });
	// Once the show's detail lands, so what it sets off happens on the page.
	await until(
		() => (target.querySelector('a.np-link')?.textContent ?? '').includes(TITLE),
		'the now-playing link'
	);
	await new Promise((r) => setTimeout(r, 200));
	const link = target.querySelector('a.np-link')!.getAttribute('href') ?? '';
	// The breadcrumb's show segment, and the up-chevron that reads it,
	// go to the same place.
	expect(get(breadcrumb)[1]?.href).toBe(link);
	return link;
}

describe('play route — the link up to the show page', () => {
	it("carries a guessed card's row", async () => {
		expect(await showLink({ guess: '1', row: ROW })).toBe(
			`/anime/${KITSU_ID}?guessed=${encodeURIComponent(ROW)}`
		);
	});

	it('carries nothing for a session no guess opened', async () => {
		expect(await showLink({ row: ROW })).toBe(`/anime/${KITSU_ID}`);
		unmount(app!);
		app = null;
		expect(await showLink({})).toBe(`/anime/${KITSU_ID}`);
	});
});
