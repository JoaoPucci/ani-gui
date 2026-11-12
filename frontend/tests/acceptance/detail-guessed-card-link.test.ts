// Acceptance: the show page a guessed Continue card opened says, in
// one quiet line, which history title it was matched from, and lets
// the user name the show the card really is.
//
// A Continue card whose history row recorded no Kitsu id is shown as
// the show a guess matched it to — here, as Continue Watching does, by
// a remembered title match. The player's links up to the show page
// carry the card's row (`?guessed=<row>`), and that page alone carries
// the line. Linking writes, through the backend, the chosen show's id
// on every guessed row the card stands for (what the write leaves
// alone is the backend's: `backend/src/commands/history_link_test.rs`);
// the fake backend below only records the id on the rows named.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { served } from './link-backend';
import { groupMembers } from '../fakes/link-rules';
import type { LinkGroupRequest } from '../../src/lib/api';
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
		const id = next.pathname.match(/^\/anime\/([^/]+)$/)?.[1];
		if (id) setParams({ id });
	}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import HomePage from '../../src/routes/+page.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { goto } from '$app/navigation';
import { __resetApiBaseForTests, markWatched } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { appLinkDeps } from '../../src/lib/history/link-deps';
import { rememberAudio, storedAudio } from '../../src/lib/play/audio-choice';

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

const TITLE = 'Black Clover';
/** The show the guess matched the card to. */
const GUESS = '12625';
const GUESS_TITLE = 'Black Clover Special';
/** The show the card really is. */
const RIGHT = '13209';
/** The card's row, and a row of the same title it hides: both have
 *  no watch moment, so the card is the one further along. */
const CARD = 'hianime:black-clover-949';
const HIDDEN = 'black-clover-77';

interface Row {
	ep_no: string;
	id: string;
	title: string;
	kitsu_id?: string;
	named?: boolean;
}

let rows: Row[];
let linkCalls: { card: string; rows: string[]; kitsu_id: string }[];
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	for (const t of toastStore.items) toastStore.dismiss(t.id);
	rows = [
		{ ep_no: '5', id: CARD, title: TITLE },
		{ ep_no: '3', id: HIDDEN, title: TITLE }
	];
	linkCalls = [];
	target = document.createElement('div');
	document.body.appendChild(target);
	serveBackend();
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

function show(id: string) {
	return id === RIGHT
		? { ...kitsuRef(RIGHT, TITLE, 170), start_date: '2017-10-03' }
		: { ...kitsuRef(GUESS, GUESS_TITLE, 26), start_date: '2016-11-27' };
}

function serveBackend() {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json(served(rows))),
			http.get(`${API_BASE}/api/history/:id/number`, ({ params }) => {
				const row = rows.find((r) => r.id === params.id);
				return HttpResponse.json(row ? { ep_no: row.ep_no } : null);
			}),
			http.get(`${API_BASE}/api/history/by-kitsu/:id`, ({ params }) =>
				HttpResponse.json(rows.find((r) => r.kitsu_id === params.id) ?? null)
			),
			// Continue Watching remembers the title's guess.
			http.get(`${API_BASE}/api/title-match`, ({ request }) =>
				HttpResponse.json(new URL(request.url).searchParams.get('title') === TITLE ? GUESS : null)
			),
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const { query } = (await request.json()) as { query: string };
				if (query === TITLE) return HttpResponse.json([show(GUESS), show(RIGHT)]);
				if (query === 'black clover') return HttpResponse.json([show(RIGHT)]);
				return HttpResponse.json([]);
			}),
			http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
				HttpResponse.json(show(String(params.id)))
			),
			http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
				HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 26,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.post(`${API_BASE}/api/history/link`, async ({ request }) => {
				const body = (await request.json()) as { card: string; rows: string[]; kitsu_id: string };
				linkCalls.push(body);
				const linked = rows
					.filter((r) => body.rows.includes(r.id))
					.map((r) => ({
						row: r.id,
						previous: r.kitsu_id ?? null,
						ep_no: r.ep_no,
						watched_at: null
					}));
				rows = rows.map((r) =>
					body.rows.includes(r.id) ? { ...r, kitsu_id: body.kitsu_id, named: true } : r
				);
				return HttpResponse.json({ kitsu_id: body.kitsu_id, linked });
			}),
			// Both rows are Black Clover's one listing: the card's own and
			// a retired provider's bare row with the same words.
			http.post(`${API_BASE}/api/history/link-group`, async ({ request }) => {
				const body = (await request.json()) as LinkGroupRequest;
				const members = groupMembers(body.card, body.show, body.saved, body.rows);
				return HttpResponse.json(members.filter((r) => rows.some((h) => h.id === r)));
			}),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null))
		])
	);
}

