// Acceptance: a Continue click opens no player once the viewer moved on.
//
// A Continue click awaits its settings, its cap and its session, and
// then opens the player. The viewer can move on meanwhile — press
// Back, follow another link. A click that navigated anyway pulled
// them into the player from wherever they had gone, or over the
// navigation they had just started.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, tick, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { forgetNavigationListeners, startNavigation, startQueryNavigation } from './navigation';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
// Every Continue click runs through the controller `makeStartResume`
// builds, and the click's promise settles only once the click has run
// to its end: navigated, shown its failure, or let go of the card.
// Recording each click's promise lets a case wait for the click to be
// over rather than for a fixed time.
const clicks = vi.hoisted(() => [] as Promise<void>[]);
vi.mock('$lib/history/start-resume', async (importOriginal) => {
	const real = await importOriginal<typeof import('$lib/history/start-resume')>();
	const makeStartResume: typeof real.makeStartResume = (deps) => {
		const start = real.makeStartResume(deps);
		return (...args) => {
			const click = start(...args);
			clicks.push(click);
			return click;
		};
	};
	return { ...real, makeStartResume };
});
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
import { m } from '../../src/lib/paraglide/messages';

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
let numberHeld: Promise<void> | null = null;
let numberAsked = false;
let batchHeld: Promise<void> | null = null;
let batchAsked = false;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	clicks.length = 0;
	forgetNavigationListeners();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	numberHeld = null;
	batchHeld = null;
	batchAsked = false;
	server.use(
		...homeHandlers({ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW }] }, [
			// The cache the failure message is worded from: the other
			// audio's answer for the show.
			http.post(`${API_BASE}/api/availability/batch`, async () => {
				batchAsked = true;
				if (batchHeld) await batchHeld;
				return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			}),
			// A row read again waits while a scenario holds it: the click
			// reads its row before it asks for a session.
			http.get(`${API_BASE}/api/history/:id/number`, async () => {
				if (numberHeld) {
					numberAsked = true;
					await numberHeld;
				}
				return HttpResponse.json({ ep_no: '3' });
			}),
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

/** The last Continue click has run to its end, and what it rendered
 *  is flushed. A click that went on to ask for a session it is never
 *  handed does not end, so this times out rather than passing. */
async function clickOver() {
	await until(() => clicks.length > 0, 'a Continue click');
	let over = false;
	void Promise.allSettled([clicks[clicks.length - 1]]).then(() => (over = true));
	await until(() => over, 'the Continue click to run to its end');
	await tick();
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
		await clickOver();

		expect(goto).not.toHaveBeenCalled();
	});

	it('opens no player over a navigation the viewer started meanwhile', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		startNavigation();
		stream.dispatch('done', SESSION);
		await clickOver();

		expect(goto).not.toHaveBeenCalled();
		// Home is still on screen until that navigation lands; the card
		// it let go of takes a click again.
		expect(overlay()).toBeNull();
	});

	it('opens the player through a navigation that stays on home', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		startQueryNavigation();
		stream.dispatch('done', SESSION);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the player');
	});

	it('opens the player while the viewer stays', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();

		stream.dispatch('done', SESSION);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the player');
	});
});

// The click reads its row again before it asks for a session. A viewer
// who moved on during that read has abandoned the click before any
// play began: none is resolved, no watch is recorded or mirrored to
// their trackers, and no episode is marked started.
describe('a Continue click the viewer moved on from before its session was asked for', () => {
	async function clickAndHoldTheRow(): Promise<() => void> {
		let release!: () => void;
		numberHeld = new Promise<void>((r) => (release = r));
		numberAsked = false;
		await until(() => resumeButton() !== null, 'the resolved Continue card');
		resumeButton()!.click();
		await until(() => numberAsked, 'the click to read its row again');
		return release;
	}

	it('asks for no session once the viewer has left home', async () => {
		app = mount(HomePage, { target });
		const release = await clickAndHoldTheRow();

		unmount(app);
		app = null;
		release();
		await clickOver();

		expect(FakeEventSource.instances).toEqual([]);
		expect(window.localStorage.getItem('ani-gui.watch-positions')).toBeNull();
		expect(goto).not.toHaveBeenCalled();
	});

	it('asks for no session once a navigation started, and lets the card go', async () => {
		app = mount(HomePage, { target });
		const release = await clickAndHoldTheRow();

		startNavigation();
		release();
		await clickOver();

		expect(FakeEventSource.instances).toEqual([]);
		expect(goto).not.toHaveBeenCalled();
		expect(overlay()).toBeNull();
	});
});

// A failed resume is worded after a cache lookup, and the viewer can
// move on while it is out. The message is about a click they left
// behind, and showing it would cover the page they are going to.
describe('a Continue click that fails while the viewer moves on', () => {
	it('shows no failure once a navigation started while it was worded', async () => {
		let release!: () => void;
		batchHeld = new Promise<void>((r) => (release = r));
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();
		stream.dispatch(
			'error',
			JSON.stringify({ kind: 'episode_unavailable', key: 'error.play.episode_unavailable' })
		);
		await until(() => batchAsked, 'the failure to be worded from the cache');

		startNavigation();
		release();
		await clickOver();

		expect(target.textContent).not.toContain(m.play_play_failure_episode_unavailable());
		expect(overlay()).toBeNull();
	});

	it('shows the failure while the viewer stays', async () => {
		app = mount(HomePage, { target });
		const stream = await clickAndHoldTheSession();
		stream.dispatch(
			'error',
			JSON.stringify({ kind: 'episode_unavailable', key: 'error.play.episode_unavailable' })
		);
		await until(
			() => (target.textContent ?? '').includes(m.play_play_failure_episode_unavailable()),
			'the failure message'
		);
	});
});
