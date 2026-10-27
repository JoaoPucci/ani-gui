// Acceptance: what the sign-in notice already did right, held in place.
//
// These scenarios pass with and without the change that taught the
// notice to say what a renewal found. They pin the behaviour that
// change must not disturb:
//
//   - a session that ran out with no way to renew it (AniList) still
//     gets the "session expired" notice on launch;
//   - a renewal that succeeds leaves no notice behind, and the renewed
//     tokens are saved;
//   - reconnecting or disconnecting the provider clears the notice;
//   - healthy sign-ins load their lists with no notice and no renewal;
//   - passing toasts still auto-dismiss and still share the capped
//     stack.

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
import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { toastStore, TOAST_MAX_STACK } from '../../src/lib/toasts/store.svelte';
import type { PersistedAccount, Provider } from '../../src/lib/account/types';

function account(over: Partial<PersistedAccount>): PersistedAccount {
	return {
		access_token: 'access',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: '7',
		username: 'shiro',
		avatar_url: null,
		...over
	};
}

const EXPIRED_ANILIST = 'Your AniList session expired. Sign in again to keep syncing.';

let layoutTarget: HTMLElement;
let homeTarget: HTMLElement | null = null;
let layout: ReturnType<typeof mount> | null = null;
let home: ReturnType<typeof mount> | null = null;
let saved: { provider: Provider; payload: PersistedAccount }[] = [];

function installKeychain(accounts: Partial<Record<Provider, PersistedAccount>>) {
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: {
			getToken: (provider: Provider) => {
				const payload = accounts[provider];
				return payload ? { ok: true, payload } : { ok: false, kind: 'not_found' };
			},
			setToken: async (provider: Provider, payload: PersistedAccount) => {
				saved.push({ provider, payload });
				return { ok: true };
			},
			clearToken: async () => ({ ok: true })
		}
	};
}

const quietBackend = [
	http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 }))
];

function bootLayout() {
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target: layoutTarget, props: { children } });
}

const toastTexts = () =>
	Array.from(layoutTarget.querySelectorAll('.toast .message')).map(
		(el) => el.textContent?.trim() ?? ''
	);

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	saved = [];
	window.localStorage.clear();
	layoutTarget = document.createElement('div');
	document.body.appendChild(layoutTarget);
});

afterEach(async () => {
	await settle(4);
	if (home) unmount(home);
	if (layout) unmount(layout);
	home = null;
	layout = null;
	homeTarget?.remove();
	homeTarget = null;
	layoutTarget.remove();
	for (const t of [...toastStore.items]) toastStore.dismiss(t.id);
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

describe('the sign-in notice, where it already worked', () => {
	it('tells the user on launch that an AniList session ran out', async () => {
		installKeychain({ anilist: account({ expires_at_epoch_s: 1_000 }) });
		server.use(...quietBackend);
		bootLayout();
		await settle();
		expect(toastTexts()).toEqual([EXPIRED_ANILIST]);
	});

	it('leaves no notice once a MAL renewal succeeds, and saves the renewed tokens', async () => {
		installKeychain({
			mal: account({ access_token: 'old', refresh_token: 'rt', expires_at_epoch_s: 1_000 })
		});
		server.use(
			...quietBackend,
			http.post(`${API_BASE}/api/account/refresh/mal`, () =>
				HttpResponse.json({
					access_token: 'renewed',
					refresh_token: 'rt2',
					expires_at_epoch_s: 4_000_000_000
				})
			)
		);
		bootLayout();
		await settle();
		expect(toastTexts()).toEqual([]);
		expect(accountStore.byProvider.mal.kind).toBe('connected');
		expect(saved).toEqual([
			{
				provider: 'mal',
				payload: account({
					access_token: 'renewed',
					refresh_token: 'rt2',
					expires_at_epoch_s: 4_000_000_000
				})
			}
		]);
	});

	it('clears the notice when the user reconnects', async () => {
		installKeychain({ anilist: account({ expires_at_epoch_s: 1_000 }) });
		server.use(...quietBackend);
		bootLayout();
		await settle();
		expect(toastTexts()).toEqual([EXPIRED_ANILIST]);
		accountStore.setConnected('anilist', account({}));
		await settle(10);
		expect(toastTexts()).toEqual([]);
	});

	it('clears the notice when the user disconnects', async () => {
		installKeychain({ anilist: account({ expires_at_epoch_s: 1_000 }) });
		server.use(...quietBackend);
		bootLayout();
		await settle();
		accountStore.setDisconnected('anilist');
		await settle(10);
		expect(toastTexts()).toEqual([]);
	});

	it('loads healthy AniList and MAL lists with no notice and no renewal', async () => {
		installKeychain({
			anilist: account({ access_token: 'al-bearer' }),
			mal: account({ access_token: 'mal-bearer', refresh_token: 'rt' })
		});
		const listed: string[] = [];
		let renewals = 0;
		server.use(
			...quietBackend,
			http.post(`${API_BASE}/api/account/refresh/:provider`, () => {
				renewals += 1;
				return HttpResponse.json(null, { status: 500 });
			}),
			...homeHandlers({}, [
				http.post(`${API_BASE}/api/account/list/:provider`, ({ params, request }) => {
					listed.push(`${params.provider} ${request.headers.get('authorization')}`);
					return HttpResponse.json([]);
				}),
				http.get(`${API_BASE}/api/account/list/:provider/cached`, () => HttpResponse.json([])),
				http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () => HttpResponse.json([])),
				http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
			])
		);
		bootLayout();
		homeTarget = document.createElement('div');
		document.body.appendChild(homeTarget);
		home = mount(HomePage, { target: homeTarget });
		await settle(80);
		expect(listed.sort()).toEqual(['anilist Bearer al-bearer', 'mal Bearer mal-bearer']);
		expect(renewals).toBe(0);
		expect(toastTexts()).toEqual([]);
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
		expect(accountStore.byProvider.mal.kind).toBe('connected');
	});

	it('still caps passing toasts and lets them go on their own', async () => {
		installKeychain({});
		server.use(...quietBackend);
		bootLayout();
		await settle(4);
		vi.useFakeTimers();
		try {
			for (let i = 0; i < TOAST_MAX_STACK + 2; i++) {
				toastStore.push({ kind: 'info', message: `passing ${i}` });
			}
			expect(toastStore.items.map((t) => t.message)).toEqual([
				'passing 2',
				'passing 3',
				'passing 4'
			]);
			vi.advanceTimersByTime(4_000);
			expect(toastStore.items).toEqual([]);
		} finally {
			vi.useRealTimers();
		}
	});
});
