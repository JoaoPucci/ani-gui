// Acceptance: an account action whose bridge call fails never leaves
// the account stuck.
//
// Signing in, cancelling a sign-in and disconnecting each ask the
// Electron main process for something over IPC: open the browser and
// wait for the callback, stop that wait, delete the saved token. The
// main process catches its own failures and answers with a reason, but
// the IPC call itself can still fail, and then the app learns nothing
// about whether the request was carried out. The app has no reload, so
// a card left on "Signing in…", or an account left mid-change, stayed
// that way until it was restarted.
//
// Each scenario fails one of those calls on the Accounts page and
// checks that the card comes back to something the user can act on,
// that it shows what the disk really holds, and that a notice says what
// went wrong.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet } from 'svelte';

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

import Layout from '../../src/routes/+layout.svelte';
import AccountPage from '../../src/routes/account/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import type { PersistedAccount, Provider } from '../../src/lib/account/types';

const SIGN_IN_FAILED = 'Sign-in failed. Try again.';
const SIGN_IN_CANCELLED = 'Sign-in was cancelled.';
const CLEAR_FAILED = "Couldn't clear the local token. Try again or check ani-gui logs.";

const SAVED: PersistedAccount = {
	access_token: 'al-bearer',
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id: '7',
	username: 'shiro',
	avatar_url: null
};

let layoutTarget: HTMLElement;
let pageTarget: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let accountPage: ReturnType<typeof mount> | null = null;

/** What the keychain holds per provider. */
let disk: Partial<Record<Provider, PersistedAccount>> = {};

interface BridgeCalls {
	openOAuth?: () => Promise<unknown>;
	cancelOAuth?: () => Promise<unknown>;
	clearToken?: (provider: Provider) => Promise<unknown>;
}

function installBridge(calls: BridgeCalls) {
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: {
			getToken: async (provider: Provider) =>
				disk[provider] ? { ok: true, payload: disk[provider] } : { ok: false, kind: 'not_found' },
			setToken: async () => ({ ok: true }),
			clearToken: calls.clearToken ?? (async () => ({ ok: true })),
			openOAuth: calls.openOAuth ?? (() => new Promise(() => {})),
			cancelOAuth: calls.cancelOAuth ?? (async () => true)
		}
	};
}

const ipcFailure = () => Promise.reject(new Error('An object could not be cloned.'));

const toastTexts = () =>
	Array.from(layoutTarget.querySelectorAll('.toast .message')).map(
		(el) => el.textContent?.trim() ?? ''
	);

function card(provider: Provider): HTMLElement {
	const name = provider === 'anilist' ? 'AniList' : 'MyAnimeList';
	const found = Array.from(pageTarget.querySelectorAll<HTMLElement>('.provider-card')).find(
		(el) => el.querySelector('.provider-name')?.textContent?.trim() === name
	);
	if (!found) throw new Error(`no ${name} card`);
	return found;
}

function button(provider: Provider, label: string): HTMLButtonElement {
	const found = Array.from(card(provider).querySelectorAll('button')).find(
		(b) => b.textContent?.trim() === label
	);
	if (!found) throw new Error(`no "${label}" button on the ${provider} card`);
	return found;
}

const buttonLabels = (provider: Provider) =>
	Array.from(card(provider).querySelectorAll('button')).map((b) => b.textContent?.trim());

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

async function open() {
	server.use(
		http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
		...homeHandlers({}, [
			http.post(`${API_BASE}/api/account/auth-url/:provider`, () =>
				HttpResponse.json({ url: 'https://anilist.co/api/v2/oauth/authorize?x=1' })
			),
			http.post(`${API_BASE}/api/account/list/:provider`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/account/list/:provider/cached`, () => HttpResponse.json([])),
			http.delete(`${API_BASE}/api/account/list/:provider/cache`, () =>
				HttpResponse.json(null, { status: 204 })
			),
			http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
		])
	);
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target: layoutTarget, props: { children } });
	accountPage = mount(AccountPage, { target: pageTarget });
	await settle();
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	window.localStorage.clear();
	disk = {};
	layoutTarget = document.createElement('div');
	pageTarget = document.createElement('div');
	document.body.append(layoutTarget, pageTarget);
});

afterEach(async () => {
	await settle(4);
	if (accountPage) unmount(accountPage);
	if (layout) unmount(layout);
	accountPage = null;
	layout = null;
	layoutTarget.remove();
	pageTarget.remove();
	for (const t of [...toastStore.items]) toastStore.dismiss(t.id);
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

describe('an account action whose bridge call fails', () => {
	it('a sign-in that cannot reach the browser comes back to Sign in and says it failed', async () => {
		installBridge({ openOAuth: ipcFailure });
		await open();
		expect(card('anilist').dataset.state).toBe('disconnected');

		button('anilist', 'Sign in').click();
		await settle();

		expect(card('anilist').dataset.state).toBe('disconnected');
		expect(buttonLabels('anilist')).toEqual(['Sign in']);
		expect(toastTexts()).toEqual([SIGN_IN_FAILED]);
	});

	it('a cancel that cannot reach the browser wait still cancels the sign-in', async () => {
		installBridge({ cancelOAuth: ipcFailure });
		await open();

		button('anilist', 'Sign in').click();
		await settle();
		expect(card('anilist').dataset.state).toBe('connecting');

		button('anilist', 'Cancel').click();
		await settle();

		expect(card('anilist').dataset.state).toBe('disconnected');
		expect(buttonLabels('anilist')).toEqual(['Sign in']);
		expect(toastTexts()).toEqual([SIGN_IN_CANCELLED]);
	});

	it('a disconnect whose clear cannot be confirmed shows the sign-in still saved and says the clear failed', async () => {
		disk.anilist = SAVED;
		installBridge({ clearToken: ipcFailure });
		await open();
		expect(card('anilist').dataset.state).toBe('connected');

		button('anilist', 'Disconnect').click();
		await settle();

		expect(card('anilist').dataset.state).toBe('connected');
		expect(button('anilist', 'Disconnect').disabled).toBe(false);
		expect(accountStore.accountChanging.anilist).toBe(false);
		expect(toastTexts()).toEqual([CLEAR_FAILED]);
	});

	it('a disconnect whose clear landed before its answer was lost ends signed out, with no failure notice', async () => {
		disk.anilist = SAVED;
		installBridge({
			clearToken: (provider) => {
				delete disk[provider];
				return ipcFailure();
			}
		});
		await open();

		button('anilist', 'Disconnect').click();
		await settle();

		expect(card('anilist').dataset.state).toBe('disconnected');
		expect(buttonLabels('anilist')).toEqual(['Sign in']);
		expect(toastTexts()).toEqual([]);
	});
});
