// Acceptance: the player of a guessed Continue session says the show
// was guessed, and lets the viewer keep it or change it.
//
// The JoJo card's own row is hianime's 2000 OVA listing, which a guess
// from its title matched to the 2012 series; its play landed on the
// 2012 listing and wrote a row there, which the same guess matches.
// The player's slip — a line attached to the now-playing row, never
// over the picture — names the history title the guess came from and
// offers "That's right" and "Not this show? Change"; the ⋯ menu offers
// the same picker. The picker holds playback while it is open. A link
// to another show covers the card's own row only, and the slip stays
// to say what is playing is still the guessed show. Hide sets the slip
// aside for the card's row until the app restarts.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { linkBackend, linkBackendHandlers, type LinkBackend } from './link-backend';
import { m } from '../../src/lib/paraglide/messages';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
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
import { goto } from '$app/navigation';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { __resetHiddenRowsForTests } from '../../src/lib/history/in-place-link.svelte';

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
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

const TITLE = "JoJo's Bizarre Adventure";
/** The 2012 series: the guess, and what plays. */
const GUESS = '7158';
const GUESS_TITLE = 'JoJo no Kimyou na Bouken (TV)';
/** The 2000 OVA: what the card really is. */
const RIGHT = '4013';
const RIGHT_TITLE = 'JoJo no Kimyou na Bouken (2000)';
/** The card's own row, and the row its guessed play wrote. */
const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const PLAYED = 'hianime:jojos-bizarre-adventure-2012-708';

let backend: LinkBackend;
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function show(id: string) {
	const en = { en: TITLE };
	return id === RIGHT
		? {
				...kitsuRef(RIGHT, RIGHT_TITLE, 13),
				titles: en,
				slug: 'jojo-no-kimyou-na-bouken-2000',
				subtype: 'OVA',
				start_date: '2000-03-03'
			}
		: {
				...kitsuRef(GUESS, GUESS_TITLE, 26),
				titles: en,
				slug: 'jojo-no-kimyou-na-bouken-tv',
				start_date: '2012-10-06'
			};
}

function serve() {
	server.use(
		...linkBackendHandlers(backend),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig('sub'))),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json(show(String(params.id)))
		),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
			HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
		// Continue Watching remembers the title's guess, for both rows.
		http.get(`${API_BASE}/api/title-match`, ({ request }) =>
			HttpResponse.json(
				(new URL(request.url).searchParams.get('title') ?? '').startsWith(TITLE) ? GUESS : null
			)
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([show(GUESS), show(RIGHT)])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 26,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null))
	);
}

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	__resetHiddenRowsForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	for (const t of toastStore.items) toastStore.dismiss(t.id);
	backend = linkBackend([
		{ ep_no: '2', id: PLAYED, title: TITLE },
		{ ep_no: '1', id: CARD, title: TITLE }
	]);
	serve();
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

const settle = () => new Promise((r) => setTimeout(r, 300));
const slip = () => target.querySelector<HTMLElement>('[data-testid="guess-slip"]');
const slipText = () => slip()?.textContent ?? '';
const guessNote = () => m.play_link_guess_note({ title: TITLE });
const button = (label: string, within: ParentNode = target) =>
	Array.from(within.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').trim().includes(label)
	) ?? null;
const dialog = () => target.querySelector('dialog');
const toastWith = (message: string) => toastStore.items.find((t) => t.message === message);

/** The player of the JoJo card's guessed session, at episode 2. */
async function openPlayer(query: Record<string, string> = { guess: '1', row: CARD }) {
	setParams({ id: GUESS });
	setUrl(`/play/${GUESS}`, { session: 'session-1', kind: 'mp4', episode: '2', ...query });
	app = mount(PlayPage, { target });
	await until(
		() => (target.querySelector('a.np-link')?.textContent ?? '').includes(GUESS_TITLE),
		'the now-playing row'
	);
}

async function openPicker() {
	await until(() => slipText().includes(guessNote()), 'the slip');
	button(m.detail_link_change(), slip()!)!.click();
	await until(() => dialog()?.open === true, 'the picker');
}

