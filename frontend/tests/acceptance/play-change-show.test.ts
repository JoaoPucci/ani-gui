// Acceptance: the player of a guessed Continue session asks whether
// this is the show the user meant.
//
// The JoJo card's own row is hianime's 2000 OVA listing, which a guess
// from its title matched to the 2012 series; its play landed on the
// 2012 listing and wrote a row there, which the same guess matches.
// The player's slip — a line attached to the now-playing row, never
// over the picture — names the history title and asks. Yes records the
// show without a word. No opens the picker ("Which show is it?"), as
// the ⋯ menu does; it holds playback while it is open. A pick of
// another show covers the card's own row only, and the user switches
// to it now or keeps watching this: the slip alone says which, with no
// toast and no Undo, and the ⋯ entry stays to change it again. Hide
// sets the slip aside for the card's row until the app restarts.

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
import PlayRoute from './PlayRoute.svelte';
import { goto } from '$app/navigation';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { __resetHiddenRowsForTests } from '../../src/lib/history/in-place-link.svelte';
import { recoveryResume } from '../../src/lib/play/resume-after-recovery';

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
	dispatch(name: string, data: string) {
		const ev = { data, type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
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
/** A third show, to pick after a first pick. */
const THIRD = '9999';
const THIRD_TITLE = 'JoJo no Kimyou na Bouken (2007)';
/** The card's own row, and the row its guessed play wrote. */
const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const PLAYED = 'hianime:jojos-bizarre-adventure-2012-708';

let backend: LinkBackend;
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function show(id: string) {
	const en = { en: TITLE };
	if (id === THIRD)
		return { ...kitsuRef(THIRD, THIRD_TITLE, 1), titles: en, slug: 'jojo-2007', subtype: 'movie' };
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
		http.post(`${API_BASE}/api/kitsu/search`, () =>
			HttpResponse.json([show(GUESS), show(RIGHT), show(THIRD)])
		),
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
	button(m.play_link_change_button(), slip()!)!.click();
	await until(() => dialog()?.open === true, 'the picker');
}

/** Pick `title` in the open picker and answer its last step with
 *  `answer`: the one button of a pick of the show playing, or on the
 *  player, "Keep watching this" or "Switch to it now". */
async function pickAndLink(title: string, answer = m.play_link_keep_watching()) {
	const row = () =>
		Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).find((b) =>
			(b.textContent ?? '').includes(title)
		);
	await until(() => row() !== undefined, `${title} among the candidates`);
	row()!.click();
	await until(() => !button(answer, dialog()!)?.disabled, `the ${answer} button`);
	button(answer, dialog()!)!.click();
	await until(() => backend.linkCalls.length > 0, 'the link');
}

const moreMenu = async () => {
	target
		.querySelector<HTMLButtonElement>(`button[aria-label="${m.play_more_aria_label()}"]`)!
		.click();
	await until(() => target.querySelector('#np-more-menu') !== null, 'the menu');
};

describe('the slip of a guessed session', () => {
	it('names the history title and asks whether this is the show meant, under the player', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		expect(button(m.play_link_guess_right(), slip()!)).not.toBeNull();
		// One label, worded as one phrase in every language.
		expect(button(m.play_link_change_button(), slip()!)?.textContent?.trim()).toBe(
			m.play_link_change_button()
		);
		expect(slip()!.querySelector('svg[data-guess-mark]')).not.toBeNull();
		// Attached to the now-playing row, outside the frame fullscreen
		// shows.
		expect(slip()!.closest('.watch-column')).not.toBeNull();
		expect(target.querySelector('.player-frame')!.contains(slip())).toBe(false);
		// Quiet: no dialog on arrival.
		expect(dialog()).toBeNull();
	});

	it("records nothing on Yes when the play landed on another listing than the card's", async () => {
		// The JoJo card's play landed on the 2012 listing: recording the
		// 2012 series would put it on the 2000 line. Yes sets the
		// question aside instead.
		await openPlayer({ guess: '1', row: CARD, landed: 'other' });
		await until(() => slipText().includes(guessNote()), 'the slip');
		expect(button(m.play_link_change_button(), slip()!)).not.toBeNull();
		button(m.play_link_guess_right(), slip()!)!.click();
		await until(() => slip() === null, 'the slip to go');
		await settle();
		expect(backend.linkCalls).toEqual([]);
		expect(toastStore.items).toEqual([]);
	});

	it('holds its room with nothing in it while the guess is decided, and goes if there is none', async () => {
		// The card's row records its show by now: nothing to offer.
		backend.rows = backend.rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: RIGHT } : r));
		let answer: () => void = () => {};
		const asked = new Promise<void>((r) => (answer = r));
		server.use(
			http.get(`${API_BASE}/api/history`, async () => {
				await asked;
				return HttpResponse.json(backend.rows);
			})
		);
		await openPlayer();
		await until(() => slip()?.dataset.state === 'holding', 'the held room');
		// No mark, no text and no buttons in a room that may come to nothing.
		expect(slip()!.querySelector('button')).toBeNull();
		expect(slip()!.querySelector('svg')).toBeNull();
		expect(slipText().trim()).toBe('');
		answer();
		await until(() => slip() === null, 'the room to go');
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
		// The episode the player and breadcrumb show, named as the one
		// playing: Continue Watching picks up at the next one.
		expect(text).toContain(m.detail_link_dialog_title());
		expect(text).toContain(m.play_link_dialog_body({ episode: '2', title: TITLE }));
		expect(text).not.toContain(m.detail_link_dialog_body({ episode: '2', title: TITLE }));
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

describe('a pick of another show on the player, kept watching here', () => {
	it("records the card's own row only, and the slip alone says so", async () => {
		await openPlayer();
		await openPicker();
		// Picking another show asks whether to switch to it now.
		const pick = () =>
			Array.from(dialog()!.querySelectorAll<HTMLButtonElement>('li button')).find((b) =>
				(b.textContent ?? '').includes(RIGHT_TITLE)
			);
		await until(() => pick() !== undefined, 'the candidates');
		pick()!.click();
		await until(() => button(m.play_link_switch_now(), dialog()!) !== null, 'the switch question');
		expect(dialog()!.textContent).toContain(m.play_link_switch_heading({ title: RIGHT_TITLE }));
		expect(dialog()!.textContent).toContain(
			m.play_link_switch_hint({ episode: '2', time: '0:00' })
		);
		expect(button(m.detail_link_confirm_button(), dialog()!)).toBeNull();
		// Switch is the answer the focus is on.
		await until(
			() => document.activeElement === button(m.play_link_switch_now(), dialog()!),
			'focus on Switch'
		);
		button(m.play_link_keep_watching(), dialog()!)!.click();
		await until(() => backend.linkCalls.length > 0, 'the pick to be saved');
		expect(backend.linkCalls[0].card).toBe(CARD);
		// The 2012 listing's row stays the 2012 series' history.
		expect(backend.rows.find((r) => r.id === CARD)?.kitsu_id).toBe(RIGHT);
		expect(backend.rows.find((r) => r.id === PLAYED)?.kitsu_id).toBeUndefined();
		await until(
			() => slipText().includes(m.play_link_linked_note({ title: RIGHT_TITLE })),
			'the note'
		);
		// Said once, in the player: no toast, so no Undo.
		expect(toastStore.items).toEqual([]);
		expect(button(m.play_link_switch_now(), slip()!)).not.toBeNull();
		// The ⋯ entry stays, to change the show again.
		await moreMenu();
		expect(button(m.play_link_menu_item())).not.toBeNull();
	});

	it('plays later episodes of the session as the pick, which history keeps', async () => {
		// A backend that records whatever id a watch names, on the card's
		// row: only the renderer stands between the pick and the old guess.
		const watched: Record<string, unknown>[] = [];
		server.use(
			http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
				const body = (await request.json()) as Record<string, unknown>;
				watched.push(body);
				const id = (body.kitsu_id ?? undefined) as string | undefined;
				if (id)
					backend.rows = backend.rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: id } : r));
				return new HttpResponse(null, { status: 204 });
			})
		);
		await openPlayer();
		await openPicker();
		await pickAndLink(RIGHT_TITLE);
		await until(
			() => slipText().includes(m.play_link_linked_note({ title: RIGHT_TITLE })),
			'the note'
		);
		const next = target.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
		)!;
		await until(() => !next.disabled, 'the next episode button');
		const before = FakeEventSource.instances.length;
		next.click();
		await until(() => FakeEventSource.instances.length > before, 'the next episode play');
		const play = FakeEventSource.instances.at(-1)!;
		const asked = new URL(play.url, 'http://x').searchParams;
		expect(asked.get('episode')).toBe('3');
		// Named as the pick, on the card's row; never as the old guess.
		expect(asked.get('guess_row')).toBe(CARD);
		expect(asked.get('guess_id')).toBe(RIGHT);
		expect(asked.get('kitsu_id')).toBeNull();
		// It lands on the card's own listing.
		play.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-2',
				master_url: '/s/t/master.m3u8',
				media_kind: 'mp4',
				own_listing: true
			})
		);
		await until(() => watched.length > 0, 'the watch');
		expect(watched.every((w) => w.kitsu_id !== GUESS && w.guess_id !== GUESS)).toBe(true);
		expect(backend.rows.find((r) => r.id === CARD)?.kitsu_id).toBe(RIGHT);
	});
});

