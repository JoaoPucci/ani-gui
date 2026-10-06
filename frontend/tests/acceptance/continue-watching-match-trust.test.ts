// Acceptance: what a Continue card records, and where a card that
// cannot resolve sends the user.
//
// A play from a Continue card records the card's Kitsu id on the
// history row, and the row is matched by that id from then on. A card
// whose match was a title-search guess must not record it — one click
// would pin a wrong guess to the row for good — while a card that read
// the id the row recorded keeps recording it. A recorded id Kitsu no
// longer has (a 404) leaves the row to the matching a row with no id
// gets, and a card that resolves to nothing links to a search for its
// title.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

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

const SHOW = 'Cowboy Bebop';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
	);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
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

const resumeButton = () =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(SHOW)) ?? null;

/** Click the resolved card and return the Kitsu id its play asked to
 *  record, or null when it asked to record none. */
async function clickAndReadRecordedId(): Promise<string | null> {
	await until(() => resumeButton() !== null, 'the resolved Continue card');
	resumeButton()!.click();
	await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
	const stream = FakeEventSource.instances[0];
	const recorded = new URL(stream.url).searchParams.get('kitsu_id');
	// Settle the play as resolved, so the session opens and the next
	// case's click is not folded into this one's pending request.
	stream.dispatch(
		'done',
		JSON.stringify({ id: 's-1', kind: 'mp4', has_subtitles: false, quality: '1080', mode: 'sub' })
	);
	await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the play page');
	return recorded;
}

/** Whether the play page was opened as a session from a guess. */
function openedAsGuess(): boolean {
	const url = String(vi.mocked(goto).mock.calls.at(-1)?.[0] ?? '');
	return new URL(url, 'http://app.test').searchParams.get('guess') === '1';
}

describe('what a Continue card records', () => {
	it('does not record a title-search guess', async () => {
		server.use(
			...homeHandlers({ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([kitsuRef('1', SHOW, 26)])
				)
			])
		);
		app = mount(HomePage, { target });

		expect(await clickAndReadRecordedId()).toBeNull();
		expect(openedAsGuess()).toBe(true);
	});

	it('records the id the row recorded', async () => {
		server.use(
			...homeHandlers(
				{ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW, kitsu_id: '2' }] },
				[
					http.get(`${API_BASE}/api/kitsu/anime/2`, () =>
						HttpResponse.json(kitsuRef('2', SHOW, 26))
					)
				]
			)
		);
		app = mount(HomePage, { target });

		expect(await clickAndReadRecordedId()).toBe('2');
		expect(openedAsGuess()).toBe(false);
	});
});

// The positions a card's session writes are its history row's: the
// guess may be corrected on a later load, and removing the card still
// has to reach what was kept under it.
describe("a Continue card's session", () => {
	it("writes its positions for the card's history row, and carries the row on", async () => {
		window.localStorage.clear();
		server.use(
			...homeHandlers({ history: [{ ep_no: '5', id: 'hianime:cowboy-bebop-1', title: SHOW }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([kitsuRef('1', SHOW, 26)])
				)
			])
		);
		app = mount(HomePage, { target });

		await clickAndReadRecordedId();
		const url = String(vi.mocked(goto).mock.calls.at(-1)?.[0] ?? '');
		expect(new URL(url, 'http://app.test').searchParams.get('row')).toBe('hianime:cowboy-bebop-1');
		const kept = JSON.parse(window.localStorage.getItem('ani-gui.watch-positions') ?? '[]');
		expect(kept).toEqual([['1:6', 0, 'hianime:cowboy-bebop-1']]);
	});
});

describe('a Continue row whose recorded id Kitsu no longer has', () => {
	it('is matched again, and the guess is not recorded', async () => {
		server.use(
			...homeHandlers(
				{ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW, kitsu_id: '999' }] },
				[
					http.get(`${API_BASE}/api/kitsu/anime/999`, () =>
						HttpResponse.json(
							{ kind: 'upstream', status: 404, key: 'error.network.upstream' },
							{ status: 502 }
						)
					),
					http.post(`${API_BASE}/api/kitsu/search`, () =>
						HttpResponse.json([kitsuRef('3', SHOW, 26)])
					)
				]
			)
		);
		app = mount(HomePage, { target });

		expect(await clickAndReadRecordedId()).toBeNull();
		expect(openedAsGuess()).toBe(true);
	});
});

describe('a Continue row that resolves to no show', () => {
	it('links to a search for its title', async () => {
		server.use(
			...homeHandlers({ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW }] })
		);
		app = mount(HomePage, { target });

		const link = () =>
			Array.from(target.querySelectorAll('a')).find((a) => a.textContent?.includes(SHOW)) ?? null;
		await until(() => link() !== null, 'the unresolved row to render as a link');
		const href = new URL(link()!.getAttribute('href')!, 'http://app.test');
		expect(href.pathname).toMatch(/\/search$/);
		expect(href.searchParams.get('q')).toBe(SHOW);
	});
});
