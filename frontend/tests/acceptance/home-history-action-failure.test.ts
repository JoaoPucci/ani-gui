// Acceptance: a destructive history action on Home that fails says so.
//
// Clear history and a Continue card's remove both ask first, in a
// confirm dialog, and then call the backend. When that call failed,
// the dialog closed, the cards stayed, the rejection went unhandled,
// and nothing told the user. A user could only conclude the click had
// been missed. Now a failure is reported the way the app reports other
// failed actions, an error toast, and the cards stay so the user can
// try again from the same control, which gets focus back.

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
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { m } from '../../src/lib/paraglide/messages';

const TITLE = 'Cowboy Bebop';
const HISTORY = [{ ep_no: '3', id: 'allanime-1', title: TITLE }];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let unhandled: unknown[] = [];
const onUnhandled = (reason: unknown) => unhandled.push(reason);

/** What a request to the history endpoints is answered with. */
type Answer = () => Promise<Response> | Response;
const refused = () =>
	HttpResponse.json({ key: 'error.io.generic', kind: 'io' }, { status: 500 }) as Response;
const unreachable = () => HttpResponse.error() as Response;
const done = () => new HttpResponse(null, { status: 204 }) as Response;

let clears = 0;
let deletes: string[] = [];

function serve(answers: { clear?: Answer; remove?: Answer }) {
	server.use(
		...homeHandlers({ history: HISTORY }, [
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([kitsuRef('1', TITLE, 26)])
			),
			http.delete(`${API_BASE}/api/history`, () => {
				clears += 1;
				return (answers.clear ?? done)();
			}),
			http.delete(`${API_BASE}/api/history/:id`, ({ params }) => {
				deletes.push(String(params.id));
				return (answers.remove ?? done)();
			})
		])
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	clears = 0;
	deletes = [];
	unhandled = [];
	process.on('unhandledRejection', onUnhandled);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	process.off('unhandledRejection', onUnhandled);
	if (app) unmount(app);
	app = null;
	target.remove();
	for (const t of [...toastStore.items]) toastStore.dismiss(t.id);
	localStorage.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}
const settle = () => new Promise((r) => setTimeout(r, 50));

const dialog = () => target.querySelector('[role="dialog"]');
const dialogButton = (label: string) =>
	(Array.from(target.querySelectorAll('[role="dialog"] button')).find(
		(b) => b.textContent?.trim() === label
	) as HTMLButtonElement | undefined) ?? null;
/** The Continue Watching row's Clear history action. */
const clearAction = () =>
	(Array.from(target.querySelectorAll('.strip-header button')).find(
		(b) => b.textContent?.trim() === m.home_clear_history_action()
	) as HTMLButtonElement | undefined) ?? null;
/** The remove chip on the card. */
const removeChip = () =>
	(Array.from(target.querySelectorAll('button')).find(
		(b) => b.getAttribute('aria-label') === m.home_delete_card_aria_label({ title: TITLE })
	) as HTMLButtonElement | undefined) ?? null;
const cards = () => target.querySelectorAll('[data-entry-id]').length;
const errorToasts = () => toastStore.items.filter((t) => t.kind === 'error').map((t) => t.message);

async function mountHome() {
	app = mount(HomePage, { target });
	await until(() => clearAction() !== null, 'the Clear history action');
	await until(() => removeChip() !== null, 'the card’s remove chip');
}

/** Opens Clear history from the keyboard's place on it. */
async function openClear() {
	const clear = clearAction()!;
	clear.focus();
	clear.click();
	await until(() => dialog() !== null, 'the confirm dialog');
	return clear;
}

async function openRemove() {
	const chip = removeChip()!;
	chip.focus();
	chip.click();
	await until(() => dialog() !== null, 'the confirm dialog');
	return chip;
}

describe('Clear history on Home, when the clear fails', () => {
	for (const [name, answer] of [
		['the backend refuses it', refused],
		['the backend cannot be reached', unreachable]
	] as const) {
		it(`says so when ${name}, keeps the cards, and gives focus back to Clear history`, async () => {
			serve({ clear: answer });
			await mountHome();
			const clear = await openClear();
			dialogButton(m.home_clear_confirm_button())!.click();

			await until(() => errorToasts().length > 0, 'the failure message');
			expect(errorToasts()).toEqual([m.home_clear_history_failed()]);
			await until(() => dialog() === null, 'the dialog to close');
			expect(clears).toBe(1);
			// The cards stay: nothing was cleared.
			expect(cards()).toBeGreaterThan(0);
			expect(clearAction()).not.toBeNull();
			await until(() => document.activeElement === clear, 'focus back on Clear history');
			await settle();
			expect(unhandled).toEqual([]);
		});
	}

	it('lets the user try again from the same action, and that try clears', async () => {
		let failing = true;
		serve({ clear: () => (failing ? refused() : done()) });
		await mountHome();
		await openClear();
		dialogButton(m.home_clear_confirm_button())!.click();
		await until(() => errorToasts().length > 0, 'the failure message');
		await until(() => dialog() === null, 'the dialog to close');

		failing = false;
		await openClear();
		dialogButton(m.home_clear_confirm_button())!.click();
		await until(() => clearAction() === null, 'the Continue Watching row to go');
		expect(clears).toBe(2);
		// The only message is the first try's.
		expect(errorToasts()).toEqual([m.home_clear_history_failed()]);
	});

	it('says nothing when the clear lands', async () => {
		serve({});
		await mountHome();
		await openClear();
		dialogButton(m.home_clear_confirm_button())!.click();
		await until(() => clearAction() === null, 'the Continue Watching row to go');
		expect(dialog()).toBeNull();
		expect(errorToasts()).toEqual([]);
	});

	it('sends one clear however many times confirm is pressed, and reports it once', async () => {
		let release: () => void = () => {};
		const held = new Promise<void>((r) => (release = r));
		serve({
			clear: async () => {
				await held;
				return refused();
			}
		});
		await mountHome();
		await openClear();
		const confirm = dialogButton(m.home_clear_confirm_button())!;
		// Two presses in the same turn, before the page can draw the
		// dialog busy.
		confirm.click();
		confirm.click();
		await settle();
		confirm.click();
		release();
		await until(() => dialog() === null, 'the dialog to close');
		expect(clears).toBe(1);
		expect(errorToasts()).toEqual([m.home_clear_history_failed()]);
	});

	it('stays open while the clear is out: Cancel and Esc wait for it to settle', async () => {
		let release: () => void = () => {};
		const held = new Promise<void>((r) => (release = r));
		serve({
			clear: async () => {
				await held;
				return refused();
			}
		});
		await mountHome();
		await openClear();
		dialogButton(m.home_clear_confirm_button())!.click();
		await until(() => clears === 1, 'the clear to go out');
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		dialogButton(m.confirm_button_cancel())!.click();
		(dialog() as HTMLElement).click();
		await settle();
		// Still open and busy: closing it now would hide the outcome.
		expect(dialog()).not.toBeNull();
		expect(dialogButton(m.home_clear_confirm_button())!.disabled).toBe(true);
		release();
		await until(() => errorToasts().length > 0, 'the failure message');
		await until(() => dialog() === null, 'the dialog to close');
		expect(cards()).toBeGreaterThan(0);
	});
});

