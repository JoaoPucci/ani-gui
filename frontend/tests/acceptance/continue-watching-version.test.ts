// Acceptance: a Continue card plays the version remembered for its
// show.
//
// The choice is made on the show's detail page and kept per Kitsu id;
// a card that opens the show's episode from the home page skips that
// page, so it reads the choice itself and names it on the play and on
// the Continue Watching stamp. With nothing remembered it names none,
// which plays the backend's default, the uncensored copy.

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
import { applyChoice } from '../../src/lib/versions/version-choice';

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

const SHOW = "JoJo's Bizarre Adventure";

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
	);
	FakeEventSource.instances.length = 0;
	window.localStorage.clear();
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

function useRecordedShow(watched: Record<string, unknown>[]) {
	server.use(
		...homeHandlers(
			{ history: [{ ep_no: '3', id: 'hianime:jojo-2', title: SHOW, kitsu_id: '2' }] },
			[
				http.get(`${API_BASE}/api/kitsu/anime/2`, () => HttpResponse.json(kitsuRef('2', SHOW, 26))),
				http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
					watched.push((await request.json()) as Record<string, unknown>);
					return new HttpResponse(null, { status: 204 });
				})
			]
		)
	);
}

/** Click the card, settle its play, and return the play's stream URL. */
async function clickCard(): Promise<URL> {
	await until(() => resumeButton() !== null, 'the resolved Continue card');
	resumeButton()!.click();
	await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
	const stream = FakeEventSource.instances[0];
	stream.dispatch(
		'done',
		JSON.stringify({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4'
		})
	);
	await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the play page');
	return new URL(stream.url);
}

describe('a Continue card and the versions of its show', () => {
	it('plays and stamps the version remembered for the show', async () => {
		applyChoice('2', 'standard');
		const watched: Record<string, unknown>[] = [];
		useRecordedShow(watched);
		app = mount(HomePage, { target });

		expect((await clickCard()).searchParams.get('version')).toBe('standard');
		await until(() => watched.length > 0, 'the Continue Watching stamp');
		expect(watched[0].version).toBe('standard');
	});

	it('names no version when none is remembered', async () => {
		const watched: Record<string, unknown>[] = [];
		useRecordedShow(watched);
		app = mount(HomePage, { target });

		expect((await clickCard()).searchParams.has('version')).toBe(false);
		await until(() => watched.length > 0, 'the Continue Watching stamp');
		expect('version' in watched[0]).toBe(false);
	});
});
