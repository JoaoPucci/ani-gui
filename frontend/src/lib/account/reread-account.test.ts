/**
 * `accountStore.reread(provider)`: one provider's saved sign-in read
 * again from the keyring and applied, the way the hydrate applies its
 * reads. Used to put a provider back on what the disk holds after an
 * account action whose bridge call rejected, when the app cannot know
 * whether the write or clear behind it landed.
 */

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { accountStore } from './store.svelte';
import type { PersistedAccount } from './types';

function account(overrides: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 'on-disk',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: '7',
		username: 'shiro',
		avatar_url: null,
		...overrides
	};
}

let reads = 0;
let answer: (r: unknown) => void = () => {};

/** A keyring whose reads wait for the test to answer them. */
function slowKeyring() {
	reads = 0;
	(globalThis as { window?: { aniGui?: unknown } }).window = {
		aniGui: {
			account: {
				getToken: () => {
					reads += 1;
					return new Promise((resolve) => (answer = resolve));
				}
			}
		}
	};
}

const flush = async () => {
	for (let i = 0; i < 5; i++) await Promise.resolve();
};

beforeEach(() => {
	for (const p of ['anilist', 'mal', 'inhouse'] as const) accountStore.setDisconnected(p);
	slowKeyring();
});

afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
});

describe('accountStore.reread', () => {
	it('applies what the disk holds and says it did', async () => {
		const done = accountStore.reread('anilist');
		answer({ ok: true, payload: account() });
		expect(await done).toBe(true);
		const s = accountStore.byProvider.anilist;
		expect(s.kind).toBe('connected');
		expect(s.kind === 'connected' && s.account.access_token).toBe('on-disk');
	});

	it('applies a disk that holds nothing as signed out', async () => {
		accountStore.setError('anilist', 'clear failed');
		const done = accountStore.reread('anilist');
		answer({ ok: false, kind: 'not_found' });
		expect(await done).toBe(true);
		expect(accountStore.byProvider.anilist.kind).toBe('disconnected');
	});

	it('leaves a sign-in under way alone, without asking the keyring', async () => {
		accountStore.setConnecting('anilist');
		expect(await accountStore.reread('anilist')).toBe(false);
		expect(reads).toBe(0);
		expect(accountStore.byProvider.anilist.kind).toBe('connecting');
	});

	it('leaves an account change under way alone, without asking the keyring', async () => {
		accountStore.setConnected('anilist', account({ access_token: 'held' }));
		accountStore.beginAccountChange('anilist');
		expect(await accountStore.reread('anilist')).toBe(false);
		expect(reads).toBe(0);
		const s = accountStore.byProvider.anilist;
		expect(s.kind === 'connected' && s.account.access_token).toBe('held');
	});

	it('drops a read that a change overtook while it waited', async () => {
		const done = accountStore.reread('anilist');
		await flush();
		accountStore.setConnected('anilist', account({ access_token: 'newer' }));
		answer({ ok: false, kind: 'not_found' });
		expect(await done).toBe(false);
		const s = accountStore.byProvider.anilist;
		expect(s.kind === 'connected' && s.account.access_token).toBe('newer');
	});

	it('touches no other provider', async () => {
		accountStore.setConnected('mal', account({ access_token: 'mal' }));
		const before = accountStore.byProvider.mal;
		const done = accountStore.reread('anilist');
		answer({ ok: false, kind: 'not_found' });
		await done;
		expect(accountStore.byProvider.mal).toBe(before);
	});
});
