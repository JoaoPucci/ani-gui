// Acceptance: a Continue click opens no player once the viewer moved on.
//
// A Continue click awaits its settings, its cap and its session, and
// then opens the player. The viewer can move on meanwhile — press
// Back, follow another link. A click that navigated anyway pulled
// them into the player from wherever they had gone, or over the
// navigation they had just started.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { forgetNavigationListeners, startNavigation } from './navigation';

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
	forgetNavigationListeners();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	server.use(
		...homeHandlers({ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW }] }, [
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([kitsuRef('1', SHOW, 26)])),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
		])
	);
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

/** The blocking overlay the page raises while a click works. */
const overlay = () => target.querySelector('[role="status"].backdrop');

/** Click the card and wait until only its session stands between it
 *  and the player. */
async function clickAndHoldTheSession(): Promise<FakeEventSource> {
	await until(() => resumeButton() !== null, 'the resolved Continue card');
	resumeButton()!.click();
	await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
	return FakeEventSource.instances[0];
}

const SESSION = JSON.stringify({
	id: 's-1',
	kind: 'mp4',
	has_subtitles: false,
	quality: '1080',
	mode: 'sub'
});

describe('a Continue click whose session lands after the viewer moved on', () => {
	it('opens no player once the viewer has left home', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		unmount(app);
		app = null;
		stream.dispatch('done', SESSION);
		await new Promise((r) => setTimeout(r, 100));

		expect(goto).not.toHaveBeenCalled();
	});

	it('opens no player over a navigation the viewer started meanwhile', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		startNavigation();
		stream.dispatch('done', SESSION);
		await new Promise((r) => setTimeout(r, 100));

		expect(goto).not.toHaveBeenCalled();
		// Home is still on screen until that navigation lands; the card
		// it let go of takes a click again.
		expect(overlay()).toBeNull();
	});

	it('opens the player while the viewer stays', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		stream.dispatch('done', SESSION);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the player');
	});
});
