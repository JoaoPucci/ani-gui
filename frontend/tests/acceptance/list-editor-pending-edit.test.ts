// Acceptance: a status the user asked for outlives the popover.
//
// When a save lands on some trackers and not others, the editor keeps
// the intent so a retry re-sends it to the laggards. The rules for
// that are unit-tested; what is not is the thing the rules depend on —
// that the intent is held somewhere the popover closing does not take
// with it.
//
// That is a placement fact, not a logic one. `pendingEdit` living
// inside the `{#if open}` block instead of beside it would pass every
// unit case and still lose the edit the moment the user dismissed, so
// it can only be observed by dismissing and reopening for real.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

// The save fans out to every connected tracker. Held here so the
// scenario can answer "one took it, one did not" without standing up
// two provider stubs.
const partial = vi.hoisted(() => ({
	calls: [] as { status: string; statusChanged: boolean }[],
	/** Answers for the next saves, in order; a partial once they run out. */
	answers: [] as { written: number; failed: number }[]
}));
vi.mock('$lib/account/set-entry', () => ({
	syncSetEntry: vi.fn(
		async (_kitsuId: string, save: { status: string; statusChanged: boolean }) => {
			// `statusChanged` is the observable that matters: it is what
			// tells the fan-out to send the status at all, so it is what a
			// lagging tracker needs on the retry.
			partial.calls.push({ status: save.status, statusChanged: save.statusChanged });
			// One tracker took it, one did not — which is what makes this a
			// partial rather than a save or a failure.
			return partial.answers.shift() ?? { written: 1, failed: 1 };
		}
	),
	syncRemoveEntry: vi.fn()
}));

import ListEntryEditor from '../../src/lib/components/ListEntryEditor.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { accountStore } from '../../src/lib/account/store.svelte';
import { syncRemoveEntry } from '../../src/lib/account/set-entry';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	partial.calls = [];
	partial.answers = [];
	server.use();
	// The two trackers the partial save went to. The fan-out is mocked, but
	// the editor still reads which accounts the save was made under.
	for (const [provider, user] of [
		['anilist', 'user-a'],
		['mal', 'user-m']
	] as const) {
		accountStore.setConnected(provider, {
			access_token: `${provider}-bearer`,
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: user,
			username: 'tester',
			avatar_url: null
		});
	}
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	accountStore.setDisconnected('anilist');
	accountStore.setDisconnected('mal');
	accountStore.setDisconnected('inhouse');
});

/** Connect `provider` as `user`. */
function connect(provider: 'anilist' | 'mal' | 'inhouse', user: string) {
	accountStore.setConnected(provider, {
		access_token: `${provider}-${user}`,
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: user,
		username: 'tester',
		avatar_url: null
	});
}

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

const trigger = () => target.querySelector('button') as HTMLButtonElement;
const statusSelect = () => target.querySelector('select') as HTMLSelectElement | null;

// Match the label the component actually renders, not an English
// spelling of it. The popover also holds Remove, ✕ and the two
// steppers, so this is an exact match rather than a substring — in a
// locale where one label contains the other, `includes` would pick
// whichever came first in the DOM.
const removeButton = () => target.querySelector('.le-remove') as HTMLButtonElement | null;
const saveButton = () =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.textContent?.trim() === m.detail_list_save()
	) ?? null;

