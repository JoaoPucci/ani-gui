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
import { isModalOpen } from '../../src/lib/components/modal-layer';
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
		expect(text).toContain(m.detail_link_dialog_title());
		// The card offers episode 2 (the row is at 1): the picker says so,
		// with the title the history saved.
		expect(text).toContain(m.detail_link_dialog_body({ episode: '2', title: BC_TITLE }));
		// No show page is open: the guess is the one on the card.
		await until(
			() => (dialog()!.textContent ?? '').includes(m.home_link_on_card()),
			'the current show labelled'
		);
		expect(dialog()!.textContent).not.toContain(m.detail_link_shown_now());
		expect(goto).not.toHaveBeenCalled();
	});

	it('moves focus to Link on a pick, and back to the picked row on Back', async () => {
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		const rowOf = () =>
			Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).find((b) =>
				(b.textContent ?? '').includes(BC_RIGHT_TITLE)
			);
		await until(() => rowOf() !== undefined, 'the series among the candidates');
		const id = rowOf()!.dataset.id!;
		rowOf()!.click();
		await until(
			() => document.activeElement === button(m.detail_link_confirm_button()),
			'focus on Link'
		);
		button(m.detail_link_back_button())!.click();
		await until(
			() =>
				document.activeElement === dialog()?.querySelector(`button[data-id="${CSS.escape(id)}"]`),
			"focus on the picked show's row"
		);
		expect(id).toBe(BC_RIGHT);
	});

	it("links the card's listing and reads the card again where it stands", async () => {
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
		expect(goto).not.toHaveBeenCalled();
		// The card keeps its pill, to change the show again.
		await until(() => pill(BC_CARD) !== null, 'the pill, to change it again');
	});

	it('says the show was saved, with no Undo on the notice', async () => {
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(BC_RIGHT_TITLE);
		await until(
			() => toastWith(m.detail_link_done_toast({ title: BC_RIGHT_TITLE })) !== undefined,
			'the saved notice'
		);
		const toast = toastWith(m.detail_link_done_toast({ title: BC_RIGHT_TITLE }))!;
		expect(toast.actionLabel).toBeNull();
		expect(toast.onAction).toBeNull();
	});

	it('carries the card’s audio to the show picked for it, when that show has none', async () => {
		rememberAudio(BC_GUESS, 'dub');
		// The OVA's own choice differs from what JoJo's card plays in.
		rememberAudio(JOJO_RIGHT, 'dub');
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(BC_RIGHT_TITLE);
		await until(() => storedAudio(BC_RIGHT) !== undefined, 'the audio carried');
		expect(storedAudio(BC_RIGHT)).toBe('dub');
		// A show with its own choice keeps it.
		const linked = backend.linkCalls.length;
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, "JoJo's picker");
		await until(
			() =>
				Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).some((b) =>
					(b.textContent ?? '').includes(JOJO_RIGHT_TITLE)
				),
			'the OVA among the candidates'
		);
		Array.from(dialog()!.querySelectorAll<HTMLButtonElement>('li button'))
			.find((b) => (b.textContent ?? '').includes(JOJO_RIGHT_TITLE))!
			.click();
		await until(() => !button(m.detail_link_confirm_button())?.disabled, 'the Link button');
		button(m.detail_link_confirm_button())!.click();
		await until(() => backend.linkCalls.length > linked, 'the second link');
		await settle();
		expect(storedAudio(JOJO_RIGHT)).toBe('dub');
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