async function pickAndLink(title: string) {
	const row = () =>
		Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).find((b) =>
			(b.textContent ?? '').includes(title)
		);
	await until(() => row() !== undefined, `${title} among the candidates`);
	row()!.click();
	await until(() => !button(m.detail_link_confirm_button())?.disabled, 'the Link button');
	button(m.detail_link_confirm_button())!.click();
	await until(() => backend.linkCalls.length > 0, 'the link');
}

describe('the slip of a guessed session', () => {
	it('names the history title the guess was made from, under the player, not over it', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		expect(button(m.play_link_guess_right(), slip()!)).not.toBeNull();
		expect(button(m.detail_link_change(), slip()!)).not.toBeNull();
		expect(slip()!.querySelector('svg[data-guess-mark]')).not.toBeNull();
		// Attached to the now-playing row, outside the frame fullscreen
		// shows.
		expect(slip()!.closest('.watch-column')).not.toBeNull();
		expect(target.querySelector('.player-frame')!.contains(slip())).toBe(false);
		// Quiet: no dialog on arrival.
		expect(dialog()).toBeNull();
	});

	it('offers no "That\'s right" when the play landed on another listing than the card\'s', async () => {
		// The JoJo card's play landed on the 2012 listing: "That's right"
		// would record the 2012 series on the 2000 line.
		await openPlayer({ guess: '1', row: CARD, landed: 'other' });
		await until(() => slipText().includes(guessNote()), 'the slip');
		expect(button(m.play_link_guess_right(), slip()!)).toBeNull();
		expect(button(m.detail_link_change(), slip()!)).not.toBeNull();
	});

	it('is not there for a session no guess opened', async () => {
		await openPlayer({ row: CARD });
		await settle();
		expect(slip()).toBeNull();
	});

	it('shows the same whichever controls the player uses', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () =>
				HttpResponse.json({ ...appConfig('sub'), use_custom_player_controls: true })
			)
		);
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
	});
});

describe('the ⋯ menu of a guessed session', () => {
	it('offers "Not this show…" with the guessed mark, which opens the picker', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		target
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_more_aria_label()}"]`)!
			.click();
		await until(() => button(m.play_link_menu_item()) !== null, 'the menu entry');
		const entry = button(m.play_link_menu_item())!;
		expect(entry.getAttribute('role')).toBe('menuitem');
		expect(entry.querySelector('svg[data-guess-mark]')).not.toBeNull();
		entry.click();
		await until(() => dialog()?.open === true, 'the picker');
		const text = dialog()!.textContent ?? '';
		// The episode the player and breadcrumb show.
		expect(text).toContain(m.detail_link_dialog_body({ episode: '2' }));
		// The guessed show is the one playing, not one "on this page".
		await until(
			() => (dialog()!.textContent ?? '').includes(m.play_link_playing_now()),
			'the current show labelled'
		);
		expect(dialog()!.textContent).not.toContain(m.detail_link_shown_now());
	});

	it('has no such entry for a session no guess opened', async () => {
		await openPlayer({ row: CARD });
		await settle();
		target
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_more_aria_label()}"]`)!
			.click();
		await until(() => target.querySelector('#np-more-menu') !== null, 'the menu');
		expect(button(m.play_link_menu_item())).toBeNull();
	});
});

describe('the picker on the player', () => {
	function video(playing: boolean) {
		const v = target.querySelector<HTMLVideoElement>('.player-video-slot video')!;
		let paused = !playing;
		Object.defineProperty(v, 'paused', { configurable: true, get: () => paused });
		const pause = vi.fn(() => {
			paused = true;
		});
		const play = vi.fn(async () => {
			paused = false;
		});
		v.pause = pause;
		v.play = play;
		return { pause, play };
	}

	it('pauses a playing video while open, and plays it again when closed', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		const v = video(true);
		await openPicker();
		expect(v.pause).toHaveBeenCalledTimes(1);
		button(m.detail_link_cancel_button())!.click();
		await until(() => dialog() === null, 'the picker to close');
		await until(() => v.play.mock.calls.length === 1, 'playback again');
	});

	it('leaves a paused video paused when it closes', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		const v = video(false);
		await openPicker();
		button(m.detail_link_cancel_button())!.click();
		await until(() => dialog() === null, 'the picker to close');
		await settle();
		expect(v.pause).not.toHaveBeenCalled();
		expect(v.play).not.toHaveBeenCalled();
	});
});