describe('list editor after a partial save', () => {
	it('reopens on the status the user asked for, not the one that landed', async () => {
		app = mount(ListEntryEditor, {
			target,
			props: {
				kitsuId: '42',
				total: 12,
				current: { status: 'watching', progress: 3 }
			}
		});
		await settle();

		// Open, pick a different status, save. One tracker takes it, one
		// does not — so the live value stays where it was.
		trigger().click();
		await settle();
		const select = statusSelect();
		expect(select, 'the status control should be offered').not.toBeNull();
		select!.value = 'dropped';
		select!.dispatchEvent(new Event('change', { bubbles: true }));
		await settle();

		const save = saveButton();
		expect(save, 'the Save control should be present').not.toBeNull();
		save!.click();
		await settle();
		expect(partial.calls).toHaveLength(1);
		expect(partial.calls[0].status).toBe('dropped');

		// The user gives up on the retry for now and dismisses. The
		// component stays mounted — only the popover goes.
		trigger().click();
		await settle();

		// Reopening has to offer the retry, which means seeding from
		// what was asked for. Seeding from what landed would make a
		// plain Save read as "no change" and strand the tracker that
		// failed.
		trigger().click();
		await settle();

		// The form showing 'dropped' proves nothing on its own: the
		// partial branch already moved `live` to the requested status,
		// so a reopen with NO surviving intent seeds 'dropped' too.
		// What separates them is the retry.
		expect(statusSelect()?.value).toBe('dropped');

		// Save again without touching anything. The pick now equals the
		// seed, so without a surviving intent this reads as "no status
		// change" and the fan-out skips the status — leaving the
		// tracker that failed still on the old one, forever. The
		// intent is what forces it to be sent anyway.
		const retry = saveButton();
		expect(retry, 'the Save control should be offered again on reopen').not.toBeNull();
		retry!.click();
		await settle();

		expect(partial.calls).toHaveLength(2);
		expect(partial.calls[1].status).toBe('dropped');
		expect(partial.calls[1].statusChanged).toBe(true);
	});

	it("does not carry a retry made under one account into another account's editor", async () => {
		app = mount(ListEntryEditor, {
			target,
			props: {
				kitsuId: '42',
				total: 12,
				current: { status: 'watching', progress: 3 }
			}
		});
		await settle();
		trigger().click();
		await settle();
		const select = statusSelect()!;
		select.value = 'dropped';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await settle();
		saveButton()!.click();
		await settle();
		trigger().click();
		await settle();

		// Both trackers are now other users' accounts.
		for (const [provider, user] of [
			['anilist', 'user-a2'],
			['mal', 'user-m2']
		] as const) {
			accountStore.setConnected(provider, {
				access_token: `${provider}-other`,
				refresh_token: null,
				expires_at_epoch_s: 4_000_000_000,
				user_id: user,
				username: 'someone else',
				avatar_url: null
			});
		}

		// Reopened, the form seeds from the live view, and a plain Save is not
		// a status write: the retry was the other accounts'.
		trigger().click();
		await settle();
		expect(statusSelect()?.value).toBe('dropped');
		saveButton()!.click();
		await settle();
		expect(partial.calls).toHaveLength(2);
		expect(partial.calls[1].statusChanged).toBe(false);
	});

	it("keeps one account's retry through another account's save, and offers it again when that account is back", async () => {
		const connectAs = (anilist: string, mal: string) => {
			for (const [provider, user] of [
				['anilist', anilist],
				['mal', mal]
			] as const) {
				accountStore.setConnected(provider, {
					access_token: `${provider}-${user}`,
					refresh_token: null,
					expires_at_epoch_s: 4_000_000_000,
					user_id: user,
					username: 'tester',
					avatar_url: null
				});
			}
		};
		app = mount(ListEntryEditor, {
			target,
			props: { kitsuId: '43', total: 12, current: { status: 'watching', progress: 3 } }
		});
		await settle();

		// A partial save under the first accounts leaves a retry for Dropped.
		trigger().click();
		await settle();
		const select = statusSelect()!;
		select.value = 'dropped';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await settle();
		saveButton()!.click();
		await settle();
		trigger().click();
		await settle();

		// Other accounts save the show cleanly: their save has no retry of its
		// own to clear, and the first accounts' is not theirs to clear.
		connectAs('user-a2', 'user-m2');
		partial.answers.push({ written: 2, failed: 0 });
		trigger().click();
		await settle();
		saveButton()!.click();
		await settle();
		expect(partial.calls).toHaveLength(2);

		// The first accounts are back: their retry is offered again.
		connectAs('user-a', 'user-m');
		trigger().click();
		await settle();
		saveButton()!.click();
		await settle();
		expect(partial.calls).toHaveLength(3);
		expect(partial.calls[2]).toEqual({ status: 'dropped', statusChanged: true });
	});

	describe('a retry and a later write under a different set of trackers', () => {
		/** A partial status save under AniList and MyAnimeList, leaving a
		 *  retry for Dropped, on show `kitsuId`. */
		async function partialRetry(kitsuId: string) {
			app = mount(ListEntryEditor, {
				target,
				props: { kitsuId, total: 12, current: { status: 'watching', progress: 3 } }
			});
			await settle();
			trigger().click();
			await settle();
			const select = statusSelect()!;
			select.value = 'dropped';
			select.dispatchEvent(new Event('change', { bubbles: true }));
			await settle();
			saveButton()!.click();
			await settle();
			trigger().click();
			await settle();
		}

		/** A plain Save: is it still the retry's status write? */
		async function plainSaveIsAStatusWrite(): Promise<boolean> {
			partial.answers.push({ written: 3, failed: 0 });
			trigger().click();
			await settle();
			saveButton()!.click();
			await settle();
			return partial.calls[partial.calls.length - 1].statusChanged;
		}

		it('a clean save once a third tracker is connected settles the retry', async () => {
			await partialRetry('44');
			connect('inhouse', 'user-k');
			partial.answers.push({ written: 3, failed: 0 });
			trigger().click();
			await settle();
			saveButton()!.click();
			await settle();
			expect(partial.calls).toHaveLength(2);
			expect(await plainSaveIsAStatusWrite()).toBe(false);
		});

		it('a clean save once a tracker is disconnected settles the retry', async () => {
			await partialRetry('45');
			accountStore.setDisconnected('mal');
			partial.answers.push({ written: 1, failed: 0 });
			trigger().click();
			await settle();
			saveButton()!.click();
			await settle();
			expect(await plainSaveIsAStatusWrite()).toBe(false);
		});

		it('a removal once a tracker is disconnected settles the retry', async () => {
			await partialRetry('46');
			accountStore.setDisconnected('mal');
			vi.mocked(syncRemoveEntry).mockResolvedValueOnce({ removed: 1, failed: 0 });
			trigger().click();
			await settle();
			removeButton()!.click();
			await settle();
			expect(await plainSaveIsAStatusWrite()).toBe(false);
		});

		it('a removal under the same accounts clears its own retry', async () => {
			await partialRetry('47');
			vi.mocked(syncRemoveEntry).mockResolvedValueOnce({ removed: 2, failed: 0 });
			trigger().click();
			await settle();
			removeButton()!.click();
			await settle();
			expect(await plainSaveIsAStatusWrite()).toBe(false);
		});
	});
});