describe('a card that stopped being a guess while Home was open', () => {
	it('drops its pill rather than open nothing', async () => {
		await openHome();
		// Linked from elsewhere since Home read the cards.
		backend.rows = backend.rows.map((r) =>
			r.id === JOJO_CARD ? { ...r, kitsu_id: JOJO_RIGHT } : r
		);
		pill(JOJO_CARD)!.click();
		await until(() => pill(JOJO_CARD) === null, 'the pill to go');
		expect(dialog()).toBeNull();
	});

	it('keeps its pill when the check cannot read the history, and opens on the next click', async () => {
		await openHome();
		// The database is busy for the check's read, once.
		server.use(
			http.get(`${API_BASE}/api/history`, () => new HttpResponse(null, { status: 503 }), {
				once: true
			})
		);
		pill(JOJO_CARD)!.click();
		await until(() => toastWith(m.home_link_check_failed()) !== undefined, 'the notice');
		expect(pill(JOJO_CARD)).not.toBeNull();
		expect(dialog()?.open ?? false).toBe(false);
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
	});

	it("keeps its pill when Kitsu cannot answer the card's match, and says so", async () => {
		await openHome();
		// Kitsu fails every read while the check is out.
		let kitsuDown = true;
		const down = () => (kitsuDown ? new HttpResponse(null, { status: 503 }) : undefined);
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/:id`, down),
			http.post(`${API_BASE}/api/kitsu/search`, down),
			http.get(`${API_BASE}/api/title-match`, down)
		);
		pill(JOJO_CARD)!.click();
		await until(() => toastWith(m.home_link_check_failed()) !== undefined, 'the notice');
		expect(pill(JOJO_CARD)).not.toBeNull();
		expect(dialog()?.open ?? false).toBe(false);
		kitsuDown = false;
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
	});

	it('offers no pick on its failed resume', async () => {
		await openHome();
		backend.rows = backend.rows.map((r) => (r.id === BC_CARD ? { ...r, kitsu_id: BC_RIGHT } : r));
		cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		await settle();
		const overlay = target.querySelector<HTMLElement>('[role="alertdialog"]')!;
		expect(button(m.home_link_pick_button(), overlay)).toBeNull();
		expect(overlay.textContent).not.toContain(m.home_resume_guess_note({ title: BC_TITLE }));
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

	it('gives focus back to the card when the picker it opened closes', async () => {
		const overlay = await failResume();
		button(m.home_link_pick_button(), overlay)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		button(m.detail_link_cancel_button())!.click();
		await until(() => dialog() === null || dialog()?.open === false, 'the picker to close');
		const card = cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!;
		await until(() => document.activeElement === card, 'focus on the card');
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

describe("a link forgets what Home kept for the card's former show", () => {
	// The special has one episode, the series 170: each probe answers
	// for the show it is asked about.
	beforeEach(() => {
		server.use(
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const { kitsu_id } = (await request.json()) as { kitsu_id?: string };
				const count = kitsu_id === BC_GUESS ? 1 : kitsu_id === BC_RIGHT ? 170 : 26;
				return HttpResponse.json({ available: true, episode_count: count });
			})
		);
	});

	it("a failed special's exact cap does not hold the linked series at episode 1", async () => {
		await openHome();
		const card = () => cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!;
		// The special's resume asks for its one episode, and fails: the
		// click pinned that cap as exact.
		card().click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		const first = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(first.searchParams.get('episode')).toBe('1');
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
		await pickAndLink(BC_RIGHT_TITLE);
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the series');
		await settle();
		// Continue goes on to episode 2 of the series.
		const before = FakeEventSource.instances.length;
		card().click();
		await until(() => FakeEventSource.instances.length > before, 'the second resume');
		const second = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(second.searchParams.get('episode')).toBe('2');
	});
});

describe("a probe still on its way for the card's former show", () => {
	// The first load's availability probe for JoJo's guess is held: the
	// card is drawn and takes its pill from the match alone, so the user
	// can link it before the probe answers.
	let answerProbe: () => void = () => {};
	beforeEach(() => {
		const answered = new Promise<void>((resolve) => {
			answerProbe = resolve;
		});
		server.use(
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const { kitsu_id } = (await request.json()) as { kitsu_id?: string };
				if (kitsu_id === JOJO_GUESS) await answered;
				return HttpResponse.json({ available: true, episode_count: 26 });
			})
		);
	});

	it('lands without putting the guess back on the linked card', async () => {
		await openHome();
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the card as the OVA');
		answerProbe();
		await settle();
		expect(showLine(JOJO_CARD)).toContain(JOJO_RIGHT_TITLE);
		// Its next resume is a play of the OVA.
		cell(JOJO_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		const url = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(url.searchParams.get('kitsu_id')).toBe(JOJO_RIGHT);
	});
});

describe("the cap retry for a card's former show", () => {
	// JoJo's guess answers its first probe unconfirmed, so the row goes
	// into the cap retry, which starts once every first probe answered.
	// Black Clover's first probe is held until the card is linked, so the
	// retry starts after the link. Asked again, the guess would confirm a
	// cap of 1 — the series' would hold the OVA at episode 1.
	let releaseBc: () => void = () => {};
	let guessProbes = 0;
	beforeEach(() => {
		guessProbes = 0;
		// The 2012 row probes the same guess, legitimately: leave it out.
		backend.rows = backend.rows.filter((r) => r.id !== JOJO_PLAYED);
		const bcHeld = new Promise<void>((resolve) => {
			releaseBc = resolve;
		});
		server.use(
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const { kitsu_id } = (await request.json()) as { kitsu_id?: string };
				if (kitsu_id === BC_GUESS) await bcHeld;
				if (kitsu_id === JOJO_GUESS) {
					guessProbes++;
					return HttpResponse.json({
						available: true,
						episode_count: 1,
						episode_count_approximate: guessProbes === 1
					});
				}
				const count = kitsu_id === JOJO_RIGHT ? 13 : 26;
				return HttpResponse.json({ available: true, episode_count: count });
			})
		);
	});

	it('asks nothing more about the guess once the card is linked, and keeps the linked cap', async () => {
		await openHome();
		expect(guessProbes).toBe(1);
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the card as the OVA');
		const asked = guessProbes;
		// The retry starts, and its first step (half a second) passes.
		releaseBc();
		await new Promise((r) => setTimeout(r, 1500));
		expect(guessProbes).toBe(asked);
		// The card still has the OVA's cap: Continue goes on to episode 2.
		cell(JOJO_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		const url = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(url.searchParams.get('kitsu_id')).toBe(JOJO_RIGHT);
		expect(url.searchParams.get('episode')).toBe('2');
	}, 15_000);
});

describe('a guessed card whose resume is running', () => {
	it('cannot open its picker until the resume ends', async () => {
		await openHome();
		cell(JOJO_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		expect(pill(JOJO_CARD)!.disabled).toBe(true);
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		await until(() => pill(JOJO_CARD)?.disabled === false, 'the pill back');
	});
});

describe('a card linked while its first probe waits in the queue', () => {
	// Four rows of a recorded show take the four probe slots first and
	// hold them; JoJo's card, matched by a slower title search, waits in
	// the queue behind them, and is linked there.
	let releaseSlots: () => void = () => {};
	let guessProbes = 0;
	beforeEach(() => {
		guessProbes = 0;
		backend.rows = [
			{ ep_no: '1', id: JOJO_CARD, title: JOJO_TITLE },
			...['1', '2', '3', '4'].map((i) => ({
				ep_no: '5',
				id: `hianime:gintama-${i}`,
				title: 'Gintama',
				kitsu_id: KNOWN_ID
			}))
		];
		const slots = new Promise<void>((resolve) => {
			releaseSlots = resolve;
		});
		server.use(
			http.get(`${API_BASE}/api/title-match`, async ({ request }) => {
				const title = new URL(request.url).searchParams.get('title') ?? '';
				if (!title.startsWith(JOJO_TITLE)) return HttpResponse.json(null);
				await new Promise((r) => setTimeout(r, 200));
				return HttpResponse.json(JOJO_GUESS);
			}),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const { kitsu_id } = (await request.json()) as { kitsu_id?: string };
				if (kitsu_id === KNOWN_ID) await slots;
				if (kitsu_id === JOJO_GUESS) guessProbes++;
				return HttpResponse.json({ available: true, episode_count: 26 });
			})
		);
	});

	it('is not probed for its former show once a slot frees', async () => {
		app = mount(HomePage, { target });
		await until(() => pill(JOJO_CARD) !== null, 'the JoJo card and its pill');
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the card as the OVA');
		const asked = guessProbes;
		releaseSlots();
		await new Promise((r) => setTimeout(r, 800));
		expect(guessProbes).toBe(asked);
		expect(showLine(JOJO_CARD)).toContain(JOJO_RIGHT_TITLE);
	});
});

describe('a link that answers after the user left Home', () => {
	// The link's notice and its write outlive the Home it was made on:
	// the Home the user came back to reads its cards again when it lands.
	it('shows the card as the linked show on the Home the user came back to', async () => {
		let release: () => void = () => {};
		const saved = new Promise<void>((resolve) => {
			release = resolve;
		});
		// Held, then answered by the fake backend's own route.
		server.use(
			http.post(`${API_BASE}/api/history/link`, async () => {
				await saved;
			})
		);
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		const row = () =>
			Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).find((b) =>
				(b.textContent ?? '').includes(BC_RIGHT_TITLE)
			);
		await until(() => row() !== undefined, 'the series among the candidates');
		row()!.click();
		await until(() => !button(m.detail_link_confirm_button())?.disabled, 'the Link button');
		button(m.detail_link_confirm_button())!.click();
		// Home is left while the link is out, and opened again.
		unmount(app!);
		app = null;
		await openHome();
		release();
		await until(() => backend.linkCalls.length === 1, 'the link');
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the series');
		expect(toastWith(m.detail_link_done_toast({ title: BC_RIGHT_TITLE }))).toBeDefined();
	});
});

describe('two pills clicked close together', () => {
	// Each pill checks its card's guess again before its picker opens,
	// reading the history. JoJo's pill is clicked, then Black Clover's,
	// and JoJo's check answers first, while Black Clover's is still out.
	const held: (() => void)[] = [];
	let hold = false;
	beforeEach(() => {
		hold = false;
		held.length = 0;
		// Held reads answer nothing on release: the fake backend's own
		// route answers them.
		server.use(
			http.get(`${API_BASE}/api/history`, async () => {
				if (hold) await new Promise<void>((resolve) => held.push(resolve));
			})
		);
	});

	it("leaves the first card's pill, and opens the later card's picker", async () => {
		await openHome();
		hold = true;
		pill(JOJO_CARD)!.click();
		await until(() => held.length === 1, "JoJo's check");
		pill(BC_CARD)!.click();
		await until(() => held.length === 2, "Black Clover's check");
		hold = false;
		held[0]();
		await settle();
		// JoJo's card is no less a guess for being overtaken.
		expect(pill(JOJO_CARD)).not.toBeNull();
		held[1]();
		await until(() => dialog()?.open === true, "Black Clover's picker");
		expect(dialog()!.textContent).toContain(
			m.detail_link_dialog_body({ episode: '2', title: BC_TITLE })
		);
		await settle();
		expect(pill(JOJO_CARD)).not.toBeNull();
	});
});

describe('the picker while Clear history runs', () => {
	// Clear history holds its dialog open while the clear is out, and
	// opens no other dialog over it: a guessed card's pill neither.
	let releaseClear: () => void = () => {};
	let clearAsked = false;
	beforeEach(() => {
		clearAsked = false;
		const cleared = new Promise<void>((resolve) => {
			releaseClear = resolve;
		});
		server.use(
			http.delete(`${API_BASE}/api/history`, async () => {
				clearAsked = true;
				await cleared;
				backend.rows = [];
				return new HttpResponse(null, { status: 204 });
			})
		);
	});

	it('is not opened by a pill whose check answers once a clear is running', async () => {
		await openHome();
		// The pill's check reads the history, held until the clear runs.
		let releaseRead: () => void = () => {};
		let readAsked = false;
		const read = new Promise<void>((resolve) => {
			releaseRead = resolve;
		});
		server.use(
			http.get(`${API_BASE}/api/history`, async () => {
				readAsked = true;
				await read;
			})
		);
		pill(BC_CARD)!.click();
		await until(() => readAsked, "the pill's check");
		button(m.home_clear_history_action())!.click();
		await until(() => button(m.home_clear_confirm_button()) !== null, 'the confirm dialog');
		button(m.home_clear_confirm_button())!.click();
		await until(() => clearAsked, 'the clear');
		releaseRead();
		await settle();
		expect(dialog()?.open).not.toBe(true);
		releaseClear();
		await until(() => cell(BC_CARD) === null, 'the cards cleared');
		await settle();
		expect(dialog()?.open).not.toBe(true);
	});

	it('is not opened from a pill until the clear has answered', async () => {
		await openHome();
		button(m.home_clear_history_action())!.click();
		await until(() => button(m.home_clear_confirm_button()) !== null, 'the confirm dialog');
		button(m.home_clear_confirm_button())!.click();
		await until(() => clearAsked, 'the clear');
		// A keyboard user reaches the pill behind the busy dialog.
		const p = pill(BC_CARD)!;
		expect(p.disabled).toBe(true);
		p.disabled = false;
		p.click();
		await settle();
		expect(dialog()?.open).not.toBe(true);
		releaseClear();
		await until(() => cell(BC_CARD) === null, 'the cards cleared');
		expect(dialog()?.open).not.toBe(true);
	});
});

describe('a card the user named a show for, after the app restarted', () => {
	it('keeps its pill, which opens the picker on the show it was named', async () => {
		backend.rows = backend.rows.map((r) =>
			r.id === BC_CARD || r.id === BC_HIDDEN ? { ...r, kitsu_id: BC_RIGHT, named: true } : r
		);
		app = mount(HomePage, { target });
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the named show');
		await settle();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await until(
			() => (dialog()!.textContent ?? '').includes(BC_RIGHT_TITLE),
			'the named show in the picker'
		);
		// A card whose show a play recorded, with no naming, has none.
		expect(pill(KNOWN)).toBeNull();
	});
});

describe('a resume clicked while a later answer is still matching the card', () => {
	// JoJo's card is named as the OVA from its pill, then answered again
	// as the 2012 series. While the card is matched as the series (its
	// detail held), it still shows the OVA and is clicked: the click
	// resumes the OVA, whose cap lookup (held too) answers 1 only once
	// the card is the series. That cap is the OVA's: the series' next
	// resume must not replay episode 1 because of it.
	let holdDetail = false;
	let releaseDetail: () => void = () => {};
	let releaseLookup: () => void = () => {};
	let detailAsks = 0;
	let lookupAsked = false;
	beforeEach(() => {
		holdDetail = false;
		detailAsks = 0;
		lookupAsked = false;
		const detail = new Promise<void>((resolve) => {
			releaseDetail = resolve;
		});
		const lookup = new Promise<void>((resolve) => {
			releaseLookup = resolve;
		});
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/:id`, async ({ params }) => {
				if (String(params.id) === JOJO_GUESS && holdDetail) {
					detailAsks++;
					await detail;
				}
				return HttpResponse.json(show(String(params.id)));
			}),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const { kitsu_id, background } = (await request.json()) as {
					kitsu_id?: string;
					background?: boolean;
				};
				if (kitsu_id === JOJO_RIGHT && background === false) {
					lookupAsked = true;
					await lookup;
					return HttpResponse.json({ available: true, episode_count: 1 });
				}
				return HttpResponse.json({ available: true, episode_count: 26 });
			})
		);
	});

	it("does not keep the OVA's cap on the card once it is the series", async () => {
		await openHome();
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		await pickAndLink(JOJO_RIGHT_TITLE);
		await until(() => showLine(JOJO_CARD).includes(JOJO_RIGHT_TITLE), 'the card as the OVA');
		await settle();
		// Answered again: the series, matched with its detail held.
		await until(() => pill(JOJO_CARD) !== null, 'the pill, to change it again');
		pill(JOJO_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker again');
		holdDetail = true;
		const calls = backend.linkCalls.length;
		const series = () =>
			Array.from(dialog()?.querySelectorAll<HTMLButtonElement>('li button') ?? []).find((b) =>
				(b.textContent ?? '').includes(JOJO_GUESS_TITLE)
			);
		await until(() => series() !== undefined, 'the series among the candidates');
		series()!.click();
		await until(() => !button(m.detail_link_confirm_button())?.disabled, 'the Link button');
		button(m.detail_link_confirm_button())!.click();
		await until(() => backend.linkCalls.length > calls, 'the second link');
		await until(() => detailAsks > 0, 'the card being matched again');
		// The card still shows the OVA, and is clicked.
		const card = () => cell(JOJO_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!;
		card().click();
		await until(() => lookupAsked, "the click's cap lookup");
		releaseDetail();
		await until(() => showLine(JOJO_CARD).includes(JOJO_GUESS_TITLE), 'the card as the series');
		releaseLookup();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		await settle();
		// The series' resume goes on to episode 2.
		const before = FakeEventSource.instances.length;
		card().click();
		await until(() => FakeEventSource.instances.length > before, 'the second resume');
		const url = new URL(FakeEventSource.instances.at(-1)!.url, 'http://x');
		expect(url.searchParams.get('kitsu_id')).toBe(JOJO_GUESS);
		expect(url.searchParams.get('episode')).toBe('2');
	});
});

