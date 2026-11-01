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
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

const TITLE = 'Black Clover';
/** The show the guess matched the card to. */
const GUESS = '12625';
const GUESS_TITLE = 'Black Clover Special';
/** The show the card really is. */
const RIGHT = '13209';
/** The card's row, and an older row of the same title it hides. */
const CARD = 'hianime:black-clover-949';
const HIDDEN = 'black-clover-77';

interface Row {
	ep_no: string;
	id: string;
	title: string;
	kitsu_id?: string;
}

let rows: Row[];
let linkCalls: { rows: string[]; kitsu_id: string }[];
let unlinkCalls: { kitsu_id: string; rows: { row: string; previous: string | null }[] }[];
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
		{ ep_no: '1', id: CARD, title: TITLE },
		{ ep_no: '3', id: HIDDEN, title: TITLE }
	];
	linkCalls = [];
	unlinkCalls = [];
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
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json(rows)),
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
				const body = (await request.json()) as { rows: string[]; kitsu_id: string };
				linkCalls.push(body);
				const linked = rows
					.filter((r) => body.rows.includes(r.id))
					.map((r) => ({
						row: r.id,
						previous: r.kitsu_id ?? null,
						ep_no: r.ep_no,
						watched_at: null
					}));
				rows = rows.map((r) => (body.rows.includes(r.id) ? { ...r, kitsu_id: body.kitsu_id } : r));
				return HttpResponse.json({ kitsu_id: body.kitsu_id, linked });
			}),
			http.post(`${API_BASE}/api/history/unlink`, async ({ request }) => {
				const body = (await request.json()) as {
					kitsu_id: string;
					rows: { row: string; previous: string | null }[];
				};
				unlinkCalls.push(body);
				rows = rows.map((r) => {
					const l = body.rows.find((x) => x.row === r.id);
					if (!l) return r;
					const { kitsu_id: _dropped, ...rest } = r;
					void _dropped;
					return l.previous ? { ...rest, kitsu_id: l.previous } : rest;
				});
				return HttpResponse.json({ restored: body.rows.map((r) => r.row), kept: [] });
			}),
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
		// Quiet: no dialog on arrival.
		expect(target.querySelector('[role="dialog"]')).toBeNull();
	});

	it('says nothing when opened any other way', async () => {
		await openShowPage(GUESS);
		await settle();
		expect(hasNote()).toBe(false);
		expect(button(m.detail_link_change())).toBeNull();
	});

	it('says nothing once the row records a show', async () => {
		rows = rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: GUESS } : r));
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
		expect(target.querySelector('[role="dialog"]')).toBeNull();
	});
});

/** On the guessed page, pick the right show and link it. */
async function linkToTheRightShow() {
	await openShowPage(GUESS, { guessed: CARD });
	await until(hasNote, 'the matched-from line');
	button(m.detail_link_change())!.click();
	await until(() => target.querySelector('[role="dialog"]') !== null, 'the picker');
	const row = () =>
		Array.from(target.querySelectorAll('[role="dialog"] li button')).find((b) =>
			/^Black Clover\s+2017\b/.test((b.textContent ?? '').trim())
		) as HTMLButtonElement | undefined;
	await until(() => row() !== undefined, 'the right show among the candidates');
	row()!.click();
	await until(
		() => (target.textContent ?? '').includes(m.detail_link_confirm_heading({ title: TITLE })),
		'the confirmation'
	);
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

describe('changing the show', () => {
	it('links every guessed row of the card, goes to the show, and Home follows', async () => {
		expect(await homeCards()).toEqual([GUESS_TITLE]);
		unmount(app!);
		app = null;

		await linkToTheRightShow();

		expect(linkCalls[0]).toEqual({ rows: [CARD, HIDDEN], kitsu_id: RIGHT });
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the show page');
		expect(String(vi.mocked(goto).mock.calls.at(-1)?.[0])).toBe(`/anime/${RIGHT}`);
		await until(() => !hasNote(), 'the line to go');
		const done = toastStore.items.find((t) => t.actionLabel === m.detail_link_undo());
		expect(done?.message).toBe(m.detail_link_done_toast({ title: TITLE }));

		// A fresh load of Home: one card, as the linked show, and no
		// hidden row surfacing as the old guess.
		expect(await homeCards()).toEqual([TITLE]);
	});

	it('undoes the link and comes back to the page, which offers it again', async () => {
		await linkToTheRightShow();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the show page');
		const done = toastStore.items.find((t) => t.actionLabel === m.detail_link_undo())!;
		done.onAction!();

		await until(() => unlinkCalls.length === 1, 'the undo');
		expect(unlinkCalls[0].kitsu_id).toBe(RIGHT);
		expect(unlinkCalls[0].rows.map((r) => r.row)).toEqual([CARD, HIDDEN]);
		await until(
			() => String(vi.mocked(goto).mock.calls.at(-1)?.[0]).startsWith(`/anime/${GUESS}`),
			'the way back'
		);
		expect(String(vi.mocked(goto).mock.calls.at(-1)?.[0])).toBe(
			`/anime/${GUESS}?guessed=${encodeURIComponent(CARD)}`
		);
		await until(hasNote, 'the line again');
		expect(rows.every((r) => r.kitsu_id === undefined)).toBe(true);
	});
});