const note = () => m.detail_link_matched({ title: TITLE });
const hasNote = () => (target.textContent ?? '').includes(note());
const button = (label: string) =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').trim().includes(label)
	) ?? null;

async function openShowPage(id: string, query: Record<string, string> = {}) {
	setUrl(`/anime/${id}`, query);
	setParams({ id });
	app = mount(DetailPage, { target });
	await until(() => (target.querySelector('h1')?.textContent ?? '').length > 0, 'the title');
}

describe('the show page a guessed Continue card opened', () => {
	it('says which history title it was matched from', async () => {
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		expect(button(m.detail_link_change())).not.toBeNull();
		// The app's one mark for a guess.
		expect(target.querySelector('.guess-note [data-guess-mark]')).not.toBeNull();
		// Quiet: no dialog on arrival.
		expect(target.querySelector('dialog')).toBeNull();
	});

	it('says nothing when opened any other way', async () => {
		await openShowPage(GUESS);
		await settle();
		expect(hasNote()).toBe(false);
		expect(button(m.detail_link_change())).toBeNull();
	});

	it('says nothing once the row records another show', async () => {
		rows = rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: RIGHT } : r));
		await openShowPage(GUESS, { guessed: CARD });
		await settle();
		expect(hasNote()).toBe(false);
		// The room held for the line goes once nothing is offered.
		expect(target.querySelector('.guess-note')).toBeNull();
	});

	it("keeps the line when the row records the page's show by the guessed session's own play", async () => {
		// The player's guessed play landed on the card's own listing and
		// recorded the guess: the session is still the guess its card
		// opened, and the page it goes up to still asks.
		await markWatched({
			title: TITLE,
			episode: '5',
			mode: 'sub',
			guess_row: CARD,
			guess_id: GUESS
		});
		rows = rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: GUESS } : r));
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		expect(button(m.detail_link_change())).not.toBeNull();
	});

	it('says nothing when revisited once the show was played from its own page, which recorded it', async () => {
		await markWatched({
			title: TITLE,
			episode: '5',
			mode: 'sub',
			guess_row: CARD,
			guess_id: GUESS
		});
		// The show played from its page records it on the card's row.
		await markWatched({ title: TITLE, episode: '6', mode: 'sub', kitsu_id: GUESS });
		rows = rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: GUESS } : r));
		// Back to the page the guessed player went up to.
		await openShowPage(GUESS, { guessed: CARD });
		await settle();
		expect(hasNote()).toBe(false);
	});

	it('plays as it always does with the line there', async () => {
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		await until(() => {
			const b = button(m.detail_play_button_episode_one());
			return b !== null && !b.disabled;
		}, 'the Play button');
		button(m.detail_play_button_episode_one())!.click();
		await until(
			() =>
				FakeEventSource.instances.some(
					(i) => new URL(i.url).searchParams.get('prefetch') !== 'true'
				),
			'the play to start'
		);
		expect(target.querySelector('dialog')).toBeNull();
	});
});

