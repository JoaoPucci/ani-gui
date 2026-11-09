// Acceptance: Sub or Dub is remembered per show.
//
// Picking the audio on a show's page remembers it for that show, in
// local storage like the subtitle language, and writes nothing to the
// settings: the Settings value is only the default for shows never
// chosen on. Every request the show's pages make — availability, plays
// and warms, downloads, the hand-offs — asks for the show's audio, and
// the label says when the show has a choice of its own.

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
import { rememberAudio } from '../../src/lib/play/audio-choice';
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
/** The audio each availability question asked about, in order. */
const probedModes: string[] = [];
/** How many times the page wrote the settings. */
let settingsWrites = 0;
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
	probedModes.length = 0;
	settingsWrites = 0;
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
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			probes += 1;
			probedModes.push(((await request.json()) as { mode: string }).mode);
			return HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
				episode_count_approximate: false
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.put(`${API_BASE}/api/settings`, () => {
			settingsWrites += 1;
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
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

const audioButton = (label: 'SUB' | 'DUB') =>
	Array.from(target.querySelectorAll<HTMLButtonElement>('button.seg-btn')).find(
		(b) => b.textContent?.trim() === label
	) ?? null;

const audioLabel = () =>
	target.querySelector(`[aria-label="${m.detail_audio_group_aria_label()}"] .seg-label`)
		?.textContent ?? '';

/** The streams the page's plays opened. */
const plays = () =>
	FakeEventSource.instances
		.map((es) => new URL(es.url))
		.filter((u) => u.pathname === '/api/play/stream');

async function mountDetail(settings: Record<string, unknown> = {}, id = KITSU_ID) {
	setParams({ id });
	const h = useShowHandlers(settings);
	app = mount(DetailPage, { target });
	await until(() => h.probes() > 0, 'the page-load availability lookup');
	await until(
		() => audioButton('DUB') !== null && !audioButton('DUB')!.disabled,
		'the audio control'
	);
	return h;
}

describe('detail route — Sub or Dub per show', () => {
	it('a show never chosen on follows the Settings default, and says nothing of its own', async () => {
		await mountDetail({ mode: 'dub' });
		expect(audioButton('DUB')?.getAttribute('aria-pressed')).toBe('true');
		expect(audioLabel()).not.toContain(m.detail_audio_note_show());
		expect(probedModes.every((mode) => mode === 'dub')).toBe(true);
	});

	it('picking Dub remembers it for the show, writes no setting, and marks the label', async () => {
		await mountDetail({ mode: 'sub' });
		audioButton('DUB')!.click();
		flushSync();
		await until(() => probedModes.includes('dub'), 'the dub question');
		expect(audioButton('DUB')?.getAttribute('aria-pressed')).toBe('true');
		expect(audioLabel()).toContain(m.detail_audio_note_show());
		expect(settingsWrites).toBe(0);

		// The show keeps it on the next visit.
		unmount(app!);
		app = null;
		await mountDetail({ mode: 'sub' });
		expect(audioButton('DUB')?.getAttribute('aria-pressed')).toBe('true');
		expect(audioLabel()).toContain(m.detail_audio_note_show());
	});

	it('another show is not touched by the choice', async () => {
		rememberAudio(KITSU_ID, 'dub');
		const OTHER = '9999';
		setParams({ id: OTHER });
		const h = useShowHandlers({ mode: 'sub' });
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/${OTHER}`, () =>
				HttpResponse.json({ ...kitsuRef(OTHER, 'Other Show', EPISODES), status: 'finished' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${OTHER}`, () =>
				HttpResponse.json({
					aired: EPISODES,
					next_episode: null,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/history/by-kitsu/${OTHER}`, () => HttpResponse.json(null))
		);
		app = mount(DetailPage, { target });
		await until(() => h.probes() > 0, 'the page-load availability lookup');
		await until(() => audioButton('SUB') !== null, 'the audio control');
		expect(audioButton('SUB')?.getAttribute('aria-pressed')).toBe('true');
		expect(audioLabel()).not.toContain(m.detail_audio_note_show());
		expect(probedModes.every((mode) => mode === 'sub')).toBe(true);
	});

	it("the show's audio reaches its warms and its downloads", async () => {
		rememberAudio(KITSU_ID, 'dub');
		await mountDetail({ mode: 'sub' });
		await until(() => plays().length > 0, "the page's warm");
		for (const play of plays()) expect(play.searchParams.get('mode')).toBe('dub');
		await until(() => {
			const b = detailDownloadButton();
			return b !== null && !b.disabled;
		}, 'the Download action to enable');
		detailDownloadButton()!.click();
		await until(() => dialog() !== null, 'the download dialog');
		dialogButton(m.download_mode_all())!.click();
		flushSync();
		const dl = await confirmDownload();
		expect(dl.searchParams.get('mode')).toBe('dub');
	});
});

describe('detail route — lists of other shows keep the default', () => {
	it("the similar titles are filtered by the Settings default, not this show's audio", async () => {
		rememberAudio(KITSU_ID, 'dub');
		const batchModes: string[] = [];
		setParams({ id: KITSU_ID });
		const h = useShowHandlers({ mode: 'sub' });
		server.use(
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([kitsuRef('777', 'Similar Show', 12)])
			),
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				batchModes.push(((await request.json()) as { mode: string }).mode);
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			}),
			http.post(`${API_BASE}/api/availability/warm`, () => new HttpResponse(null, { status: 202 }))
		);
		app = mount(DetailPage, { target });
		await until(() => h.probes() > 0, 'the page-load availability lookup');
		await until(() => batchModes.length > 0, 'the similar titles filter');
		expect(batchModes).toEqual(['sub']);
	});
});

describe('detail route — the audio control without the settings', () => {
	it('picks and remembers the audio even when the settings cannot be read', async () => {
		setParams({ id: KITSU_ID });
		const h = useShowHandlers({ mode: 'sub' });
		server.use(http.get(`${API_BASE}/api/settings`, () => new HttpResponse(null, { status: 500 })));
		app = mount(DetailPage, { target });
		await until(() => h.probes() > 0, 'the page-load availability lookup');
		await until(() => audioButton('DUB') !== null, 'the audio control');
		// With neither a default nor a choice, neither audio reads as picked.
		expect(audioButton('SUB')?.getAttribute('aria-pressed')).toBe('false');
		expect(audioButton('DUB')?.disabled).toBe(false);
		audioButton('DUB')!.click();
		flushSync();
		expect(audioButton('DUB')?.getAttribute('aria-pressed')).toBe('true');
		expect(audioLabel()).toContain(m.detail_audio_note_show());
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

describe("play route — the show's own audio", () => {
	it('its warms, its availability and its downloads ask for it', async () => {
		rememberAudio(KITSU_ID, 'dub');
		setParams({ id: KITSU_ID });
		useShowHandlers({ mode: 'sub' });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		await until(() => probedModes.length > 0, 'the availability question');
		expect(probedModes.every((mode) => mode === 'dub')).toBe(true);
		await until(() => plays().length > 0, 'the next episode warm');
		for (const play of plays()) expect(play.searchParams.get('mode')).toBe('dub');
		await until(() => moreButton() !== null, 'the More actions button under the player');
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
		dialogButton(m.download_mode_this_episode())!.click();
		flushSync();
		const dl = await confirmDownload();
		expect(dl.searchParams.get('mode')).toBe('dub');
	});
});
