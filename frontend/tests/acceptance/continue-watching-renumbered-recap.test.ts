// Acceptance: a Continue card over a row an earlier version numbered
// otherwise.
//
// A stitched split entry's recap before its second part was numbered
// "24.5" by an earlier version; it is "12.5" now. A history row from
// then reads "24.5" until a listing of the show puts its numbering
// right, and the card's own availability probe pays for that listing.
// The card offers the episode after the one the row reads as once the
// probe has answered — 13, not 25 — and a click resumes from it.

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
import { m } from '../../src/lib/paraglide/messages';
import { savePosition } from '../../src/lib/play/watch-position';

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

const SHOW = 'Stitched Show';
/** Kitsu's episode list, each titled by its number. */
const EPISODES = Array.from({ length: 36 }, (_, i) => ({
	id: `e${i + 1}`,
	canonical_title: `Title ${i + 1}`,
	season_number: 1,
	number: i + 1,
	relative_number: i + 1,
	length: 24,
	synopsis: null,
	airdate: null,
	thumbnail: null
}));
const ROW = 'hianime:stitched-show-4';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	window.localStorage.clear();
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
const offers = (episode: number) =>
	resumeButton()?.textContent?.includes(m.home_resume_episode_label({ episode })) ?? false;

/** The home route over the row; the backend puts its numbering right
 *  when the probe pays for the listing. */
function mountRenumbered(titled = false) {
	let repaired = false;
	server.use(
		...homeHandlers({ history: [{ ep_no: '24.5', id: ROW, title: SHOW }] }, [
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([kitsuRef('1', SHOW, 36)])),
			http.post(`${API_BASE}/api/availability`, () => {
				repaired = true;
				return HttpResponse.json({ available: true, episode_count: 36, approximate: false });
			}),
			http.get(`${API_BASE}/api/history/:id/number`, () =>
				HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
				HttpResponse.json(titled ? EPISODES : [])
			)
		])
	);
	app = mount(HomePage, { target });
}

describe('a Continue card over a renumbered recap', () => {
	it('offers the episode after the one the row reads as once its probe has answered', async () => {
		mountRenumbered();
		await until(() => offers(13), 'the card to offer episode 13');
		expect(offers(25)).toBe(false);
	});

	it('names the episode it offers, not the one it read before', async () => {
		mountRenumbered(true);
		await until(
			() => (resumeButton()?.textContent ?? '').includes('Title 13'),
			"the card to carry episode 13's title"
		);
		expect(resumeButton()?.textContent ?? '').not.toContain('Title 25');
	});

	it('resumes from the episode the row reads as when clicked before its probe has answered', async () => {
		// The card's probe is still out, so the card offers what it read,
		// 25. The click's own cap lookup pays for the listing that puts
		// the row right, and the click goes on from what the row reads as
		// after it.
		let repaired = false;
		server.use(
			...homeHandlers({ history: [{ ep_no: '24.5', id: ROW, title: SHOW }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([kitsuRef('1', SHOW, 36)])
				),
				http.post(`${API_BASE}/api/availability`, async ({ request }) => {
					const body = (await request.json()) as { background?: boolean };
					// The background probe never answers.
					if (body.background !== false) return new Promise<never>(() => {});
					repaired = true;
					return HttpResponse.json({ available: true, episode_count: 36, approximate: false });
				}),
				http.get(`${API_BASE}/api/history/:id/number`, () =>
					HttpResponse.json(repaired ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' })
				)
			])
		);
		app = mount(HomePage, { target });
		await until(() => offers(25), 'the card to offer what it read');
		resumeButton()!.click();
		await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
		const stream = FakeEventSource.instances[0];
		expect(new URL(stream.url).searchParams.get('episode')).toBe('13');
		stream.dispatch(
			'done',
			JSON.stringify({ session_id: 's-1', media_url: 'x', media_kind: 'hls', episode: '13' })
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the play page');
		const url = String(vi.mocked(goto).mock.calls.at(-1)?.[0] ?? '');
		expect(new URL(url, 'http://app.test').searchParams.get('episode')).toBe('13');
	});

	it('offers the recap left part-way under the number it had before', async () => {
		savePosition('1', 24.5, 600, 1420);
		mountRenumbered();
		await until(() => offers(12.5), 'the card to offer the recap left part-way');
		expect(offers(13)).toBe(false);
	});

	it('moves the position under the old number when the probe fails', async () => {
		// Another listing put the stamp right before this load, and the
		// probe fails: the row says what it read as before all the same.
		savePosition('1', 24.5, 600, 1420);
		server.use(
			...homeHandlers({ history: [{ ep_no: '12.5', id: ROW, title: SHOW }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([kitsuRef('1', SHOW, 36)])
				),
				http.post(`${API_BASE}/api/availability`, () => new HttpResponse(null, { status: 503 })),
				http.get(`${API_BASE}/api/history/:id/number`, () =>
					HttpResponse.json({ ep_no: '12.5', former_ep_no: '24.5' })
				)
			])
		);
		app = mount(HomePage, { target });
		await until(() => offers(12.5), 'the card to offer the recap left part-way');
	});
});
