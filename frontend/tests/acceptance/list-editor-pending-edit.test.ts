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
	calls: [] as { status: string; statusChanged: boolean }[]
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
			return { written: 1, failed: 1 };
		}
	),
	syncRemoveEntry: vi.fn()
}));

import ListEntryEditor from '../../src/lib/components/ListEntryEditor.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	partial.calls = [];
	server.use();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

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
});
