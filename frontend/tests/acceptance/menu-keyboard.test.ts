// Acceptance: the app's popup menus outside the play page answer the
// keyboard the way the play page's menus do.
//
// The account chip's menu, the download dock's and the Continue row's
// are announced as menus (`role="menu"`), which promises a menu's
// keys. Opening one puts focus on its first item; Down and Up step
// through the items and wrap at the ends, Home and End go to the
// first and last; Left and Right go nowhere and reach nothing else —
// the download dock opens over the play page, whose player takes the
// arrows as volume and seek keys. A typed character stays in the menu
// too: n, p, f and m are the player's keys. Escape and Shift+Tab close
// the menu with focus back on its button; Tab closes it with focus
// moving on from the button. A key with Alt, Ctrl or Meta held is
// the browser's.
//
// The dock's items change while it is open: a download that ends
// swaps its Cancel for Reveal and Dismiss, and a dismissed row takes
// its actions with it. Focus on an item that went moves to the item
// now in its place. The last row dismissed takes the dock away, and
// focus moves on to the control after it; the dock that comes back for
// the next download comes back closed. The account chip's menu goes
// the same way with the chip when the account signs out.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers } from './home-handlers';

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

import AccountChip from '../../src/lib/components/AccountChip.svelte';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';
import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import { formatSpeed } from '../../src/lib/download/speed';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
});

