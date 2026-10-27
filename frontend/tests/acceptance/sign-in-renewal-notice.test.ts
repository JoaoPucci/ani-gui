// Acceptance: opening the app with a MAL sign-in that can no longer be
// renewed tells the user so, once, in words that say what is known.
//
// The boot path is the layout's: it reads the persisted sign-in,
// finds the access token past its expiry, and asks the backend to
// renew it. What the tracker answers decides the notice:
//
//   - MAL refuses the refresh token: the only fix is a new sign-in.
//   - MAL does not answer: the sign-in may still be fine, and the app
//     tries again on its own.
//
// Before the renewal answers, nothing is known, so nothing is said:
// a notice that appears and vanishes on every launch where the renewal
// succeeds teaches the user to look past it. Once shown, the notice
// stays until the user acts or the sign-in recovers: later toasts do
// not push it off the stack, and the read that fails with the dead
// token after it does not show a second one.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse, delay } from 'msw';
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
import { toastStore } from '../../src/lib/toasts/store.svelte';
import type { PersistedAccount, Provider } from '../../src/lib/account/types';

/** A MAL sign-in whose access token ran out long ago and that carries a
 *  refresh token, as the keychain hands it back on a cold launch. */
const DEAD_MAL: PersistedAccount = {
	access_token: 'dead-access',
	refresh_token: 'dead-refresh',
	expires_at_epoch_s: 1_000,
	user_id: '42',
	username: 'shiro',
	avatar_url: null
};

const REFUSED = 'MAL refused to renew your sign-in. Reconnect to keep syncing.';
const UNREACHABLE =
	"Couldn't reach MAL to renew your sign-in. It may still be valid; the app keeps trying.";
const SIGN_IN_COPY = [
	REFUSED,
	UNREACHABLE,
	'Your MAL session expired. Sign in again to keep syncing.'
];

let layoutTarget: HTMLElement;
let homeTarget: HTMLElement | null = null;
let layout: ReturnType<typeof mount> | null = null;
let home: ReturnType<typeof mount> | null = null;
/** Answers a held renewal, so a scenario that fails before answering
 *  it does not also leave the request open. */
let release: () => void = () => {};

function installKeychain() {
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: {
			getToken: (provider: Provider) =>
				provider === 'mal' ? { ok: true, payload: DEAD_MAL } : { ok: false, kind: 'not_found' }
		}
	};
}

/** What the app has open besides the account flow, answered as a
 *  backend with nothing to say about it. */
const quietBackend = [
	http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 }))
];

const refusedRefresh = http.post(`${API_BASE}/api/account/refresh/mal`, () =>
	HttpResponse.json(
		{ kind: 'refresh_rejected', key: 'error.account.refresh_rejected' },
		{ status: 401 }
	)
);

const unreachableRefresh = http.post(`${API_BASE}/api/account/refresh/mal`, () =>
	HttpResponse.json(
		{ kind: 'upstream', key: 'error.network.upstream', status: 503 },
		{ status: 502 }
	)
);

function bootLayout() {
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target: layoutTarget, props: { children } });
}

/** The sign-in notices the user can see, by their words. */
const signInToasts = () =>
	Array.from(layoutTarget.querySelectorAll('.toast .message'))
		.map((el) => el.textContent?.trim() ?? '')
		.filter((text) => SIGN_IN_COPY.includes(text));

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	installKeychain();
	layoutTarget = document.createElement('div');
	document.body.appendChild(layoutTarget);
});

afterEach(async () => {
	release();
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
	release = () => {};
});

describe('opening the app with a dead MAL sign-in', () => {
	it('says nothing while the renewal is out, then asks to reconnect once MAL refuses it', async () => {
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		server.use(
			...quietBackend,
			http.post(`${API_BASE}/api/account/refresh/mal`, async () => {
				await held;
				return HttpResponse.json(
					{ kind: 'refresh_rejected', key: 'error.account.refresh_rejected' },
					{ status: 401 }
				);
			})
		);
		bootLayout();
		await settle();
		expect(signInToasts(), 'nothing is known before MAL answers').toEqual([]);

		release();
		await settle();
		expect(signInToasts()).toEqual([REFUSED]);
	});

	it('says MAL could not be reached, not that the sign-in is dead, when MAL does not answer', async () => {
		server.use(...quietBackend, unreachableRefresh);
		bootLayout();
		await settle();
		expect(signInToasts()).toEqual([UNREACHABLE]);
	});

	it('keeps the notice on screen through a burst of later toasts', async () => {
		server.use(...quietBackend, refusedRefresh);
		bootLayout();
		await settle();
		for (let i = 0; i < 4; i++) toastStore.push({ kind: 'info', message: `passing ${i}` });
		await settle(4);
		expect(signInToasts()).toEqual([REFUSED]);
	});

	it('shows one notice, not a second one, when the list read fails with the dead token', async () => {
		let listReads = 0;
		server.use(
			...quietBackend,
			refusedRefresh,
			...homeHandlers({}, [
				http.post(`${API_BASE}/api/account/list/mal`, async () => {
					listReads += 1;
					await delay(5);
					return HttpResponse.json(
						{ kind: 'invalid_token', key: 'error.stream.invalid_token' },
						{ status: 401 }
					);
				}),
				http.get(`${API_BASE}/api/account/list/mal/cached`, () => HttpResponse.json([])),
				http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () => HttpResponse.json([])),
				http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
			])
		);
		bootLayout();
		homeTarget = document.createElement('div');
		document.body.appendChild(homeTarget);
		home = mount(HomePage, { target: homeTarget });
		await settle(80);
		expect(listReads, 'the home rail asked MAL for the list').toBeGreaterThan(0);
		expect(signInToasts()).toEqual([REFUSED]);
	});

	it('tells the outcome again after the user asks to try now', async () => {
		let renewals = 0;
		server.use(
			...quietBackend,
			http.post(`${API_BASE}/api/account/refresh/mal`, () => {
				renewals += 1;
				return HttpResponse.json(
					{ kind: 'upstream', key: 'error.network.upstream', status: 503 },
					{ status: 502 }
				);
			})
		);
		bootLayout();
		await settle();
		expect(signInToasts()).toEqual([UNREACHABLE]);

		const tryNow = Array.from(
			layoutTarget.querySelectorAll<HTMLButtonElement>('.toast .action')
		).find((b) => b.textContent?.trim() === 'Try now');
		tryNow?.click();
		await settle();
		expect(renewals, 'the click renewed again').toBe(2);
		// The user acted and the renewal failed again: saying nothing now
		// would read as success.
		expect(signInToasts()).toEqual([UNREACHABLE]);
	});
});