/** On the guessed page, pick the right show and link it. */
async function linkToTheRightShow() {
	await openShowPage(GUESS, { guessed: CARD });
	await until(hasNote, 'the matched-from line');
	button(m.detail_link_change())!.click();
	await until(() => target.querySelector('dialog') !== null, 'the picker');
	const row = () =>
		Array.from(target.querySelectorAll('dialog li button')).find((b) =>
			/^Black Clover\s+2017\b/.test((b.textContent ?? '').trim())
		) as HTMLButtonElement | undefined;
	await until(() => row() !== undefined, 'the right show among the candidates');
	row()!.click();
	await until(
		() => (target.textContent ?? '').includes(m.detail_link_confirm_heading({ title: TITLE })),
		'the confirmation'
	);
	// The card hides another row of the same guess, which moves with it.
	await until(
		() => (target.textContent ?? '').includes(m.detail_link_confirm_rows()),
		'the note that the hidden row moves too'
	);
	await until(() => !button(m.detail_link_confirm_button())!.disabled, 'the Link button');
	button(m.detail_link_confirm_button())!.click();
	await until(() => linkCalls.length === 1, 'the link');
}

/** Home's Continue Watching cards, by their show line. */
async function homeCards(): Promise<string[]> {
	if (app) unmount(app);
	setUrl('/');
	app = mount(HomePage, { target });
	await until(
		() => target.querySelectorAll('.resume-card:not(.resume-card-loading)').length > 0,
		'the Continue cards'
	);
	await settle();
	return Array.from(target.querySelectorAll('.resume-card .resume-show')).map(
		(e) => e.textContent ?? ''
	);
}

describe('the show page of a line the user named a show for', () => {
	it('shows the Change line, opened with the line as its player links up', async () => {
		// The player's link up carries the line (play-change-show).
		rows = rows.map((r) => ({ ...r, kitsu_id: RIGHT, named: true }));
		await openShowPage(RIGHT, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		expect(button(m.detail_link_change())).not.toBeNull();
	});
});

describe('changing the show', () => {
	it('links every guessed row of the card, goes to the show, and Home follows', async () => {
		expect(await homeCards()).toEqual([GUESS_TITLE]);
		unmount(app!);
		app = null;

		await linkToTheRightShow();

		expect(linkCalls[0]).toEqual({ card: CARD, rows: [CARD, HIDDEN], kitsu_id: RIGHT });
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the show page');
		expect(String(vi.mocked(goto).mock.calls.at(-1)?.[0])).toBe(`/anime/${RIGHT}`);
		await until(() => !hasNote(), 'the line to go');
		const done = toastStore.items.find(
			(t) => t.message === m.detail_link_done_toast({ title: TITLE })
		);
		expect(done).toBeDefined();

		// A fresh load of Home: one card, as the linked show, and no
		// hidden row surfacing as the old guess.
		expect(await homeCards()).toEqual([TITLE]);
	});

	it('says the show was saved, with no Undo on the notice', async () => {
		await linkToTheRightShow();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the show page');
		const done = toastStore.items.find(
			(t) => t.message === m.detail_link_done_toast({ title: TITLE })
		);
		expect(done).toBeDefined();
		expect(done!.actionLabel).toBeNull();
		expect(done!.onAction).toBeNull();
	});

	it('leaves the user where they went while the link was out, and keeps the link', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		server.use(
			http.post(`${API_BASE}/api/history/link`, async ({ request }) => {
				const body = (await request.json()) as { card: string; rows: string[]; kitsu_id: string };
				linkCalls.push(body);
				await held;
				return HttpResponse.json({ kitsu_id: body.kitsu_id, linked: [] });
			})
		);
		await linkToTheRightShow();
		// Back, while the link is out.
		setUrl('/');
		release();
		await until(
			() => toastStore.items.some((t) => t.message === m.detail_link_done_toast({ title: TITLE })),
			'the saved notice'
		);
		await settle();
		expect(vi.mocked(goto)).not.toHaveBeenCalled();
	});
});

describe('the picker', () => {
	it('opens in the top layer, as a modal dialog, so it shows over a fullscreen player', async () => {
		const showModal = vi.spyOn(HTMLDialogElement.prototype, 'showModal');
		try {
			await openShowPage(GUESS, { guessed: CARD });
			await until(hasNote, 'the matched-from line');
			button(m.detail_link_change())!.click();
			await until(() => target.querySelector('dialog') !== null, 'the picker');
			await until(() => showModal.mock.calls.length === 1, 'showModal');
			expect(target.querySelector('dialog')!.open).toBe(true);
			// The show the card shows now is labelled as this page.
			await until(
				() =>
					(target.querySelector('dialog')!.textContent ?? '').includes(m.detail_link_shown_now()),
				'the current show'
			);
		} finally {
			showModal.mockRestore();
		}
	});
});

