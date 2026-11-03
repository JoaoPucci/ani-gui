// Acceptance: a guessed Continue card's own ways to the change-show
// picker, on Home.
//
// A guessed card carries, on hover or keyboard focus only, a "Not this
// show?" pill with the ≈ mark at the corner the delete chip leaves
// free; a card whose show the row recorded carries none, and no card
// carries an always-on mark. The pill opens the picker in place, and a
// link reads the cards again where they stand. Black Clover's guess —
// Kitsu's one-episode Jump Festa special — is not on the streaming
// source, so its resume fails on Home and never reaches the player: the
// failed resume says the show was guessed and offers "Pick the right
// show" beside "Got it", with focus on the offer.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { linkBackend, linkBackendHandlers, type LinkBackend } from './link-backend';
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

import HomePage from '../../src/routes/+page.svelte';
import { goto } from '$app/navigation';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { toastStore } from '../../src/lib/toasts/store.svelte';

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

const BC_TITLE = 'Black Clover';
/** Kitsu's one-episode special, the guess for Black Clover's card. */
const BC_GUESS = '12625';
const BC_GUESS_TITLE = 'Black Clover: Jump Festa 2016 Special';
/** The TV series the card really is. */
const BC_RIGHT = '13209';
const BC_RIGHT_TITLE = 'Black Clover (TV)';
const BC_CARD = 'hianime:black-clover-949';
/** A retired provider's bare row of the same listing, hidden behind the card. */
const BC_HIDDEN = 'black-clover-77';

const JOJO_TITLE = "JoJo's Bizarre Adventure";
const JOJO_GUESS = '7158';
const JOJO_GUESS_TITLE = 'JoJo no Kimyou na Bouken (TV)';
const JOJO_RIGHT = '4013';
const JOJO_RIGHT_TITLE = 'JoJo no Kimyou na Bouken (2000)';
const JOJO_CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const JOJO_PLAYED = 'hianime:jojos-bizarre-adventure-2012-708';

/** A show the row recorded: its card is no guess. */
const KNOWN = 'hianime:gintama-1237';
const KNOWN_ID = '818';

let backend: LinkBackend;
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function show(id: string) {
	const shows: Record<string, ReturnType<typeof kitsuRef> & { subtype?: string }> = {
		[BC_GUESS]: { ...kitsuRef(BC_GUESS, BC_GUESS_TITLE, 1), subtype: 'special' },
		[BC_RIGHT]: { ...kitsuRef(BC_RIGHT, BC_RIGHT_TITLE, 170), titles: { en: BC_TITLE } },
		[JOJO_GUESS]: { ...kitsuRef(JOJO_GUESS, JOJO_GUESS_TITLE, 26), titles: { en: JOJO_TITLE } },
		[JOJO_RIGHT]: {
			...kitsuRef(JOJO_RIGHT, JOJO_RIGHT_TITLE, 13),
			titles: { en: JOJO_TITLE },
			subtype: 'OVA'
		},
		[KNOWN_ID]: kitsuRef(KNOWN_ID, 'Gintama', 201)
	};
	return shows[id];
}

