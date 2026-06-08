import { describe, expect, it } from 'vitest';
import { chipDescriptor } from './chip-descriptor';
import type { PersistedAccount, Provider, ProviderState } from './types';

function disconnected(): ProviderState {
	return { kind: 'disconnected' };
}

function account(over: Partial<PersistedAccount> = {}): PersistedAccount {
	return {
		access_token: 't',
		refresh_token: null,
		expires_at_epoch_s: 0,
		user_id: '1',
		username: 'name',
		avatar_url: null,
		...over
	};
}

function connected(acc: PersistedAccount, lastSyncedAt: number | null = null): ProviderState {
	return { kind: 'connected', account: acc, lastSyncedAt };
}

function expired(acc: PersistedAccount): ProviderState {
	return { kind: 'expired', account: acc };
}

function errored(acc: PersistedAccount | null, message = 'boom'): ProviderState {
	return { kind: 'error', account: acc, message };
}

function build(over: Partial<Record<Provider, ProviderState>>): Record<Provider, ProviderState> {
	return {
		anilist: disconnected(),
		mal: disconnected(),
		inhouse: disconnected(),
		...over
	};
}

describe.skip('chipDescriptor', () => {
	it('hides the chip when no provider is connected', () => {
		expect(chipDescriptor(build({}))).toEqual({ kind: 'hidden' });
	});

	it('hides the chip when every provider is mid-connecting (no resolved identity yet)', () => {
		expect(chipDescriptor(build({ anilist: { kind: 'connecting' } }))).toEqual({ kind: 'hidden' });
	});

	it('surfaces a connected AniList account with no warning', () => {
		const out = chipDescriptor(
			build({
				anilist: connected(account({ username: 'shiro', avatar_url: 'https://x/a.png' }))
			})
		);
		expect(out).toEqual({
			kind: 'connected',
			provider: 'anilist',
			username: 'shiro',
			avatarUrl: 'https://x/a.png',
			warning: null
		});
	});

	it('prefers AniList over MAL when both are connected', () => {
		const out = chipDescriptor(
			build({
				mal: connected(account({ username: 'mal-name' })),
				anilist: connected(account({ username: 'al-name' }))
			})
		);
		expect(out).toMatchObject({ provider: 'anilist', username: 'al-name' });
	});

	it('flags expired sessions with a warning so the chip can render an amber dot', () => {
		const out = chipDescriptor(
			build({
				anilist: expired(account({ username: 'shiro' }))
			})
		);
		expect(out).toMatchObject({ kind: 'connected', warning: 'expired', username: 'shiro' });
	});

	it('flags transient errors with a warning when there is still a known account', () => {
		const out = chipDescriptor(
			build({
				anilist: errored(account({ username: 'shiro' }))
			})
		);
		expect(out).toMatchObject({ kind: 'connected', warning: 'error', username: 'shiro' });
	});

	it('hides the chip when an error has no surviving account (orphaned token wipe)', () => {
		expect(chipDescriptor(build({ anilist: errored(null) }))).toEqual({ kind: 'hidden' });
	});

	it('prefers a healthy AniList over an expired MAL', () => {
		const out = chipDescriptor(
			build({
				anilist: connected(account({ username: 'healthy' })),
				mal: expired(account({ username: 'stale' }))
			})
		);
		expect(out).toMatchObject({ provider: 'anilist', username: 'healthy', warning: null });
	});

	it('falls back to the MAL account when AniList is disconnected', () => {
		const out = chipDescriptor(
			build({
				mal: connected(account({ username: 'mal-user', avatar_url: null }))
			})
		);
		expect(out).toMatchObject({ provider: 'mal', username: 'mal-user', avatarUrl: null });
	});
});