describe('the player behind the picker', () => {
	it('takes none of its shortcuts while the picker is open', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		const v = target.querySelector<HTMLVideoElement>('.player-video-slot video')!;
		await openPicker();
		const before = v.muted;
		const inside = dialog()!.querySelector('button')!;
		inside.focus();
		inside.dispatchEvent(new KeyboardEvent('keydown', { key: 'm', bubbles: true }));
		await settle();
		expect(v.muted).toBe(before);
		// …and takes them again once it closes.
		button(m.detail_link_cancel_button())!.click();
		await until(() => dialog() === null, 'the picker to close');
		document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'm', bubbles: true }));
		await until(() => v.muted !== before, 'the mute shortcut');
	});
});

describe('linking from the player', () => {
	it("links the card's own row only, and the slip says what is still playing", async () => {
		await openPlayer();
		await openPicker();
		await pickAndLink(RIGHT_TITLE);
		expect(backend.linkCalls[0].card).toBe(CARD);
		// The 2012 listing's row stays the 2012 series' history.
		expect(backend.rows.find((r) => r.id === CARD)?.kitsu_id).toBe(RIGHT);
		expect(backend.rows.find((r) => r.id === PLAYED)?.kitsu_id).toBeUndefined();
		await until(
			() => slipText().includes(m.play_link_linked_note({ title: RIGHT_TITLE })),
			'the linked note'
		);
		expect(toastWith(m.detail_link_done_toast({ title: RIGHT_TITLE }))?.actionLabel).toBe(
			m.detail_link_undo()
		);
		// The ⋯ entry goes with the guess.
		target
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_more_aria_label()}"]`)!
			.click();
		await until(() => target.querySelector('#np-more-menu') !== null, 'the menu');
		expect(button(m.play_link_menu_item())).toBeNull();
		// The way to the linked show.
		button(m.play_link_open_linked(), slip()!)!.click();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the way to the linked show');
		expect(String(vi.mocked(goto).mock.calls.at(-1)?.[0])).toBe(`/anime/${RIGHT}`);
	});

	it('gives the slip its guess back on Undo', async () => {
		await openPlayer();
		await openPicker();
		await pickAndLink(RIGHT_TITLE);
		await until(
			() => toastWith(m.detail_link_done_toast({ title: RIGHT_TITLE })) !== undefined,
			'the toast'
		);
		toastWith(m.detail_link_done_toast({ title: RIGHT_TITLE }))!.onAction!();
		await until(() => backend.unlinkCalls.length === 1, 'the undo');
		await until(() => slipText().includes(guessNote()), 'the guess again');
		expect(backend.rows.every((r) => r.kitsu_id === undefined)).toBe(true);
	});
});

describe('"That\'s right"', () => {
	it('records the guessed show on the line, says it was kept, and Undo brings the guess back', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		button(m.play_link_guess_right(), slip()!)!.click();
		await until(() => backend.linkCalls.length === 1, 'the link');
		expect(backend.linkCalls[0]).toMatchObject({ card: CARD, kitsu_id: GUESS });
		expect(backend.rows.find((r) => r.id === CARD)?.kitsu_id).toBe(GUESS);
		await until(() => slip() === null, 'the slip to go');
		const toast = toastWith(m.play_link_kept_toast({ title: GUESS_TITLE }));
		expect(toast?.actionLabel).toBe(m.detail_link_undo());
		toast!.onAction!();
		await until(() => backend.unlinkCalls.length === 1, 'the undo');
		await until(() => slipText().includes(guessNote()), 'the guess again');
	});
});

describe('Hide', () => {
	it('sets the slip aside for the card until the app restarts', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		target
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_link_hide_aria()}"]`)!
			.click();
		await until(() => slip() === null, 'the slip to go');
		// The ⋯ entry still reaches the picker while the session is a guess.
		target
			.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_more_aria_label()}"]`)!
			.click();
		await until(() => button(m.play_link_menu_item()) !== null, 'the menu entry');

		// The card played again: still hidden.
		unmount(app!);
		app = null;
		await openPlayer();
		await settle();
		expect(slip()).toBeNull();

		// A restart forgets it.
		unmount(app!);
		app = null;
		__resetHiddenRowsForTests();
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip after a restart');
	});
});