function serve() {
	server.use(
		...homeHandlers({}, [
			...linkBackendHandlers(backend),
			// The JoJo card's own row leads here, watched last, and the
			// 2012 listing's row hides behind it. The case where the 2012
			// row was written by the card's own guessed play, and watched
			// later, is below ("the line clicked").
			http.get(`${API_BASE}/api/watched-at`, () =>
				HttpResponse.json({ [JOJO_CARD]: 2_000, [JOJO_PLAYED]: 1_000 })
			),
			http.get(`${API_BASE}/api/title-match`, ({ request }) => {
				const title = new URL(request.url).searchParams.get('title') ?? '';
				if (title.startsWith(BC_TITLE)) return HttpResponse.json(BC_GUESS);
				if (title.startsWith(JOJO_TITLE)) return HttpResponse.json(JOJO_GUESS);
				return HttpResponse.json(null);
			}),
			http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
				HttpResponse.json(show(String(params.id)))
			),
			http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
				HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const { query } = (await request.json()) as { query: string };
				if (query.toLowerCase().startsWith('black clover')) {
					return HttpResponse.json([show(BC_GUESS), show(BC_RIGHT)]);
				}
				if (query.includes('2000')) return HttpResponse.json([show(JOJO_RIGHT), show(JOJO_GUESS)]);
				if (query.toLowerCase().startsWith('jojo')) return HttpResponse.json([show(JOJO_GUESS)]);
				return HttpResponse.json([]);
			}),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
		])
	);
}

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	for (const t of toastStore.items) toastStore.dismiss(t.id);
	backend = linkBackend([
		{ ep_no: '1', id: BC_CARD, title: BC_TITLE },
		{ ep_no: '1', id: BC_HIDDEN, title: BC_TITLE },
		{ ep_no: '2', id: JOJO_PLAYED, title: JOJO_TITLE },
		{ ep_no: '1', id: JOJO_CARD, title: JOJO_TITLE },
		{ ep_no: '5', id: KNOWN, title: 'Gintama', kitsu_id: KNOWN_ID }
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
const cell = (row: string) =>
	target.querySelector<HTMLElement>(`.resume-cell[data-entry-id="${CSS.escape(row)}"]`);
const pill = (row: string) => cell(row)?.querySelector<HTMLButtonElement>('.resume-link') ?? null;
const showLine = (row: string) => cell(row)?.querySelector('.resume-show')?.textContent ?? '';
const dialog = () => target.querySelector('dialog');
const button = (label: string, within: ParentNode = target) =>
	Array.from(within.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').trim().includes(label)
	) ?? null;
const toastWith = (message: string) => toastStore.items.find((t) => t.message === message);

async function openHome() {
	app = mount(HomePage, { target });
	await until(
		() =>
			showLine(BC_CARD).includes(BC_GUESS_TITLE) &&
			showLine(JOJO_CARD).includes(JOJO_GUESS_TITLE) &&
			showLine(KNOWN).includes('Gintama'),
		'the Continue cards'
	);
	await settle();
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

describe("a guessed Continue card's pill", () => {
	it('is on guessed cards only, with the ≈ mark, and no card carries an always-on mark', async () => {
		await openHome();
		expect(pill(BC_CARD)?.textContent).toContain(m.detail_link_not_this());
		expect(pill(BC_CARD)!.querySelector('svg[data-guess-mark]')).not.toBeNull();
		expect(pill(JOJO_CARD)).not.toBeNull();
		expect(pill(KNOWN)).toBeNull();
		// The mark is the pill's alone.
		for (const card of target.querySelectorAll('.resume-card')) {
			expect(card.querySelector('[data-guess-mark]')).toBeNull();
		}
	});

	it('opens the picker in place, on the card it was made from', async () => {
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		const text = dialog()!.textContent ?? '';
		expect(text).toContain(m.detail_link_dialog_title({ title: BC_TITLE }));
		// The card offers episode 2 (the row is at 1): the picker says so.
		expect(text).toContain(m.detail_link_dialog_body({ episode: '2' }));
		// No show page is open: the guess is the one on the card.
		await until(
			() => (dialog()!.textContent ?? '').includes(m.home_link_on_card()),
			'the current show labelled'
		);
		expect(dialog()!.textContent).not.toContain(m.detail_link_shown_now());
		expect(goto).not.toHaveBeenCalled();
	});

	it("links the card's listing and reads the card again where it stands; Undo puts it back", async () => {
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(BC_RIGHT_TITLE);
		// The card's own row and the bare row of its listing.
		expect(backend.rows.filter((r) => r.kitsu_id === BC_RIGHT).map((r) => r.id)).toEqual([
			BC_CARD,
			BC_HIDDEN
		]);
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the linked show');
		expect(pill(BC_CARD)).toBeNull();
		expect(goto).not.toHaveBeenCalled();
		const toast = toastWith(m.detail_link_done_toast({ title: BC_RIGHT_TITLE }))!;
		expect(toast.actionLabel).toBe(m.detail_link_undo());
		toast.onAction!();
		await until(() => backend.unlinkCalls.length === 1, 'the undo');
		await until(() => showLine(BC_CARD).includes(BC_GUESS_TITLE), 'the card as the guess again');
	});

	it('links only the JoJo card, and the 2012 row comes out as a card of its own', async () => {
		await openHome();
		expect(cell(JOJO_PLAYED)).toBeNull();
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		expect(backend.linkCalls[0].card).toBe(JOJO_CARD);
		expect(backend.rows.find((r) => r.id === JOJO_CARD)?.kitsu_id).toBe(JOJO_RIGHT);
		expect(backend.rows.find((r) => r.id === JOJO_PLAYED)?.kitsu_id).toBeUndefined();
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the card as the OVA');
		await until(
			() => showLine(JOJO_PLAYED).includes(JOJO_GUESS_TITLE),
			'the 2012 row as the 2012 series'
		);
	});
});

describe("a guessed card's failed resume", () => {
	async function failResume() {
		await openHome();
		const card = cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!;
		card.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		return target.querySelector<HTMLElement>('[role="alertdialog"]')!;
	}

	it('says the show was guessed and offers the picker, focused, beside Got it', async () => {
		const overlay = await failResume();
		expect(overlay.textContent).toContain(m.home_resume_guess_note({ title: BC_TITLE }));
		const pick = button(m.home_link_pick_button(), overlay)!;
		expect(pick.querySelector('svg[data-guess-mark]')).not.toBeNull();
		expect(button(m.errors_dismiss_default_label(), overlay)).not.toBeNull();
		await until(() => document.activeElement === pick, 'focus on Pick the right show');
	});

	it('opens the picker from the overlay, which goes', async () => {
		const overlay = await failResume();
		button(m.home_link_pick_button(), overlay)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		expect(target.querySelector('[role="alertdialog"]')).toBeNull();
		await pickAndLink(BC_RIGHT_TITLE);
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the linked show');
	});

	it('closes on Got it, leaving the viewer on Home and the card as it was', async () => {
		const overlay = await failResume();
		button(m.errors_dismiss_default_label(), overlay)!.click();
		await until(() => target.querySelector('[role="alertdialog"]') === null, 'the overlay to go');
		await settle();
		expect(dialog()).toBeNull();
		expect(goto).not.toHaveBeenCalled();
		expect(backend.linkCalls).toEqual([]);
		expect(showLine(BC_CARD)).toContain(BC_GUESS_TITLE);
	});

	it('says nothing of a guess for a card whose show the row recorded', async () => {
		await openHome();
		cell(KNOWN)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		const overlay = target.querySelector<HTMLElement>('[role="alertdialog"]')!;
		expect(button(m.home_link_pick_button(), overlay)).toBeNull();
		expect(overlay.textContent).not.toContain(m.home_resume_guess_note({ title: 'Gintama' }));
	});
});

describe('a card whose guessed play wrote a row on another listing stands for the line clicked', () => {
	// JoJo's 2000 card, guessed as the 2012 series, played the 2012
	// series, whose resolve landed on the 2012 listing and wrote that row
	// later than the card's own. The backend names the card that row
	// came from (`guessed_from`). The merged card still shows the latest
	// progress, and stands for the 2000 line: its pill, its failed resume
	// and its player all correct that line.
	beforeEach(() => {
		backend.rows = backend.rows.map((r) =>
			r.id === JOJO_PLAYED ? { ...r, guessed_from: JOJO_CARD } : r
		);
		server.use(
			http.get(`${API_BASE}/api/watched-at`, () =>
				HttpResponse.json({ [JOJO_CARD]: 1_000, [JOJO_PLAYED]: 2_000 })
			)
		);
	});

	async function openMerged() {
		app = mount(HomePage, { target });
		await until(() => showLine(JOJO_PLAYED).includes(JOJO_GUESS_TITLE), 'the merged JoJo card');
		await settle();
		// One card for the two lines: the newer row's.
		expect(cell(JOJO_CARD)).toBeNull();
	}

	it('its pill links the 2000 line alone; then each line is a card of its own show', async () => {
		await openMerged();
		pill(JOJO_PLAYED)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		expect(backend.linkCalls[0].card).toBe(JOJO_CARD);
		expect(backend.linkCalls[0].rows).toEqual([JOJO_CARD]);
		expect(backend.rows.find((r) => r.id === JOJO_CARD)?.kitsu_id).toBe(JOJO_RIGHT);
		expect(backend.rows.find((r) => r.id === JOJO_PLAYED)?.kitsu_id).toBeUndefined();
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the 2000 line as the OVA');
		await until(
			() => showLine(JOJO_PLAYED).includes(JOJO_GUESS_TITLE),
			'the 2012 row as the 2012 series'
		);
	});

	it('its resume plays as a guess of the 2000 line, from the newer progress', async () => {
		await openMerged();
		cell(JOJO_PLAYED)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		const url = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(url.searchParams.get('guess_row')).toBe(JOJO_CARD);
		// The 2012 row is at episode 2: Continue goes on to 3.
		expect(url.searchParams.get('episode')).toBe('3');
	});

	it("its failed resume picks the 2000 line's show", async () => {
		await openMerged();
		cell(JOJO_PLAYED)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		const overlay = target.querySelector<HTMLElement>('[role="alertdialog"]')!;
		await until(() => button(m.home_link_pick_button(), overlay) !== null, 'the pick button');
		button(m.home_link_pick_button(), overlay)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		expect(backend.linkCalls[0].card).toBe(JOJO_CARD);
		expect(backend.linkCalls[0].rows).toEqual([JOJO_CARD]);
	});
});

describe("a guessed card's player is told where its play landed", () => {
	async function resumeJojo(ownListing: boolean) {
		await openHome();
		cell(JOJO_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances.at(-1)!.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-1',
				master_url: '/s/t/master.m3u8',
				media_kind: 'hls',
				own_listing: ownListing
			})
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the player');
		return String(vi.mocked(goto).mock.calls.at(-1)?.[0]);
	}

	it("as another listing than the card's", async () => {
		const href = await resumeJojo(false);
		expect(new URL(href, 'http://x').searchParams.get('landed')).toBe('other');
	});

	it("as the card's own listing", async () => {
		const href = await resumeJojo(true);
		expect(new URL(href, 'http://x').searchParams.get('landed')).toBeNull();
	});
});
