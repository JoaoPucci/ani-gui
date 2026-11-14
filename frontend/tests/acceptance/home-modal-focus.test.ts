// Acceptance: Home's modal surfaces hold the keyboard while they are up.
//
// When a Continue card's resume failed, its failure overlay covered the
// page for the pointer but not for the keyboard: focus stayed on the
// card behind it, Tab walked on to Clear history and the cards' remove
// chips, and their confirm dialog then opened underneath the overlay,
// with both answering Escape at once. The resume's loading overlay and
// the confirm dialogs had the same gap.
//
// One modal at a time, as a well-made app does it: a modal takes focus
// as it opens, Tab and Shift+Tab cycle inside it, the rest of the page
// is inert, Escape does what its close does, and focus goes back to
// the control that opened it.

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
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	server.use(
		...homeHandlers({ history: [{ ep_no: '3', id: 'hianime:cowboy-bebop-1', title: SHOW }] }, [
			http.post(`${API_BASE}/api/availability/batch`, () =>
				HttpResponse.json({ cached: {}, playable_episode_counts: {} })
			),
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
	window.localStorage.clear();
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

const card = () => target.querySelector<HTMLButtonElement>('button.resume-card');
const clearAction = () =>
	(Array.from(target.querySelectorAll('.strip-header button')).find(
		(b) => b.textContent?.trim() === m.home_clear_history_action()
	) as HTMLButtonElement | undefined) ?? null;
const removeChip = () =>
	(Array.from(target.querySelectorAll('button')).find(
		(b) => b.getAttribute('aria-label') === m.home_delete_card_aria_label({ title: SHOW })
	) as HTMLButtonElement | undefined) ?? null;
const failure = () => target.querySelector<HTMLElement>('[role="alertdialog"]');
const confirm = () => target.querySelector<HTMLElement>('[role="dialog"]');
const loading = () => target.querySelector<HTMLElement>('[role="status"].backdrop');
const inert = (el: Element | null) => el !== null && el.closest('[inert]') !== null;
const active = () => document.activeElement;

/** Presses a key where focus is, and says whether the page kept the
 *  browser from doing its own thing with it. */
function press(key: string, shiftKey = false): boolean {
	const at = (document.activeElement ?? document.body) as HTMLElement;
	const ev = new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true });
	at.dispatchEvent(ev);
	return ev.defaultPrevented;
}

async function mountHome() {
	app = mount(HomePage, { target });
	await until(() => card() !== null && !card()!.disabled, 'the resolved Continue card');
	await until(() => clearAction() !== null && removeChip() !== null, 'the row actions');
}

/** Click the card from the keyboard's place on it; the resume waits
 *  on its session. */
async function startResume(): Promise<{ opener: HTMLButtonElement; stream: FakeEventSource }> {
	const opener = card()!;
	opener.focus();
	opener.click();
	await until(() => FakeEventSource.instances.length > 0, 'the resume play stream');
	return { opener, stream: FakeEventSource.instances[0] };
}

async function failResume() {
	const started = await startResume();
	started.stream.dispatch('error', JSON.stringify({ kind: 'timeout' }));
	await until(() => failure() !== null, 'the resume failure');
	await until(() => loading() === null, 'the loading overlay to leave');
	await settle();
	return started;
}

describe('the failure overlay of a Continue resume', () => {
	it('is a labelled modal dialog', async () => {
		await mountHome();
		await failResume();
		const dialog = failure()!;
		expect(dialog.getAttribute('aria-modal')).toBe('true');
		const label = document.getElementById(dialog.getAttribute('aria-labelledby') ?? '');
		expect(label?.textContent).toContain(SHOW);
		const description = document.getElementById(dialog.getAttribute('aria-describedby') ?? '');
		expect(description?.textContent?.trim().length).toBeGreaterThan(0);
	});

	it('takes focus as it opens', async () => {
		await mountHome();
		await failResume();
		expect(failure()!.contains(active())).toBe(true);
	});

	it('keeps Tab and Shift+Tab inside it', async () => {
		await mountHome();
		await failResume();
		for (const shift of [false, true, false, false, true, true]) {
			expect(press('Tab', shift)).toBe(true);
			expect(failure()!.contains(active())).toBe(true);
		}
	});

	it('leaves Clear history, the remove chips and the cards out of reach', async () => {
		await mountHome();
		await failResume();
		expect(inert(failure())).toBe(false);
		expect(inert(clearAction())).toBe(true);
		expect(inert(removeChip())).toBe(true);
		expect(inert(card())).toBe(true);
	});

	it('opens no confirm dialog underneath it', async () => {
		await mountHome();
		await failResume();
		clearAction()!.click();
		removeChip()!.click();
		await settle();
		expect(confirm()).toBeNull();
	});

	it('closes on Escape, gives the page back, and returns focus to the card', async () => {
		await mountHome();
		const { opener } = await failResume();
		expect(press('Escape')).toBe(true);
		await until(() => failure() === null, 'the overlay to close');
		expect(inert(clearAction())).toBe(false);
		expect(inert(card())).toBe(false);
		await until(() => active() === opener, 'focus back on the card');
	});

	it('returns focus to the card when its button closes it', async () => {
		await mountHome();
		const { opener } = await failResume();
		const dismiss = Array.from(failure()!.querySelectorAll('button')).find(
			(b) => b.textContent?.trim() === m.errors_dismiss_default_label()
		)!;
		dismiss.click();
		await until(() => failure() === null, 'the overlay to close');
		await until(() => active() === opener, 'focus back on the card');
	});
});

describe('while a Continue resume runs', () => {
	it('holds the page, so no confirm can be open when its failure lands', async () => {
		await mountHome();
		const { stream } = await startResume();
		await until(() => loading() !== null, 'the loading overlay');
		expect(loading()!.contains(active())).toBe(true);
		expect(press('Tab')).toBe(true);
		expect(loading()!.contains(active())).toBe(true);
		expect(inert(clearAction())).toBe(true);
		expect(inert(removeChip())).toBe(true);
		clearAction()!.click();
		removeChip()!.click();
		await settle();
		expect(confirm()).toBeNull();

		stream.dispatch('error', JSON.stringify({ kind: 'timeout' }));
		await until(() => failure() !== null, 'the resume failure');
		expect(confirm()).toBeNull();
	});
});

describe('a confirm dialog on Home', () => {
	for (const [name, opener] of [
		['Clear history', clearAction],
		['a card’s remove chip', removeChip]
	] as const) {
		it(`opened from ${name} keeps Tab inside it and the page behind it inert`, async () => {
			await mountHome();
			const from = opener()!;
			from.focus();
			from.click();
			await until(() => confirm() !== null, 'the confirm dialog');
			expect(confirm()!.contains(active())).toBe(true);
			for (const shift of [false, true, true, false]) {
				expect(press('Tab', shift)).toBe(true);
				expect(confirm()!.contains(active())).toBe(true);
			}
			expect(inert(card())).toBe(true);
			expect(inert(from)).toBe(true);

			expect(press('Escape')).toBe(true);
			await until(() => confirm() === null, 'the dialog to close');
			expect(inert(card())).toBe(false);
			await until(() => active() === from, 'focus back where the dialog was opened');
		});
	}

	it('starts no resume underneath it', async () => {
		await mountHome();
		clearAction()!.click();
		await until(() => confirm() !== null, 'the confirm dialog');
		card()!.click();
		await settle();
		expect(FakeEventSource.instances).toHaveLength(0);
		expect(loading()).toBeNull();
	});
});