const settle = async (steps = 4) => {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 0));
};

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.innerHTML}`);
}

type Mods = Partial<Record<'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey', boolean>>;

/** Presses `key` where focus is, as the keyboard does; answers the
 *  event, to read whether the page let the browser act on it. */
async function press(key: string, mods: Mods = {}): Promise<KeyboardEvent> {
	const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...mods });
	(document.activeElement ?? document.body).dispatchEvent(e);
	await settle();
	return e;
}

const focused = () => document.activeElement as HTMLElement | null;
/** The focused element's name, as a screen reader reads it out. */
const focusedName = () => focused()?.getAttribute('aria-label') ?? focused()?.textContent?.trim();
const menu = () => target.querySelector<HTMLElement>('[role="menu"]');
const items = () => Array.from(menu()?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);

/** The menu's button: it says it opens a menu, and which one. */
function trigger(): HTMLButtonElement {
	const button = target.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');
	if (!button) throw new Error(`no menu button\n${target.innerHTML}`);
	return button;
}

async function openMenu() {
	trigger().click();
	await until(() => menu() !== null, 'the menu to open');
	await settle();
	expect(trigger().getAttribute('aria-expanded')).toBe('true');
	expect(trigger().getAttribute('aria-controls')).toBe(menu()!.id);
}

/** A page listener of the play page's kind: what reaches it would be a
 *  volume or seek key. */
function pageKeys() {
	const heard: string[] = [];
	const listener = (e: KeyboardEvent) => heard.push(e.key);
	window.addEventListener('keydown', listener);
	return { heard, stop: () => window.removeEventListener('keydown', listener) };
}

/** Steps through `names`, the menu's items in order, as the keys do. */
async function walk(names: string[]) {
	expect(focusedName()).toBe(names[0]);
	for (let i = 1; i < names.length; i++) {
		await press('ArrowDown');
		expect(focusedName()).toBe(names[i]);
	}
	await press('ArrowDown');
	expect(focusedName()).toBe(names[0]);
	await press('ArrowUp');
	expect(focusedName()).toBe(names[names.length - 1]);
	await press('Home');
	expect(focusedName()).toBe(names[0]);
	await press('End');
	expect(focusedName()).toBe(names[names.length - 1]);
}

/** The keys that leave the menu: Escape and Shift+Tab put focus back
 *  on its button, Tab hands the browser the button to move on from. */
async function leaves() {
	for (const [key, mods] of [
		['Escape', {}],
		['Tab', { shiftKey: true }],
		['Tab', {}]
	] as const) {
		await openMenu();
		const e = await press(key, mods);
		expect(menu(), `${key} closes the menu`).toBeNull();
		expect(focused()).toBe(trigger());
		expect(trigger().getAttribute('aria-expanded')).toBe('false');
		// Tab alone is left to the browser, to move on from the button.
		expect(e.defaultPrevented).toBe(key === 'Escape' || mods.shiftKey === true);
	}
}

/** Left and Right, the vertical keys and typed characters stay in the
 *  menu — on the play page n, f and m are the player's next-episode,
 *  fullscreen and mute keys; a key with Alt held is the browser's. */
async function holdsItsKeys() {
	const keys = pageKeys();
	for (const key of ['ArrowLeft', 'ArrowRight', 'ArrowDown', 'ArrowUp', 'n', 'f', 'm'])
		await press(key);
	await press('Home');
	keys.stop();
	expect(keys.heard).toEqual([]);
	await press('Home');
	const modified = await press('ArrowLeft', { altKey: true });
	expect(modified.defaultPrevented).toBe(false);
	expect(menu()).not.toBeNull();
}

describe('the account chip’s menu', () => {
	beforeEach(() => {
		signIn();
	});

	function signIn() {
		accountStore.setConnected('anilist', {
			access_token: 'test-bearer',
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-1',
			username: 'tester',
			avatar_url: null
		});
	}

	it('opens on its first item and steps through Manage and Disconnect, wrapping', async () => {
		app = mount(AccountChip, { target });
		await settle();
		await openMenu();
		await walk([m.account_chip_pop_manage(), m.account_chip_pop_disconnect()]);
		await holdsItsKeys();
		// The header naming the account describes the menu, and is no
		// landmark inside it.
		expect(menu()!.querySelector('header, footer, nav, aside, main, section')).toBeNull();
		const header = document.getElementById(menu()!.getAttribute('aria-describedby') ?? '');
		expect(header?.textContent).toContain('tester');
	});

	it('closes on Escape, Shift+Tab and Tab, with focus on its button', async () => {
		app = mount(AccountChip, { target });
		await settle();
		await leaves();
	});

	it('goes with the chip when the account signs out, and the chip comes back closed', async () => {
		app = mount(AccountChip, { target });
		await settle();
		await openMenu();
		accountStore.setDisconnected('anilist');
		await until(() => target.querySelector('button') === null, 'the chip to go');
		signIn();
		await until(() => target.querySelector('button') !== null, 'the chip to come back');
		expect(menu()).toBeNull();
		expect(trigger().getAttribute('aria-expanded')).toBe('false');
	});
});

describe('the download dock’s menu', () => {
	function seed() {
		// Newest first: the active download, then two that finished.
		const done = downloadStore.add({
			title: 'Frieren',
			episode: '1',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markDone(done, '/dl');
		const failed = downloadStore.add({
			title: 'Dandadan',
			episode: '2',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markError(failed, 'boom');
		const active = downloadStore.add({
			title: 'Mushishi',
			episode: '3',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markActive(active, new AbortController());
		return { active, done, failed };
	}

	const cancel = m.download_dock_cancel_aria_label();
	const reveal = m.download_dock_reveal_aria_label();
	const dismiss = m.download_dock_dismiss_aria_label();

	it('makes every action an item, each telling which download it acts on', async () => {
		seed();
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		// Nothing in the menu takes focus without being one of its items.
		const buttons = Array.from(menu()!.querySelectorAll('button'));
		expect(buttons.every((b) => b.getAttribute('role') === 'menuitem')).toBe(true);
		const described = items().map((el) =>
			(el.getAttribute('aria-describedby') ?? '')
				.split(' ')
				.map((id) => document.getElementById(id)?.textContent ?? '')
				.join(' ')
		);
		expect(described[0]).toContain('Mushishi');
		expect(described[1]).toContain('Dandadan');
		expect(described[2]).toContain('Frieren');
		expect(described[3]).toContain('Frieren');
	});

	it('tells a queued download’s Cancel apart from a running one’s', async () => {
		const queued = downloadStore.add({
			title: 'Frieren',
			episode: '1',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markQueued(queued, new AbortController());
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		const description = (items()[0].getAttribute('aria-describedby') ?? '')
			.split(' ')
			.map((id) => document.getElementById(id)?.textContent ?? '')
			.join(' ');
		expect(description).toContain('Frieren');
		expect(description).toContain(m.download_dock_queued());
	});

	it('tells where each download stands, without announcing every change', async () => {
		downloadStore.add({
			title: 'Frieren',
			episode: '1',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		const retrying = downloadStore.add({
			title: 'Dandadan',
			episode: '2',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markActive(retrying, new AbortController());
		downloadStore.setProgress(retrying, 'status.download.retry_ffmpeg');
		const running = downloadStore.add({
			title: 'Mushishi',
			episode: '3',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markActive(running, new AbortController());
		downloadStore.setProgress(running, 'status.download.rate 2500000');
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		const description = (el: HTMLElement) =>
			(el.getAttribute('aria-describedby') ?? '')
				.split(' ')
				.map((id) => document.getElementById(id)?.textContent?.trim() ?? '')
				.join(' ');
		// Newest first: running, retrying, starting.
		const [onRunning, onRetrying, onStarting] = items().map(description);
		expect(onRunning).toContain('Mushishi');
		expect(onRunning).toContain(
			m.download_dock_state_downloading_speed({ speed: formatSpeed(2_500_000) })
		);
		expect(onRetrying).toContain(m.download_dock_state_downloading());
		expect(onRetrying).toContain(m.download_status_retry_ffmpeg());
		expect(onStarting).toContain('Frieren');
		expect(onStarting).toContain(m.download_dock_state_starting());
		// A description is read when the item is reached; nothing in the
		// menu speaks up on its own as the speed ticks.
		expect(menu()!.querySelector('[aria-live], [role="status"], [role="progressbar"]')).toBeNull();
	});

	it('opens on its first item and steps through every action, wrapping', async () => {
		seed();
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		await walk([cancel, dismiss, reveal, dismiss]);
		await holdsItsKeys();
	});

	it('closes on Escape, Shift+Tab and Tab, with focus on its button', async () => {
		seed();
		app = mount(DownloadDock, { target });
		await settle();
		await leaves();
	});

	it('keeps focus in the menu when the download it was on ends or goes', async () => {
		const { active, failed } = seed();
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		expect(focusedName()).toBe(cancel);

		// The download ends: its Cancel goes, and its Reveal takes the place.
		downloadStore.markDone(active, '/dl');
		await settle();
		expect(focusedName()).toBe(reveal);
		expect(menu()!.contains(focused())).toBe(true);

		// Dismissed from the keyboard: the row goes, and the item now in
		// its place — the next row's first — takes focus.
		await press('ArrowDown');
		expect(focusedName()).toBe(dismiss);
		focused()!.click();
		await settle();
		expect(items()).toHaveLength(3);
		expect(focused()).toBe(items()[1]);
		expect(focusedName()).toBe(reveal);

		// The last item dismissed: focus takes the one now last.
		await press('End');
		focused()!.click();
		await settle();
		expect(items()).toHaveLength(1);
		expect(focused()).toBe(items()[0]);
		expect(downloadStore.items.map((i) => i.id)).toEqual([failed]);
	});

	it('takes the menu away with the dock, and the next download’s dock comes back closed', async () => {
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '1',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		downloadStore.markDone(id, '/dl');
		// The control the top bar has after the dock.
		const next = document.createElement('button');
		next.textContent = 'next';
		target.after(next);
		app = mount(DownloadDock, { target });
		await settle();
		await openMenu();
		await press('ArrowDown');
		expect(focusedName()).toBe(m.download_dock_dismiss_aria_label());
		focused()!.click();
		await until(() => target.querySelector('button') === null, 'the dock to go');
		// Focus went with the dock to what follows it, not to the page.
		expect(focused()).toBe(next);
		next.remove();

		downloadStore.add({
			title: 'Dandadan',
			episode: '2',
			mode: 'sub',
			quality: 'best',
			destDir: '/dl'
		});
		await until(() => target.querySelector('button') !== null, 'the dock to come back');
		expect(menu()).toBeNull();
		expect(trigger().getAttribute('aria-expanded')).toBe('false');
	});
});

describe('the Continue row’s menu', () => {
	async function home() {
		server.use(
			...homeHandlers({ history: [{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' }] })
		);
		app = mount(HomePage, { target });
		await until(
			() =>
				target.querySelector(`button[aria-label="${m.home_clear_menu_button_aria()}"]`) !== null,
			'the Continue row'
		);
	}

	it('is named by its button and opens on Clear, the one item, which the keys keep', async () => {
		await home();
		await openMenu();
		expect(trigger().getAttribute('aria-label')).toBe(m.home_clear_menu_button_aria());
		expect(menu()!.getAttribute('aria-labelledby')).toBe(trigger().id);
		await walk([m.home_clear_menu_item()]);
		await holdsItsKeys();
	});

	it('closes on Escape, Shift+Tab and Tab, with focus on its button', async () => {
		await home();
		await leaves();
	});
});