describe('Removing a Continue card on Home, when the remove fails', () => {
	for (const [name, answer] of [
		['the backend refuses it', refused],
		['the backend cannot be reached', unreachable]
	] as const) {
		it(`says so when ${name}, keeps the card, and gives focus back to its remove chip`, async () => {
			serve({ remove: answer });
			await mountHome();
			const chip = await openRemove();
			dialogButton(m.home_delete_confirm_button())!.click();

			await until(() => errorToasts().length > 0, 'the failure message');
			expect(errorToasts()).toEqual([m.home_delete_failed({ title: TITLE })]);
			await until(() => dialog() === null, 'the dialog to close');
			expect(deletes).toEqual(['allanime-1']);
			expect(removeChip()).toBe(chip);
			await until(() => document.activeElement === chip, 'focus back on the remove chip');
			await settle();
			expect(unhandled).toEqual([]);
		});
	}

	it('sends one remove however many times confirm is pressed', async () => {
		let release: () => void = () => {};
		const held = new Promise<void>((r) => (release = r));
		serve({
			remove: async () => {
				await held;
				return refused();
			}
		});
		await mountHome();
		await openRemove();
		const confirm = dialogButton(m.home_delete_confirm_button())!;
		confirm.click();
		confirm.click();
		await settle();
		release();
		await until(() => dialog() === null, 'the dialog to close');
		expect(deletes).toEqual(['allanime-1']);
		expect(errorToasts()).toEqual([m.home_delete_failed({ title: TITLE })]);
	});

	it('says nothing when the remove lands', async () => {
		serve({});
		await mountHome();
		await openRemove();
		dialogButton(m.home_delete_confirm_button())!.click();
		await until(() => removeChip() === null, 'the card to go');
		expect(errorToasts()).toEqual([]);
	});
});

describe('Home’s history dialogs while a request is out', () => {
	const OTHER = 'Trigun';
	const chipFor = (title: string) =>
		(Array.from(target.querySelectorAll('button')).find(
			(b) => b.getAttribute('aria-label') === m.home_delete_card_aria_label({ title })
		) as HTMLButtonElement | undefined) ?? null;
	const dialogTitle = () => dialog()?.querySelector('h2')?.textContent ?? null;

	it('keeps the dialog on the remove that is out: another chip or Clear history opens nothing', async () => {
		let release: () => void = () => {};
		const held = new Promise<void>((r) => (release = r));
		server.use(
			...homeHandlers(
				{
					history: [
						{ ep_no: '3', id: 'allanime-1', title: TITLE },
						{ ep_no: '3', id: 'allanime-2', title: OTHER }
					]
				},
				[
					http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
						const { query } = (await request.json()) as { query: string };
						return HttpResponse.json(
							query === OTHER ? [kitsuRef('2', OTHER, 26)] : [kitsuRef('1', TITLE, 26)]
						);
					}),
					http.delete(`${API_BASE}/api/history`, () => {
						clears += 1;
						return done();
					}),
					http.delete(`${API_BASE}/api/history/:id`, async ({ params }) => {
						deletes.push(String(params.id));
						await held;
						return refused();
					})
				]
			)
		);
		app = mount(HomePage, { target });
		await until(() => chipFor(TITLE) !== null && chipFor(OTHER) !== null, 'both cards');

		chipFor(TITLE)!.click();
		await until(() => dialog() !== null, 'the confirm dialog');
		const asked = dialogTitle();
		dialogButton(m.home_delete_confirm_button())!.click();
		await until(() => deletes.length === 1, 'the remove to go out');

		// The backdrop stops a pointer, not the keyboard: a chip or the
		// row's action can still be pressed while the remove is out.
		chipFor(OTHER)!.click();
		clearAction()!.click();
		await settle();
		expect(target.querySelectorAll('[role="dialog"]').length).toBe(1);
		expect(dialogTitle()).toBe(asked);

		release();
		await until(() => dialog() === null, 'the dialog to close');
		expect(errorToasts()).toEqual([m.home_delete_failed({ title: TITLE })]);
		expect(deletes).toEqual(['allanime-1']);
		expect(clears).toBe(0);
	});
});
