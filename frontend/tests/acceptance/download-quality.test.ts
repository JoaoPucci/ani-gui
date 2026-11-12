// Acceptance: downloads take the download quality from Settings, and
// nothing else asks for a quality.
//
// The show page has no Quality control any more: the built-in player
// starts on Auto and the backend resolves the master for every play.
// The download window says which quality the download takes and where
// to change it, and the stream it opens asks for that quality — not a
// level picked in the player, and not the quality an older version
// wrote to the settings.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, flushSync } from 'svelte';

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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { qualityPin } from '../../src/lib/play/quality-pin';
import { downloadStore } from '../../src/lib/download/store.svelte';

const KITSU_ID = '7154';
const TITLE = "JoJo's Bizarre Adventure";
const EPISODES = 26;

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
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	// Each case's download stays open — nothing ends its stream — and
	// holds a slot in the download queue, which outlives the case. Left
	// running, a later case's download would wait as queued behind the
	// earlier cases' instead of opening its stream.
	for (const item of [...downloadStore.items]) downloadStore.cancel(item.id);
	await new Promise((r) => setTimeout(r, 0));
	if (app) unmount(app);
	app = null;
	qualityPin.clear();
	delete g.EventSource;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${document.body.textContent}`);
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

/** Every request either page makes for the show, with `settings` over
 *  the defaults. */
function useShowHandlers(settings: Record<string, unknown> = {}) {
	let probes = 0;
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json({ ...appConfig(), ...settings })),
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
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/availability`, () => {
			probes += 1;
			return HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
				episode_count_approximate: false
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	return { probes: () => probes };
}

/** The download streams confirming opened — not the page's resolves. */
const downloads = () =>
	FakeEventSource.instances
		.map((es) => new URL(es.url))
		.filter((u) => u.pathname === '/api/download/stream');

const dialog = () =>
	document.querySelector(
		'[role="dialog"][aria-labelledby="dl-confirm-title"]'
	) as HTMLElement | null;

/** A button in the confirm dialog, found by what it says. */
const dialogButton = (label: string) =>
	(Array.from(dialog()?.querySelectorAll('button') ?? []).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined) ?? null;

/** Confirm the open dialog and return the download it opened. */
async function confirmDownload() {
	const before = downloads().length;
	await until(() => {
		const confirm = dialogButton(m.download_button_confirm());
		return confirm !== null && !confirm.disabled;
	}, 'the confirm button to enable');
	dialogButton(m.download_button_confirm())!.click();
	await until(() => downloads().length > before, 'the download stream');
	return downloads()[downloads().length - 1];
}

// ---------------------------------------------------------------- detail

const detailDownloadButton = () =>
	(Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(m.detail_download_button())
	) as HTMLButtonElement | undefined) ?? null;

/** Mount the detail page and open the download dialog once the
 *  availability answer has set the range cap. */
async function openDetailDownload(settings: Record<string, unknown> = {}) {
	const h = useShowHandlers(settings);
	app = mount(DetailPage, { target });
	await until(() => h.probes() > 0, 'the page-load availability lookup');
	await until(() => {
		const b = detailDownloadButton();
		return b !== null && !b.disabled;
	}, 'the Download action to enable');
	detailDownloadButton()!.click();
	await until(() => dialog() !== null, 'the download dialog');
}

/** The streams the page's plays opened. */
const plays = () =>
	FakeEventSource.instances
		.map((es) => new URL(es.url))
		.filter((u) => u.pathname === '/api/play/stream');

/** Every group of buttons on the page that offers a quality. */
const qualityGroups = () =>
	Array.from(target.querySelectorAll('[role="group"]')).filter((g) =>
		Array.from(g.querySelectorAll('button')).some((b) => (b.textContent ?? '').trim() === '1080')
	);

describe('detail route — downloads take the download quality', () => {
	it('has no Quality control, and its plays ask for no quality', async () => {
		await openDetailDownload({ download_quality: '720' });
		expect(qualityGroups()).toEqual([]);
		await until(() => plays().length > 0, "the page's warm");
		for (const play of plays()) expect(play.searchParams.has('quality')).toBe(false);
	});

	it('the download window names the download quality, and the download asks for it', async () => {
		await openDetailDownload({ download_quality: '720' });
		expect(dialog()?.textContent).toContain(m.download_hint_quality({ quality: '720' }));
		dialogButton(m.download_mode_all())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('quality')).toBe('720');
	});

	it("an older version's quality setting is not the download quality: Best is", async () => {
		await openDetailDownload({ quality: '480', download_quality: undefined });
		expect(dialog()?.textContent).toContain(
			m.download_hint_quality({ quality: m.app_quality_best() })
		);
		dialogButton(m.download_mode_all())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('quality')).toBe('best');
	});
});

// ------------------------------------------------------------------ play

const moreButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_more_aria_label()}"]`
	) as HTMLButtonElement | null;

const menuItem = (label: string) =>
	Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined;

/** Mount the play page on a playing session for episode 3 and open
 *  the download dialog from "More actions". */
async function openPlayDownload(settings: Record<string, unknown> = {}) {
	useShowHandlers(settings);
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => moreButton() !== null, 'the More actions button under the player');
	// The entry refuses while the airing answer is still out; open the
	// menu until a click opens the dialog.
	await until(() => {
		if (dialog() !== null) return true;
		moreButton()!.click();
		flushSync();
		const item = menuItem(m.play_controls_menu_download());
		if (item && !item.disabled) {
			item.click();
			flushSync();
		}
		return dialog() !== null;
	}, 'the download dialog');
}

describe('play route — downloads take the download quality', () => {
	it('a level picked in the player is not the download quality', async () => {
		qualityPin.set(480);
		await openPlayDownload({ download_quality: '1080' });
		expect(dialog()?.textContent).toContain(m.download_hint_quality({ quality: '1080' }));
		dialogButton(m.download_mode_this_episode())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('quality')).toBe('1080');
	});
});
