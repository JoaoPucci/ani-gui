// Acceptance: a keyring that has not answered does not hold the app.
//
// Reading a saved sign-in asks the OS keyring to decrypt it. A locked
// keyring answers only once the user unlocks it, a keyring never set up
// only once its "new keyring" prompt is answered, and an unresponsive
// secret service not until the bus gives up. The read used to be a
// synchronous IPC call: the renderer's thread waited with it and the
// first page never rendered.
//
// Here the keyring holds the AniList read until the test answers it.
// The home page renders signed out meanwhile; once the read has run
// past its wait a notice says the app is waiting on the keyring; when
// the keyring answers, the account is taken, the notice comes down and
// the signed-in home loads the tracker's list.

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
import { KEYRING_WAIT_MS } from '../../src/lib/account/keyring-wait';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import type { PersistedAccount, Provider } from '../../src/lib/account/types';

const WAITING =
	'Waiting for your system keyring. Your accounts will load as soon as it answers. Unlock it if asked.';

const ACCOUNT: PersistedAccount = {
	access_token: 'al-bearer',
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id: '7',
	username: 'shiro',
	avatar_url: null
};

let layoutTarget: HTMLElement;
let homeTarget: HTMLElement;
let layout: ReturnType<typeof mount> | null = null;
let home: ReturnType<typeof mount> | null = null;
let answerAniList: (r: unknown) => void = () => {};

/** A keychain whose AniList read waits for the test; the others have
 *  nothing saved. */
function installSlowKeychain() {
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		account: {
			getToken: (provider: Provider) =>
				provider === 'anilist'
					? new Promise((resolve) => (answerAniList = resolve))
					: Promise.resolve({ ok: false, kind: 'not_found' }),
			setToken: async () => ({ ok: true }),
			clearToken: async () => ({ ok: true })
		}
	};
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
	window.localStorage.clear();
	layoutTarget = document.createElement('div');
	homeTarget = document.createElement('div');
	document.body.append(layoutTarget, homeTarget);
});

afterEach(async () => {
	await settle(4);
	if (home) unmount(home);
	if (layout) unmount(layout);
	home = null;
	layout = null;
	layoutTarget.remove();
	homeTarget.remove();
	for (const t of [...toastStore.items]) toastStore.dismiss(t.id);
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

describe('a keyring that has not answered', () => {
	it('renders the home signed out, says it is waiting, then signs in when the keyring answers', async () => {
		installSlowKeychain();
		const listed: string[] = [];
		server.use(
			http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
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

		const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
		layout = mount(Layout, { target: layoutTarget, props: { children } });
		home = mount(HomePage, { target: homeTarget });
		await settle();

		// The page is up, signed out, and asks the tracker for nothing.
		expect(layoutTarget.querySelector('.topbar, header')).not.toBeNull();
		expect(homeTarget.childElementCount).toBeGreaterThan(0);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(listed).toEqual([]);
		expect(toastTexts()).toEqual([]);

		// Past the wait, the notice says why nobody is signed in yet.
		await new Promise((r) => setTimeout(r, KEYRING_WAIT_MS + 100));
		await settle(4);
		expect(toastTexts()).toEqual([WAITING]);

		// The keyring answers: the account is taken, the notice comes
		// down, and the signed-in home loads the tracker's list.
		answerAniList({ ok: true, payload: ACCOUNT });
		await settle(80);
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
		expect(toastTexts()).toEqual([]);
		expect(listed).toEqual(['anilist Bearer al-bearer']);
	});
});