describe('the episode the picker names', () => {
	it('is the one Continue Watching picks up at, as the card and the page show it', async () => {
		// The row saved episode 5; the card, and the page's Continue,
		// offer episode 6.
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		button(m.detail_link_change())!.click();
		await until(() => target.querySelector('dialog') !== null, 'the picker');
		const text = target.querySelector('dialog')!.textContent ?? '';
		expect(text).toContain(m.detail_link_dialog_body({ episode: '6', title: TITLE }));
		expect(text).not.toContain(m.detail_link_dialog_body({ episode: '5', title: TITLE }));
	});
});

describe('the picker, by keyboard and while it links', () => {
	it('gives focus back to Change when closed', async () => {
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		const change = button(m.detail_link_change())!;
		change.focus();
		change.click();
		await until(() => target.querySelector('dialog') !== null, 'the picker');
		// Escape where focus is, as a key press is.
		(document.activeElement ?? document.body).dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
		await until(() => target.querySelector('dialog') === null, 'the picker to close');
		await until(() => document.activeElement === button(m.detail_link_change()), 'focus on Change');
	});

	it('says it is linking while the link is out', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		server.use(
			http.post(`${API_BASE}/api/history/link`, async ({ request }) => {
				const body = (await request.json()) as { card: string; rows: string[]; kitsu_id: string };
				linkCalls.push(body);
				await held;
				return HttpResponse.json({ kitsu_id: body.kitsu_id, linked: [] });
			})
		);
		await linkToTheRightShow();
		await until(() => button(m.detail_link_linking()) !== null, 'the linking label');
		release();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the show page');
	});
});

