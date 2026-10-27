// Acceptance: a list save held on a 429 keeps its show's editor locked
// across the editor being torn down and built again.
//
// The detail page unmounts its list editor whenever it leaves a show —
// for another route, or for another show — and mounts a new one on the
// way back. A save waiting out AniList's rate limit outlives that: it
// is still going to land. The editor that comes back must show it
// locked rather than a fresh form a second save could be made from;
// another show's editor must stay free; and when the save lands, the
// page on screen — not the one that started it — re-reads the entry.
//
// The unit tests pin the rules. What only the page can show is the
// lifecycle: a real unmount and remount, not a prop switch.

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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import ToastHost from '../../src/lib/components/ToastHost.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { editorFlights } from '../../src/lib/account/editor-flights.svelte';
import type { Flight } from '../../src/lib/account/editor-flight';

const SHOW_A = '50497';
const SHOW_B = '50498';
/** Long enough to leave and come back while the save waits. */
const RETRY_AFTER_S = 2;

let target: HTMLElement;
let stack: HTMLElement;
let detail: ReturnType<typeof mount> | null = null;
let toasts: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
	setParams({ id: SHOW_A });
	accountStore.setConnected('anilist', {
		access_token: 'test-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'user-1',
		username: 'tester',
		avatar_url: null
	});
	target = document.createElement('div');
	stack = document.createElement('div');
	document.body.appendChild(target);
	document.body.appendChild(stack);
	toasts = mount(ToastHost, { target: stack });
});

afterEach(() => {
	vi.restoreAllMocks();
	if (detail) unmount(detail);
	if (toasts) unmount(toasts);
	detail = null;
	toasts = null;
	target.remove();
	stack.remove();
	accountStore.setDisconnected('anilist');
});

async function until(predicate: () => boolean, what: string, timeoutMs = 10_000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(
		`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}\n--- toasts ---\n${stack.textContent}`
	);
}

const trigger = () => target.querySelector('.le-trigger') as HTMLButtonElement | null;
const statusSelect = () => target.querySelector('.le-select') as HTMLSelectElement | null;
const saveButton = () =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.textContent?.trim() === m.detail_list_save()
	) ?? null;

/** What left the app: entry reads per show, and the saves. */
interface Seen {
	reads: Record<string, number>;
	saves: number;
	/** Removals, when a scenario counts them. */
	removals?: number;
}

