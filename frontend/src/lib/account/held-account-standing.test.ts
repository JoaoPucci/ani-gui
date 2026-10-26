import { afterEach, describe, expect, it } from 'vitest';

import { accountStanding } from './held-account';
import { accountStore } from './store.svelte';

const account = (user_id: string) => ({
	access_token: 't',
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id,
	username: 'u',
	avatar_url: null
});

afterEach(() => {
	accountStore.setDisconnected('anilist');
});

describe('accountStanding', () => {
	it('is same for the listed user, other for another, gone without an account to compare', () => {
		accountStore.setConnected('anilist', account('u1'));
		expect(accountStanding('anilist', { user: 'u1' })).toBe('same');
		expect(accountStanding('anilist', { user: 'u0' })).toBe('other');
		expect(accountStanding('anilist', { user: null })).toBe('gone');
		accountStore.setDisconnected('anilist');
		expect(accountStanding('anilist', { user: 'u1' })).toBe('gone');
	});
});
