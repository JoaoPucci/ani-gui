/**
 * The account page's Connect / Reconnect and Cancel, run against the
 * real account store: what each outcome of the sign-in leaves the
 * provider in, and what the user is told.
 *
 * The case these exist for is a bridge call that rejects rather than
 * answering (`openOAuth`, `persistAccount`, `cancelOAuth`). The
 * main-process handlers catch their own failures, so a rejection is an
 * IPC failure, and the app cannot tell whether what the call asked for
 * happened. The provider must still leave `connecting` (the app has no
 * reload to get it out), end on what the disk holds, and say so.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { accountStore } from './store.svelte';
import { cancelConnect, connectProvider, type ConnectActionDeps } from './connect-action';
import type { ConnectFlowDeps } from './connect-flow';
import type { PersistedAccount, Provider } from './types';

// Resolve the locale once while no window is stubbed. Paraglide's first
// lookup records the locale it resolved, in localStorage when a window
// exists; the fake windows below carry none.
m.account_connect_error_unknown();

function account(overrides: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 'old-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: '7',
		username: 'shiro',
		avatar_url: null,
		...overrides
	};
}

/** What the keyring holds per provider; `reread` reads it. */
let disk: Partial<Record<Provider, PersistedAccount>> = {};

function installKeyring() {
	(globalThis as { window?: { aniGui?: unknown } }).window = {
		aniGui: {
			account: {
				getToken: async (p: Provider) =>
					disk[p] ? { ok: true, payload: disk[p] } : { ok: false, kind: 'not_found' }
			}
		}
	};
}

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

const NEW_TOKENS = { access_token: 'new-bearer', refresh_token: null, expires_at_epoch_s: 4e9 };
const PROFILE = { user_id: '7', username: 'shiro', avatar_url: null };

function flow(overrides: Partial<ConnectFlowDeps> = {}): ConnectFlowDeps {
	return {
		generateState: () => 'st',
		generatePkce: () => ({ verifier: 'v', challenge: 'c', method: 'plain' }),
		buildAuthUrl: async () => ({ url: 'https://auth.example/' }),
		openOAuth: async () => ({ ok: true, code: 'code', state: 'st' }),
		exchangeCode: async () => NEW_TOKENS,
		fetchMe: async () => PROFILE,
		persistAccount: async () => ({ ok: true }),
		...overrides
	};
}

let toasts: { kind: 'error'; message: string }[] = [];
let seeded: string[] = [];

/** Records each notice; a withdrawn one leaves the list. */
function pushToast(t: { kind: 'error'; message: string }): string {
	toasts.push(t);
	return String(toasts.length - 1);
}

function dismissToast(id: string): void {
	toasts.splice(Number(id), 1);
}

function deps(f: ConnectFlowDeps): ConnectActionDeps {
	return {
		store: accountStore,
		flow: f,
		seedConnectedList: async (p, bearer) => {
			seeded.push(`${p} ${bearer}`);
		},
		pushToast,
		dismissToast
	};
}

function cancelDeps(cancelOAuth: () => Promise<boolean>) {
	return {
		store: accountStore,
		cancelOAuth,
		pushToast
	};
}

const settle = async () => {
	for (let i = 0; i < 20; i++) await Promise.resolve();
};

const messages = () => toasts.map((t) => t.message);

beforeEach(() => {
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
	disk = {};
	toasts = [];
	seeded = [];
	installKeyring();
});

afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
});