function useShows(seen: Seen) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json({
				...kitsuRef(String(params.id), `Show ${params.id}`, 12),
				status: 'current'
			})
		),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
			HttpResponse.json({ aired: 3, next_episode: 4, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 3,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/account/entry/anilist`, ({ request }) => {
			const id = new URL(request.url).searchParams.get('kitsu_id') ?? '';
			seen.reads[id] = (seen.reads[id] ?? 0) + 1;
			// The tracker holds the save once it has gone: only a read after it
			// landed can show Dropped.
			if (id === SHOW_A && (seen.removals ?? 0) >= 2) return HttpResponse.json(null);
			const landed = id === SHOW_A && seen.saves >= 2;
			return HttpResponse.json({ status: landed ? 'dropped' : 'watching', progress: 3 });
		}),
		http.delete(`${API_BASE}/api/account/entry/anilist`, () => {
			seen.removals = (seen.removals ?? 0) + 1;
			if (seen.removals === 1) {
				return HttpResponse.json(
					{
						kind: 'rate_limited',
						key: 'error.network.rate_limited',
						retry_after_secs: RETRY_AFTER_S,
						refused_by: 'anilist'
					},
					{ status: 429 }
				);
			}
			return new HttpResponse(null, { status: 204 });
		}),
		http.post(`${API_BASE}/api/account/set/anilist`, () => {
			seen.saves += 1;
			if (seen.saves === 1) {
				return HttpResponse.json(
					{
						kind: 'rate_limited',
						key: 'error.network.rate_limited',
						retry_after_secs: RETRY_AFTER_S,
						refused_by: 'anilist'
					},
					{ status: 429 }
				);
			}
			return HttpResponse.json({
				provider: 'anilist',
				media_id: 7,
				mal_id: 21,
				status: 'dropped',
				progress_episodes: 3,
				score_0_to_100: null,
				updated_at_epoch_s: 0,
				title: 'x'
			});
		})
	);
}

/** Open the editor on the show on screen, pick Dropped, save, and wait
 *  for the notice that AniList holds it back. */
async function startHeldSave() {
	await until(() => trigger() !== null && !trigger()!.disabled, 'the list control');
	trigger()!.click();
	await until(() => statusSelect() !== null, 'the status control');
	const select = statusSelect()!;
	select.value = 'dropped';
	select.dispatchEvent(new Event('change', { bubbles: true }));
	await until(() => saveButton() !== null, 'the Save control');
	saveButton()!.click();
	const busy = m.account_busy_saving({
		provider: m.account_provider_anilist(),
		seconds: RETRY_AFTER_S
	});
	await until(() => (stack.textContent ?? '').includes(busy), `the notice "${busy}"`);
}

/** Leave the show and come back with the save still held: a new page,
 *  a new editor. */
async function remount() {
	unmount(detail!);
	detail = null;
	target.textContent = '';
	detail = mount(DetailPage, { target });
	await until(() => trigger() !== null && !trigger()!.disabled, 'the list control again');
}

/** AniList's account starts being changed — a disconnect in progress:
 *  the tracker is still listed as connected, so the page does not
 *  re-read, and the held write is no longer this account's. The store
 *  does not announce the change flag on its own, so the editor sees the
 *  lock lift at its next look at the flights, which another show's
 *  write settling gives it here. */
function beginAniListChange() {
	accountStore.beginAccountChange('anilist');
	const elsewhere: Flight = { kind: 'saving' };
	editorFlights.begin('elsewhere', elsewhere);
	editorFlights.end('elsewhere', elsewhere);
}

/** The held write settles at the end of its wait, superseded unsent, and
 *  the editor has the page re-read. */
async function untilTheHeldWriteSettles(seen: Seen, readsBefore: number) {
	await until(() => (seen.reads[SHOW_A] ?? 0) > readsBefore, 'the re-read once the write settled');
	expect(seen.saves).toBe(1);
}

const removeButton = () => target.querySelector('.le-remove') as HTMLButtonElement | null;

/** Pick `status` in the open popover. */
function pick(status: string) {
	const select = statusSelect()!;
	select.value = status;
	select.dispatchEvent(new Event('change', { bubbles: true }));
}

/** The editor on screen, opened: locked on the save still out. */
async function expectLockedOnTheHeldSave(seen: Seen) {
	await until(() => trigger() !== null && !trigger()!.disabled, 'the list control again');
	trigger()!.click();
	await until(() => statusSelect() !== null, 'the popover');
	expect(seen.saves).toBe(1);
	expect(statusSelect()!.disabled).toBe(true);
	expect(statusSelect()!.value).toBe('dropped');
	expect(target.textContent ?? '').toContain(m.detail_list_saving());
}

/** The save lands; the page on screen re-reads the show's entry and
 *  shows what the tracker now holds, and the popover showing the sent
 *  form closes. */
async function expectLandedAndReconciled(seen: Seen) {
	await until(() => (stack.textContent ?? '').includes(m.detail_list_saved()), 'the saved toast');
	expect(seen.saves).toBe(2);
	await until(
		() => (trigger()?.textContent ?? '').includes(m.detail_list_status_dropped()),
		'the re-read entry on the list control'
	);
	expect(statusSelect()).toBeNull();
}

describe('detail route — a held list save across an editor remount', () => {
	it('keeps the show locked after leaving for another route and coming back', async () => {
		const seen: Seen = { reads: {}, saves: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await startHeldSave();

		// Another route: the detail page goes, its editor with it.
		unmount(detail);
		detail = null;
		target.textContent = '';

		// Back to the same show: a new page, a new editor.
		detail = mount(DetailPage, { target });
		await expectLockedOnTheHeldSave(seen);
		await expectLandedAndReconciled(seen);
	});

	it('keeps A locked and B free going A → B → A while A waits', async () => {
		const seen: Seen = { reads: {}, saves: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await startHeldSave();

		// Show B: the page drops A's detail, and the editor with it.
		setParams({ id: SHOW_B });
		await until(() => (seen.reads[SHOW_B] ?? 0) > 0, "B's entry read");
		await until(() => trigger() !== null && !trigger()!.disabled, "B's list control");
		trigger()!.click();
		await until(() => statusSelect() !== null, "B's popover");
		expect(statusSelect()!.disabled).toBe(false);
		expect(saveButton()).not.toBeNull();

		// Back to A, before its save lands.
		setParams({ id: SHOW_A });
		await until(() => (seen.reads[SHOW_A] ?? 0) > 1, "A's entry read again");
		await expectLockedOnTheHeldSave(seen);
		await expectLandedAndReconciled(seen);
	});

	it('does not re-read when an account switch lifts the lock, only when the write settles', async () => {
		const seen: Seen = { reads: {}, saves: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await startHeldSave();
		unmount(detail);
		detail = null;
		target.textContent = '';
		detail = mount(DetailPage, { target });
		await until(() => trigger() !== null && !trigger()!.disabled, 'the list control again');
		const readsBefore = seen.reads[SHOW_A] ?? 0;
		// The reads counted at the instant the held write's flight ends, read
		// where the store is told — before anything can act on the ending.
		let readsWhenTheFlightEnded: number | null = null;
		const end = editorFlights.end.bind(editorFlights);
		vi.spyOn(editorFlights, 'end').mockImplementation((id, flight) => {
			if (id === SHOW_A) readsWhenTheFlightEnded = seen.reads[SHOW_A] ?? 0;
			end(id, flight);
		});

		// AniList is now another user's: the held save is no longer this
		// account's to send, and the lock lifts — but nothing has landed.
		accountStore.setConnected('anilist', {
			access_token: 'other-bearer',
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-2',
			username: 'someone else',
			avatar_url: null
		});
		// The page reads the entry once for the new account, as on any account
		// change: the positive witness that the switch has been taken in.
		await until(() => (seen.reads[SHOW_A] ?? 0) >= readsBefore + 1, "the new account's read");

		// The held write settles at the end of its wait, superseded unsent.
		// Its flight ending is the witness that the lifted lock has had all
		// the time it will get: the reads counted at that moment, before the
		// editor can act on the ending, are the account's read and nothing
		// more — a re-read on the lock lifting would be among them.
		await until(() => readsWhenTheFlightEnded !== null, 'the held write to settle');
		expect(readsWhenTheFlightEnded).toBe(readsBefore + 1);

		// Then the editor has the page re-read.
		await until(
			() => (seen.reads[SHOW_A] ?? 0) > readsBefore + 1,
			'the re-read once the write settled'
		);
		expect(seen.saves).toBe(1);
	});

	it('leaves a form opened fresh after the lock lifted open when the held write settles', async () => {
		const seen: Seen = { reads: {}, saves: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await startHeldSave();
		await remount();
		const readsBefore = seen.reads[SHOW_A] ?? 0;

		beginAniListChange();
		trigger()!.click();
		await until(() => statusSelect() !== null && !statusSelect()!.disabled, 'a fresh form');
		pick('paused');

		await untilTheHeldWriteSettles(seen, readsBefore);
		expect(statusSelect()).not.toBeNull();
		expect(statusSelect()!.value).toBe('paused');
	});

	it('leaves the sent form open once the user edits it after the lock lifted', async () => {
		const seen: Seen = { reads: {}, saves: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await startHeldSave();
		await remount();
		const readsBefore = seen.reads[SHOW_A] ?? 0;
		trigger()!.click();
		await until(() => statusSelect() !== null, 'the sent form');
		expect(statusSelect()!.disabled).toBe(true);

		beginAniListChange();
		await until(() => !statusSelect()!.disabled, 'the lock to lift');
		pick('paused');

		await untilTheHeldWriteSettles(seen, readsBefore);
		expect(statusSelect()).not.toBeNull();
		expect(statusSelect()!.value).toBe('paused');
	});

	it('closes a form reopened on a held removal when the removal lands', async () => {
		const seen: Seen = { reads: {}, saves: 0, removals: 0 };
		useShows(seen);
		detail = mount(DetailPage, { target });
		await until(() => trigger() !== null && !trigger()!.disabled, 'the list control');
		trigger()!.click();
		await until(() => removeButton() !== null, 'the Remove control');
		removeButton()!.click();
		const busy = m.account_busy_saving({
			provider: m.account_provider_anilist(),
			seconds: RETRY_AFTER_S
		});
		await until(() => (stack.textContent ?? '').includes(busy), `the notice "${busy}"`);
		await remount();

		trigger()!.click();
		await until(() => statusSelect() !== null, 'the popover on the held removal');
		expect(target.textContent ?? '').toContain(m.detail_list_removing());

		await until(
			() => (stack.textContent ?? '').includes(m.detail_list_removed()),
			'the removed toast'
		);
		await until(() => statusSelect() === null, 'the popover to close');
		expect(seen.removals).toBe(2);
	});
});
