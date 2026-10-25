// Acceptance: a list save AniList holds back is saved when the wait ends.
//
// The backend keeps AniList's request budget. A write that would spend
// past it, or that meets AniList's cool-down after a 429, is answered
// at once with HTTP 429 and the seconds until it may go — and nothing is
// sent to AniList. A save the user made is not dropped on that answer:
// the editor says AniList is busy and when it will save, waits, and
// sends the save again by itself.
//
// The unit tests pin the hold and the fan-out's use of it. What they
// cannot see is the page: that the notice reaches the toast stack the
// user is looking at, that the wait is a real timer, and that the save
// which lands afterwards ends the edit as a clean save does. So the
// assertions here are about requests that left the app and text that
// reached the DOM.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

import ListEntryEditor from '../../src/lib/components/ListEntryEditor.svelte';
import ToastHost from '../../src/lib/components/ToastHost.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import { editorProps, showEditorFor } from './editor-props.svelte';

const KITSU_ID = '42';
/** The wait the backend relays. Short, so the scenario sees the save
 *  that follows it. */
const RETRY_AFTER_S = 1;

let target: HTMLElement;
/** Where the toast stack renders, beside the editor as the layout puts it. */
let stack: HTMLElement;
let editor: ReturnType<typeof mount> | null = null;
let toasts: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
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
});

afterEach(() => {
	if (editor) unmount(editor);
	if (toasts) unmount(toasts);
	editor = null;
	toasts = null;
	target.remove();
	stack.remove();
	accountStore.setDisconnected('anilist');
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(
		`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}\n--- toasts ---\n${stack.textContent}`
	);
}

const trigger = () => target.querySelector('button') as HTMLButtonElement;
const statusSelect = () => target.querySelector('select') as HTMLSelectElement | null;
const closeButton = () =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.getAttribute('aria-label') === m.detail_list_close()
	) ?? null;
const saveButton = () =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.textContent?.trim() === m.detail_list_save()
	) ?? null;

/** AniList's list as the backend relays it: the row, and a save that is
 *  held back once with the wait before it is taken. */
/** Each save the backend received: when, and the body the editor sent. */
interface Saves {
	at: number[];
	bodies: unknown[];
}

function useTracker(saves: Saves) {
	server.use(
		http.get(`${API_BASE}/api/account/entry/anilist`, () =>
			HttpResponse.json({ status: 'watching', progress: 3 })
		),
		http.post(`${API_BASE}/api/account/set/anilist`, async ({ request }) => {
			saves.at.push(Date.now());
			saves.bodies.push(await request.json());
			if (saves.at.length === 1) {
				return HttpResponse.json(
					{
						kind: 'rate_limited',
						key: 'error.network.rate_limited',
						retry_after_secs: RETRY_AFTER_S
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

describe('list editor — AniList holds the save back', () => {
	it('says AniList is busy, waits the advertised time, and saves by itself', async () => {
		const saves: Saves = { at: [], bodies: [] };
		useTracker(saves);
		toasts = mount(ToastHost, { target: stack });
		editor = mount(ListEntryEditor, {
			target,
			props: { kitsuId: KITSU_ID, total: 12, current: { status: 'watching', progress: 3 } }
		});

		trigger().click();
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
		expect(saves.at).toHaveLength(1);

		// No click: the save goes again once the wait has passed, and lands.
		await until(() => (stack.textContent ?? '').includes(m.detail_list_saved()), 'the saved toast');
		expect(saves.at).toHaveLength(2);
		expect(saves.at[1] - saves.at[0]).toBeGreaterThanOrEqual(RETRY_AFTER_S * 1000 - 50);
		// Both carry the user's edit: the one held back, and the one that landed.
		for (const body of saves.bodies) {
			expect(body).toMatchObject({ kitsu_id: KITSU_ID, status: 'dropped' });
		}
		// Nothing says the save failed.
		expect(stack.textContent ?? '').not.toContain(m.detail_list_rate_limited());
		expect(stack.textContent ?? '').not.toContain(m.detail_list_save_failed());
	});

	it('can be dismissed while the save waits, and still reports the save', async () => {
		const saves: Saves = { at: [], bodies: [] };
		useTracker(saves);
		toasts = mount(ToastHost, { target: stack });
		editor = mount(ListEntryEditor, {
			target,
			props: { kitsuId: KITSU_ID, total: 12, current: { status: 'watching', progress: 3 } }
		});

		trigger().click();
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

		// The user closes the popover rather than watch it wait.
		const close = Array.from(target.querySelectorAll('button')).find(
			(b) => b.getAttribute('aria-label') === m.detail_list_close()
		);
		close!.click();
		await until(() => statusSelect() === null, 'the popover to close');
		// Closed while the save still waits, not by its landing.
		expect(saves.at).toHaveLength(1);

		// The save carries on and says how it ended.
		await until(() => (stack.textContent ?? '').includes(m.detail_list_saved()), 'the saved toast');
		expect(saves.at).toHaveLength(2);
	});

	it('reopens on the save still waiting, locked, rather than a fresh form', async () => {
		const saves: Saves = { at: [], bodies: [] };
		useTracker(saves);
		toasts = mount(ToastHost, { target: stack });
		editor = mount(ListEntryEditor, {
			target,
			props: { kitsuId: KITSU_ID, total: 12, current: { status: 'watching', progress: 3 } }
		});

		trigger().click();
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
		closeButton()!.click();
		await until(() => statusSelect() === null, 'the popover to close');

		// The user looks again before the save has landed.
		trigger().click();
		await until(() => statusSelect() !== null, 'the popover to reopen');
		expect(saves.at).toHaveLength(1);
		expect(statusSelect()!.disabled).toBe(true);
		expect(statusSelect()!.value).toBe('dropped');
		expect(target.textContent ?? '').toContain(m.detail_list_saving());

		await until(() => (stack.textContent ?? '').includes(m.detail_list_saved()), 'the saved toast');
	});
});

describe('list editor — a save still waiting on one show', () => {
	it('does not lock the editor once it is on another show', async () => {
		const saves: Saves = { at: [], bodies: [] };
		useTracker(saves);
		server.use(
			http.get(`${API_BASE}/api/account/entry/anilist`, () =>
				HttpResponse.json({ status: 'watching', progress: 3 })
			)
		);
		toasts = mount(ToastHost, { target: stack });
		showEditorFor(KITSU_ID);
		editor = mount(ListEntryEditor, { target, props: editorProps });

		trigger().click();
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

		// The detail route moves the same editor to another show.
		showEditorFor('43');
		await until(() => statusSelect() === null, "the first show's popover to close");
		trigger().click();
		await until(() => statusSelect() !== null, "the second show's editor to open");
		expect(statusSelect()!.disabled).toBe(false);
		expect(saveButton()).not.toBeNull();

		// The first show's save still lands, and says so.
		await until(() => (stack.textContent ?? '').includes(m.detail_list_saved()), 'the saved toast');
	});
});