describe('connectProvider: a bridge call that rejects', () => {
	it('an openOAuth that rejects puts a signed-out provider back to Connect and says sign-in failed', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: () => Promise.reject(new Error('ipc gone')) }))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(accountStore.accountChanging.anilist).toBe(false);
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it('a reconnect from expired whose openOAuth rejects keeps the expired sign-in', async () => {
		const held = account();
		disk.anilist = account({ expires_at_epoch_s: 1 });
		accountStore.setExpired('anilist', held);
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: () => Promise.reject(new Error('ipc gone')) }))
		);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('expired');
		expect(s.kind === 'expired' && s.account.access_token).toBe('old-bearer');
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it("wakes whoever waits on the provider's next change", async () => {
		const oauth = deferred<never>();
		const running = connectProvider('anilist', deps(flow({ openOAuth: () => oauth.promise })));
		await settle();
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		const woken = vi.fn();
		void accountStore.nextChange('anilist').then(woken);
		oauth.reject(new Error('ipc gone'));
		await running;
		await settle();
		expect(woken).toHaveBeenCalled();
	});

	it('a hydrate held behind the sign-in reads again once it settles', async () => {
		const oauth = deferred<never>();
		const running = connectProvider('anilist', deps(flow({ openOAuth: () => oauth.promise })));
		await settle();
		disk.anilist = account({ access_token: 'saved-elsewhere' });
		void accountStore.hydrate();
		await settle();
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		oauth.reject(new Error('ipc gone'));
		await running;
		await settle();
		const s = accountStore.byProvider.anilist;
		expect(s.kind === 'connected' && s.account.access_token).toBe('saved-elsewhere');
	});

	it('a persist that rejects after the write landed ends signed in, with no failure toast', async () => {
		await connectProvider(
			'anilist',
			deps(
				flow({
					persistAccount: async (p, payload) => {
						disk[p] = payload;
						throw new Error('reply lost');
					}
				})
			)
		);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('connected');
		expect(s.kind === 'connected' && s.account.access_token).toBe('new-bearer');
		expect(accountStore.accountChanging.anilist).toBe(false);
		expect(messages()).toEqual([]);
		expect(seeded).toEqual(['anilist new-bearer']);
	});

	it('a persist that rejects before writing ends signed out and says sign-in failed', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ persistAccount: () => Promise.reject(new Error('ipc gone')) }))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it('a sign-in that rejects from a keychain error puts the error back', async () => {
		accountStore.setError('anilist', 'Keychain read failed: decrypt_error');
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: () => Promise.reject(new Error('ipc gone')) }))
		);
		expect(accountStore.byProvider.anilist.kind).not.toBe('connecting');
		expect(accountStore.accountChanging.anilist).toBe(false);
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it('a keyring that cannot be read again keeps the restored sign-in', async () => {
		accountStore.setExpired('anilist', account());
		(globalThis as { window?: { aniGui?: unknown } }).window = {
			aniGui: { account: { getToken: async () => ({ ok: false, kind: 'decrypt_error' }) } }
		};
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: () => Promise.reject(new Error('ipc gone')) }))
		);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('expired');
		expect(s.kind === 'expired' && s.account.access_token).toBe('old-bearer');
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});
});

describe('cancelConnect', () => {
	it('a cancel that is delivered leaves the sign-in to settle as cancelled', async () => {
		const oauth = deferred<{ ok: false; kind: string }>();
		const running = connectProvider('anilist', deps(flow({ openOAuth: () => oauth.promise })));
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(async () => true)
		);
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		oauth.resolve({ ok: false, kind: 'cancelled' });
		await running;
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(messages()).toEqual([m.account_connect_error_cancelled()]);
	});

	it('a cancel that rejects still cancels: the provider goes back at once', async () => {
		const oauth = deferred<{ ok: false; kind: string }>();
		const running = connectProvider('anilist', deps(flow({ openOAuth: () => oauth.promise })));
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(accountStore.accountChanging.anilist).toBe(false);
		expect(messages()).toEqual([m.account_connect_error_cancelled()]);

		// The abandoned sign-in settling later says nothing more.
		oauth.resolve({ ok: false, kind: 'timeout' });
		await running;
		await settle();
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(messages()).toEqual([m.account_connect_error_cancelled()]);
	});

	it('a cancel that rejects restores an expired sign-in', async () => {
		accountStore.setExpired('anilist', account());
		const oauth = deferred<{ ok: false; kind: string }>();
		const running = connectProvider('anilist', deps(flow({ openOAuth: () => oauth.promise })));
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('expired');
		oauth.resolve({ ok: false, kind: 'cancelled' });
		await running;
	});

	it('an abandoned sign-in that completes in the browser anyway is not saved', async () => {
		const oauth = deferred<{ ok: true; code: string; state: string }>();
		const persist = vi.fn(async (p: Provider, payload: PersistedAccount) => {
			disk[p] = payload;
			return { ok: true as const };
		});
		const running = connectProvider(
			'anilist',
			deps(flow({ openOAuth: () => oauth.promise, persistAccount: persist }))
		);
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		oauth.resolve({ ok: true, code: 'code', state: 'st' });
		await running;
		await settle();
		expect(persist).not.toHaveBeenCalled();
		expect(disk.anilist).toBeUndefined();
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(messages()).toEqual([m.account_connect_error_cancelled()]);
	});

	it('an abandoned save already sent is read back once a newer sign-in settles', async () => {
		const save = deferred<{ ok: true }>();
		const firstRun = connectProvider(
			'anilist',
			deps(
				flow({
					persistAccount: async (p, payload) => {
						const r = await save.promise;
						disk[p] = payload;
						return r;
					}
				})
			)
		);
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		const second = deferred<{ ok: false; kind: string }>();
		const secondRun = connectProvider('anilist', deps(flow({ openOAuth: () => second.promise })));
		await settle();
		save.resolve({ ok: true });
		await firstRun;
		await settle();
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		second.resolve({ ok: false, kind: 'port_busy' });
		await secondRun;
		await settle();
		const s = accountStore.byProvider.anilist;
		expect(s.kind === 'connected' && s.account.access_token).toBe('new-bearer');
		expect(seeded).toEqual(['anilist new-bearer']);
	});

	it('an abandoned sign-in does not overwrite a newer one under way', async () => {
		const first = deferred<{ ok: false; kind: string }>();
		const firstRun = connectProvider('anilist', deps(flow({ openOAuth: () => first.promise })));
		await settle();
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		const second = deferred<{ ok: false; kind: string }>();
		const secondRun = connectProvider('anilist', deps(flow({ openOAuth: () => second.promise })));
		await settle();
		first.resolve({ ok: false, kind: 'cancelled' });
		await firstRun;
		await settle();
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
		second.resolve({ ok: false, kind: 'cancelled' });
		await secondRun;
	});

	it('a rejected cancel with no sign-in under way changes nothing', async () => {
		accountStore.setConnected('anilist', account());
		await cancelConnect(
			'anilist',
			cancelDeps(() => Promise.reject(new Error('ipc gone')))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('connected');
		expect(messages()).toEqual([]);
	});
});