describe("a named card's pill clicked while its new show is still being matched", () => {
	it('comes back once the card shows the show it was named', async () => {
		let releaseDetail: () => void = () => {};
		let hold = false;
		let asked = false;
		const detail = new Promise<void>((resolve) => {
			releaseDetail = resolve;
		});
		server.use(
			http.get(`${API_BASE}/api/kitsu/anime/:id`, async ({ params }) => {
				if (String(params.id) === BC_RIGHT && hold) {
					asked = true;
					await detail;
				}
				return HttpResponse.json(show(String(params.id)));
			})
		);
		await openHome();
		pill(BC_CARD)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		hold = true;
		await pickAndLink(BC_RIGHT_TITLE);
		await until(() => asked, 'the card being matched as the series');
		// The pill is there at once, the card still showing the special.
		await until(() => pill(BC_CARD) !== null, 'the pill');
		pill(BC_CARD)!.click();
		await settle();
		releaseDetail();
		await until(() => showLine(BC_CARD).includes(BC_RIGHT_TITLE), 'the card as the series');
		await settle();
		expect(pill(BC_CARD)).not.toBeNull();
	});
});

describe('the picker holds the keyboard while it is up', () => {
	// As every dialog of the app does ($lib/components/modal-layer): focus
	// goes in, Tab and Shift+Tab cycle inside it, the page behind it is
	// inert, Escape closes it, and focus goes back to what opened it.
	const inert = (el: Element | null) => el !== null && el.closest('[inert]') !== null;
	function press(key: string, shiftKey = false): boolean {
		const at = (document.activeElement ?? document.body) as HTMLElement;
		const ev = new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true });
		at.dispatchEvent(ev);
		return ev.defaultPrevented;
	}

	it('from a pill', async () => {
		await openHome();
		const opener = pill(BC_CARD)!;
		opener.focus();
		opener.click();
		await until(() => dialog()?.open === true, 'the picker');
		await until(() => dialog()!.contains(document.activeElement), 'focus in the picker');
		expect(isModalOpen()).toBe(true);
		expect(inert(cell(JOJO_CARD))).toBe(true);
		expect(inert(dialog())).toBe(false);
		for (const shift of [false, true, false, false, false, false, false, false]) {
			expect(press('Tab', shift)).toBe(true);
			expect(dialog()!.contains(document.activeElement)).toBe(true);
		}
		expect(press('Escape')).toBe(true);
		await until(() => dialog() === null || dialog()?.open === false, 'the picker to close');
		expect(isModalOpen()).toBe(false);
		expect(inert(cell(JOJO_CARD))).toBe(false);
		// Focus back on what opened it: the pill, or its card while the
		// pill shows on hover or focus only.
		const card = cell(BC_CARD)!.querySelector<HTMLElement>('.resume-card')!;
		await until(
			() => document.activeElement === opener || document.activeElement === card,
			'focus back on the card'
		);
	});

	it('opened from a failed resume, one modal at a time', async () => {
		await openHome();
		cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		FakeEventSource.instances
			.at(-1)!
			.dispatch('error', JSON.stringify({ kind: 'no_results', message: 'not found' }));
		await until(
			() => target.querySelector('[role="alertdialog"]') !== null,
			'the failed resume overlay'
		);
		const overlay = target.querySelector<HTMLElement>('[role="alertdialog"]')!;
		button(m.home_link_pick_button(), overlay)!.click();
		await until(() => dialog()?.open === true, 'the picker');
		expect(target.querySelector('[role="alertdialog"]')).toBeNull();
		await until(() => dialog()!.contains(document.activeElement), 'focus in the picker');
		expect(isModalOpen()).toBe(true);
		expect(press('Tab')).toBe(true);
		expect(dialog()!.contains(document.activeElement)).toBe(true);
		expect(press('Escape')).toBe(true);
		await until(() => dialog() === null || dialog()?.open === false, 'the picker to close');
		const card = cell(BC_CARD)!.querySelector<HTMLButtonElement>('.resume-card')!;
		await until(() => document.activeElement === card, 'focus back on the card');
		expect(isModalOpen()).toBe(false);
	});
});
