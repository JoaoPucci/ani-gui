// Acceptance: the app's other dialogs hold the keyboard while they are up.
//
// The update dialog (opened from the topbar's update badge, on every
// page) and the download dialog (opened from a show's Download action)
// covered the page for the pointer only. Focus stayed on the control
// that opened them, so Tab walked the page behind, and the download
// dialog answered Escape only once something inside it had been
// clicked.
//
// Each is a modal now, as Home's are: it takes focus as it opens, Tab
// and Shift+Tab stay inside it, the page behind it is inert, Escape
// closes it, and focus goes back to the control that opened it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
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
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
}));

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import UpdateBadge from '../../src/lib/components/UpdateBadge.svelte';
import UpdateDialog from '../../src/lib/components/UpdateDialog.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { updateStore } from '../../src/lib/update/store.svelte';

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

const KITSU_ID = '7154';
const TITLE = "JoJo's Bizarre Adventure";
const EPISODES = 26;

let target: HTMLElement;
let apps: ReturnType<typeof mount>[] = [];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	for (const app of apps) unmount(app);
	apps = [];
	updateStore.closeDialog();
	updateStore.setAvailable(null);
	delete g.EventSource;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${document.body.textContent}`);
}

const inert = (el: Element | null) => el !== null && el.closest('[inert]') !== null;
const active = () => document.activeElement;
function press(key: string, shiftKey = false): boolean {
	const at = (document.activeElement ?? document.body) as HTMLElement;
	const ev = new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true });
	at.dispatchEvent(ev);
	return ev.defaultPrevented;
}

describe('the update dialog', () => {
	const badge = () =>
		target.querySelector<HTMLButtonElement>(`button[aria-label="${m.update_badge_aria_label()}"]`);
	const dialog = () =>
		target.querySelector<HTMLElement>('[role="dialog"][aria-labelledby="update-dialog-title"]');

	async function openFromBadge() {
		updateStore.setAvailable({
			tag: 'v9.9.9',
			name: 'v9.9.9',
			url: 'https://example.invalid/releases/v9.9.9',
			publishedAt: '2026-11-01T00:00:00Z',
			body: 'Notes.'
		});
		apps.push(mount(UpdateBadge, { target }));
		apps.push(mount(UpdateDialog, { target, props: { currentVersion: '0.17.0' } }));
		await until(() => badge() !== null, 'the update badge');
		const opener = badge()!;
		opener.focus();
		opener.click();
		await until(() => dialog() !== null, 'the update dialog');
		return opener;
	}

	it('takes focus, keeps Tab inside it and leaves the badge out of reach', async () => {
		const opener = await openFromBadge();
		expect(dialog()!.contains(active())).toBe(true);
		for (const shift of [false, false, true, true, true]) {
			expect(press('Tab', shift)).toBe(true);
			expect(dialog()!.contains(active())).toBe(true);
		}
		expect(inert(opener)).toBe(true);
	});

	it('closes on Escape and gives focus back to the badge', async () => {
		const opener = await openFromBadge();
		expect(press('Escape')).toBe(true);
		await until(() => dialog() === null, 'the dialog to close');
		expect(inert(opener)).toBe(false);
		await until(() => active() === opener, 'focus back on the badge');
	});
});

describe('the download dialog on a show page', () => {
	function useShowHandlers() {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({
					aired: EPISODES,
					next_episode: null,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: EPISODES,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
		);
	}
	const downloadAction = () =>
		(Array.from(target.querySelectorAll('button')).find((b) =>
			(b.textContent ?? '').includes(m.detail_download_button())
		) as HTMLButtonElement | undefined) ?? null;
	const dialog = () =>
		target.querySelector<HTMLElement>('[role="dialog"][aria-labelledby="dl-confirm-title"]');

	async function openFromDownload() {
		setParams({ id: KITSU_ID });
		useShowHandlers();
		apps.push(mount(DetailPage, { target }));
		await until(() => downloadAction() !== null && !downloadAction()!.disabled, 'Download');
		const opener = downloadAction()!;
		opener.focus();
		opener.click();
		await until(() => dialog() !== null, 'the download dialog');
		return opener;
	}

	it('takes focus, keeps Tab inside it and leaves the page out of reach', async () => {
		const opener = await openFromDownload();
		expect(dialog()!.contains(active())).toBe(true);
		for (const shift of [false, true, true, false, false]) {
			expect(press('Tab', shift)).toBe(true);
			expect(dialog()!.contains(active())).toBe(true);
		}
		expect(inert(opener)).toBe(true);
	});

	it('closes on Escape and gives focus back to Download', async () => {
		const opener = await openFromDownload();
		expect(press('Escape')).toBe(true);
		await until(() => dialog() === null, 'the dialog to close');
		expect(inert(opener)).toBe(false);
		await until(() => active() === opener, 'focus back on Download');
	});
});