describe('connectProvider: outcomes that answer', () => {
	it('a sign-in that succeeds connects and seeds the list', async () => {
		await connectProvider('anilist', deps(flow()));
		const s = accountStore.byProvider.anilist;
		expect(s.kind === 'connected' && s.account.access_token).toBe('new-bearer');
		expect(seeded).toEqual(['anilist new-bearer']);
		expect(messages()).toEqual([]);
	});

	it('a busy port says so', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: async () => ({ ok: false, kind: 'port_busy' }) }))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
		expect(messages()).toEqual([m.account_connect_error_port_busy()]);
	});

	it('a state mismatch reads as the provider declining', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: async () => ({ ok: true, code: 'c', state: 'other' }) }))
		);
		expect(messages()).toEqual([m.account_connect_error_oauth_error()]);
	});

	it.each([
		['timeout', () => m.account_connect_error_timeout()],
		['oauth_error', () => m.account_connect_error_oauth_error()],
		['no_bridge', () => m.account_connect_error_no_bridge()],
		['browser_launch_failed', () => m.account_connect_error_unknown()]
	])('an OAuth failure of kind %s says so', async (kind, message) => {
		await connectProvider('anilist', deps(flow({ openOAuth: async () => ({ ok: false, kind }) })));
		expect(messages()).toEqual([message()]);
	});

	it('a keyring that cannot encrypt puts the provider in error and points at the keyring', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ persistAccount: async () => ({ ok: false, kind: 'encryption_unavailable' }) }))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('error');
		expect(messages()).toEqual([m.account_persist_error_keychain_unavailable()]);
	});

	it('any other persist failure puts the provider in error with the generic message', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ persistAccount: async () => ({ ok: false, kind: 'io_error' }) }))
		);
		expect(accountStore.byProvider.anilist.kind).toBe('error');
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it('an API failure carries its status', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ exchangeCode: () => Promise.reject({ status: 502 }) }))
		);
		expect(messages()).toEqual([`${m.account_connect_error_unknown()} (502)`]);
	});

	it('an API failure without a status says sign-in failed', async () => {
		await connectProvider(
			'anilist',
			deps(flow({ fetchMe: () => Promise.reject(new Error('net')) }))
		);
		expect(messages()).toEqual([m.account_connect_error_unknown()]);
	});

	it('a failed reconnect from an error that carries a sign-in keeps it', async () => {
		accountStore.setExpired('anilist', account());
		accountStore.setError('anilist', 'earlier');
		await connectProvider(
			'anilist',
			deps(flow({ openOAuth: async () => ({ ok: false, kind: 'timeout' }) }))
		);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('error');
		expect(s.kind === 'error' && s.account?.access_token).toBe('old-bearer');
	});
});