describe('from Home to a relinked card', () => {
	it('a guessed card’s play leads up to its show page, which relinks the card', async () => {
		// Home: the guessed card, shown as the guess.
		setUrl('/');
		app = mount(HomePage, { target });
		const card = () =>
			Array.from(target.querySelectorAll<HTMLButtonElement>('button.resume-card')).find((b) =>
				(b.textContent ?? '').includes(GUESS_TITLE)
			) ?? null;
		await until(() => card() !== null, 'the guessed card');
		card()!.click();
		await until(() => FakeEventSource.instances.length > 0, "the card's play");
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/file.mp4`,
				media_kind: 'mp4'
			})
		);
		await until(
			() => String(vi.mocked(goto).mock.calls.at(-1)?.[0] ?? '').startsWith(`/play/${GUESS}`),
			'the player'
		);
		const played = new URL(String(vi.mocked(goto).mock.calls.at(-1)?.[0]), 'http://app.test');
		expect(played.searchParams.get('guess')).toBe('1');
		unmount(app);
		app = null;

		// The player: its link up to the show page carries the card's row.
		setParams({ id: GUESS });
		app = mount(PlayPage, { target });
		const up = () => target.querySelector<HTMLAnchorElement>('a.np-link');
		await until(() => (up()?.textContent ?? '').includes(GUESS_TITLE), 'the now-playing link');
		await settle();
		const href = new URL(up()!.getAttribute('href') ?? '', 'http://app.test');
		expect(href.pathname).toBe(`/anime/${GUESS}`);
		expect(href.searchParams.get('guessed')).toBe(CARD);
		unmount(app);
		app = null;

		// The show page: the line, Change, the right show, Link.
		await openShowPage(GUESS, Object.fromEntries(href.searchParams));
		await until(hasNote, 'the matched-from line');
		button(m.detail_link_change())!.click();
		const right = () =>
			Array.from(target.querySelectorAll<HTMLButtonElement>('dialog li button')).find((b) =>
				/^Black Clover\s+2017\b/.test((b.textContent ?? '').trim())
			);
		await until(() => right() !== undefined, 'the right show among the candidates');
		right()!.click();
		await until(() => {
			const link = button(m.detail_link_confirm_button());
			return link !== null && !link.disabled;
		}, 'the Link button');
		button(m.detail_link_confirm_button())!.click();
		await until(() => linkCalls.length === 1, 'the link');
		expect(linkCalls[0]).toEqual({ card: CARD, rows: [CARD, HIDDEN], kitsu_id: RIGHT });

		// Home again: the card is the right show.
		expect(await homeCards()).toEqual([TITLE]);
	});
});

describe('a show page on screen when a link answers elsewhere', () => {
	// A link can answer after the page it was made on was left — a pick
	// on the player saved once the viewer went to a show page. The page
	// on screen reads its history again, so its Continue follows the rows.
	it("reads the show's history again, and Continue comes with the link", async () => {
		await openShowPage(RIGHT);
		await until(
			() => (target.textContent ?? '').includes(m.detail_play_button_episode_one()),
			'Play from episode 1'
		);
		await appLinkDeps().link(CARD, [CARD, HIDDEN], RIGHT);
		await until(
			() => (target.textContent ?? '').includes(m.detail_play_button_continue()),
			'Continue from the linked rows'
		);
	});

	it("keeps Continue gone when the page's first lookup, sent before a link moved the rows away, answers after it", async () => {
		rows = rows.map((r) => ({ ...r, kitsu_id: RIGHT }));
		// The page's first lookup reads the rows as they are, then is held.
		let releaseFirst: () => void = () => {};
		const held = new Promise<void>((resolve) => {
			releaseFirst = resolve;
		});
		let asks = 0;
		server.use(
			http.get(`${API_BASE}/api/history/by-kitsu/:id`, async ({ params }) => {
				const found = rows.find((r) => r.kitsu_id === params.id) ?? null;
				if (++asks === 1) await held;
				return HttpResponse.json(found);
			})
		);
		await openShowPage(RIGHT);
		await until(() => asks === 1, "the page's first lookup");
		// A link elsewhere records the rows as another show.
		await appLinkDeps().link(CARD, [CARD, HIDDEN], GUESS);
		// The link's own lookup has answered, an empty answer asked twice
		// ($lib/detail/resume-lookup), before the first one does.
		await until(() => asks === 3, "the link's lookup");
		await settle();
		releaseFirst();
		await settle();
		expect(target.textContent).not.toContain(m.detail_play_button_continue());
		expect(target.textContent).toContain(m.detail_play_button_episode_one());
	});
});

describe('the audio a correction from the show page carries', () => {
	it('is the shown show’s, even when the link answers on another show’s page', async () => {
		rememberAudio(GUESS, 'dub');
		rememberAudio('777', 'sub');
		let release: () => void = () => {};
		const saved = new Promise<void>((resolve) => {
			release = resolve;
		});
		server.use(
			http.post(`${API_BASE}/api/history/link`, async () => {
				await saved;
			})
		);
		await openShowPage(GUESS, { guessed: CARD });
		await until(hasNote, 'the matched-from line');
		button(m.detail_link_change())!.click();
		await until(() => target.querySelector('dialog') !== null, 'the picker');
		const row = () =>
			Array.from(target.querySelectorAll('dialog li button')).find((b) =>
				/^Black Clover\s+2017\b/.test((b.textContent ?? '').trim())
			) as HTMLButtonElement | undefined;
		await until(() => row() !== undefined, 'the right show among the candidates');
		row()!.click();
		await until(
			() => button(m.detail_link_confirm_button())?.disabled === false,
			'the Link button'
		);
		button(m.detail_link_confirm_button())!.click();
		// Another show's page opens while the link is out.
		setUrl('/anime/777');
		setParams({ id: '777' });
		release();
		await until(() => linkCalls.length === 1, 'the link');
		await until(() => storedAudio(RIGHT) !== undefined, 'the audio carried');
		expect(storedAudio(RIGHT)).toBe('dub');
	});
});
