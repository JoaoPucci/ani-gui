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
	__resetApiBaseForTests(API_BASE);
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
	// Settle the play, so the next case's click is not folded into this
	// one's still-pending request.
	stream.dispatch('error', JSON.stringify({ kind: 'network', key: 'error.network.unreachable' }));
	return new URL(stream.url).searchParams.get('kitsu_id');
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
	});

	it('records the id the row recorded', async () => {
		server.use(
			...homeHandlers(
				{ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW, kitsu_id: '1' }] },
				[
					http.get(`${API_BASE}/api/kitsu/anime/1`, () =>
						HttpResponse.json(kitsuRef('1', SHOW, 26))
					)
				]
			)
		);
		app = mount(HomePage, { target });

		expect(await clickAndReadRecordedId()).toBe('1');
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
						HttpResponse.json([kitsuRef('1', SHOW, 26)])
					)
				]
			)
		);
		app = mount(HomePage, { target });

		expect(await clickAndReadRecordedId()).toBeNull();
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