describe('switching to the pick', () => {
	/** The player's video, at `at` seconds in. */
	function videoAt(at: number) {
		const v = target.querySelector<HTMLVideoElement>('.player-video-slot video')!;
		Object.defineProperty(v, 'currentTime', { configurable: true, get: () => at, set: () => {} });
		v.pause = vi.fn();
		v.play = vi.fn(async () => {});
		return v;
	}

	/** The play the switch asks for: the pick's own. */
	const pickPlay = () =>
		FakeEventSource.instances.find(
			(es) => new URL(es.url, 'http://x').searchParams.get('kitsu_id') === RIGHT
		);

	/** Answer the play the switch asks for, as the backend does. */
	async function answerSwitch() {
		await until(() => pickPlay() !== undefined, 'the play of the pick');
		const play = pickPlay()!;
		const asked = new URL(play.url, 'http://x').searchParams;
		expect(asked.get('episode')).toBe('2');
		expect(asked.get('guess_row')).toBeNull();
		expect(asked.get('guess_id')).toBeNull();
		// The band between episodes holds the player meanwhile.
		expect(target.ownerDocument.querySelector('.band')).not.toBeNull();
		// The route mounts a new page for the other show
		// (play/[id]/+layout.svelte, its own test): the page on screen
		// never sees the new URL. Moving this harness's URL under it
		// would let it take what the switch left for the next page.
		vi.mocked(goto).mockImplementationOnce(async () => {});
		play.dispatch(
			'done',
			JSON.stringify({ session_id: 'session-9', master_url: '/s/u/file.mp4', media_kind: 'mp4' })
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the switch to land');
		return String(vi.mocked(goto).mock.calls.at(-1)?.[0]);
	}

	it('from the picker reloads the player at the same episode and point on the pick', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		videoAt(74);
		await openPicker();
		await pickAndLink(RIGHT_TITLE, m.play_link_switch_now());
		const href = await answerSwitch();
		const url = new URL(href, 'http://x');
		expect(url.pathname).toBe(`/play/${RIGHT}`);
		expect(url.searchParams.get('episode')).toBe('2');
		expect(url.searchParams.get('row')).toBe(CARD);
		// The row records the pick now: the session is no guess.
		expect(url.searchParams.get('guess')).toBeNull();
		expect(recoveryResume.consume(RIGHT, 2)).toBe(74);
		expect(toastStore.items).toEqual([]);
	});

	it('from the slip after keeping watching does the same', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		videoAt(30);
		await openPicker();
		await pickAndLink(RIGHT_TITLE);
		await until(() => button(m.play_link_switch_now(), slip() ?? target) !== null, 'Switch');
		button(m.play_link_switch_now(), slip()!)!.click();
		const href = await answerSwitch();
		expect(new URL(href, 'http://x').pathname).toBe(`/play/${RIGHT}`);
		expect(recoveryResume.consume(RIGHT, 2)).toBe(30);
	});

	it('opens a session that says once what plays now, with the ⋯ entry to change it again', async () => {
		// The route as SvelteKit renders it, its layout around the page,
		// and a navigation that moves the route param as SvelteKit's does:
		// the page that switched never sees the other show's URL.
		setParams({ id: GUESS });
		setUrl(`/play/${GUESS}`, {
			session: 'session-1',
			kind: 'mp4',
			episode: '2',
			guess: '1',
			row: CARD
		});
		const consumed = vi.spyOn(recoveryResume, 'consume');
		app = mount(PlayRoute, { target });
		await until(
			() => (target.querySelector('a.np-link')?.textContent ?? '').includes(GUESS_TITLE),
			'the now-playing row'
		);
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		const before = videoAt(74);
		before.playbackRate = 1.5;
		await openPicker();
		await pickAndLink(RIGHT_TITLE, m.play_link_switch_now());
		await until(() => pickPlay() !== undefined, 'the play of the pick');
		vi.mocked(goto).mockImplementationOnce(async (href) => {
			const next = new URL(String(href), 'http://127.0.0.1:31337');
			setParams({ id: next.pathname.split('/').at(-1)! });
			setUrl(next.pathname, Object.fromEntries(next.searchParams));
		});
		pickPlay()!.dispatch(
			'done',
			JSON.stringify({ session_id: 'session-9', master_url: '/s/u/file.mp4', media_kind: 'mp4' })
		);
		await until(
			() => (target.querySelector('a.np-link')?.textContent ?? '').includes(RIGHT_TITLE),
			"the pick's player"
		);
		await until(
			() => slipText().includes(m.play_link_switched_note({ title: RIGHT_TITLE })),
			'the switched note'
		);
		expect(slip()!.querySelector('button:not(.hide)')).toBeNull();
		// The pick's player, and it alone, resumes from the point carried.
		const handed = consumed.mock.calls
			.map((args, i) => [args, consumed.mock.results[i].value] as const)
			.filter(([, at]) => at !== null);
		expect(handed).toEqual([[[RIGHT, 2], 74]]);
		consumed.mockRestore();
		// The speed goes along once the pick's stream has loaded, as the
		// speed playing, not as a default a later episode would fall
		// back to.
		const after = target.querySelector<HTMLVideoElement>('.player-video-slot video')!;
		after.dispatchEvent(new Event('loadedmetadata'));
		expect(after.playbackRate).toBe(1.5);
		expect(after.defaultPlaybackRate).toBe(1);
		await moreMenu();
		expect(button(m.play_link_menu_item())).not.toBeNull();
	});

	it('never switches to an earlier pick when a later one cannot be saved', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		videoAt(74);
		await openPicker();
		await pickAndLink(RIGHT_TITLE);
		await until(() => button(m.play_link_switch_now(), slip() ?? target) !== null, 'the note');
		server.use(
			http.post(`${API_BASE}/api/history/link`, () => new HttpResponse(null, { status: 500 }))
		);
		await moreMenu();
		button(m.play_link_menu_item())!.click();
		await until(() => dialog()?.open === true, 'the picker');
		const third = () =>
			Array.from(dialog()!.querySelectorAll<HTMLButtonElement>('li button')).find((b) =>
				(b.textContent ?? '').includes(THIRD_TITLE)
			);
		await until(() => third() !== undefined, 'the third show');
		third()!.click();
		await until(() => !button(m.play_link_switch_now(), dialog()!)?.disabled, 'Switch');
		button(m.play_link_switch_now(), dialog()!)!.click();
		await until(() => dialog()?.querySelector('[role="alert"]') !== null, 'the error');
		await settle();
		expect(pickPlay()).toBeUndefined();
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
	});

	it('names no episode of a single-video work on the player, from its first line', async () => {
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
				HttpResponse.json(
					String(params.id) === GUESS
						? { ...show(GUESS), subtype: 'movie', episode_count: 1 }
						: show(String(params.id))
				)
			)
		);
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		button(m.play_link_change_button(), slip()!)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		// Before the candidates land, the line already knows the work.
		expect(dialog()!.textContent).toContain(m.play_link_dialog_body_single({ title: TITLE }));
	});

	it('says so and keeps the player as it was when the pick has no stream', async () => {
		await openPlayer();
		await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
		videoAt(74);
		await openPicker();
		await pickAndLink(RIGHT_TITLE, m.play_link_switch_now());
		await until(() => pickPlay() !== undefined, 'the play of the pick');
		pickPlay()!.dispatch('error', JSON.stringify({ kind: 'timeout' }));
		await until(
			() => toastWith(m.play_link_switch_failed({ title: RIGHT_TITLE })) !== undefined,
			'the failure'
		);
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
		expect(recoveryResume.consume(RIGHT, 2)).toBeNull();
		// Still kept watching here, with the way to switch.
		expect(slipText()).toContain(m.play_link_linked_note({ title: RIGHT_TITLE }));
		expect(button(m.play_link_switch_now(), slip()!)).not.toBeNull();
	});
});

describe('"Yes"', () => {
	it('records the guessed show on the line without a word, and the slip goes', async () => {
		await openPlayer();
		await until(() => slipText().includes(guessNote()), 'the slip');
		button(m.play_link_guess_right(), slip()!)!.click();
		await until(() => backend.linkCalls.length === 1, 'the record');
		expect(backend.linkCalls[0]).toMatchObject({ card: CARD, kitsu_id: GUESS });
		expect(backend.rows.find((r) => r.id === CARD)?.kitsu_id).toBe(GUESS);
		await until(() => slip() === null, 'the slip to go');
		expect(toastStore.items).toEqual([]);
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
