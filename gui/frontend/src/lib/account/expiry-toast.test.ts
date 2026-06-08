import { describe, expect, it } from 'vitest';
import { detectExpiredProviders } from './expiry-toast';
import type { PersistedAccount, Provider, ProviderState } from './types';

function acc(over: Partial<PersistedAccount> = {}): PersistedAccount {
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

function build(over: Partial<Record<Provider, ProviderState>>): Record<Provider, ProviderState> {
	return {
		anilist: { kind: 'disconnected' },
		mal: { kind: 'disconnected' },
		inhouse: { kind: 'disconnected' },
		...over
	};
}

describe('detectExpiredProviders', () => {
	it('returns an empty list when every provider is disconnected', () => {
		expect(detectExpiredProviders(build({}))).toEqual([]);
	});

	it('returns an empty list when every connected session is still valid', () => {
		expect(
			detectExpiredProviders(
				build({
					anilist: { kind: 'connected', account: acc({ username: 'shiro' }), lastSyncedAt: 0 }
				})
			)
		).toEqual([]);
	});

	it('flags an expired AniList session with its username', () => {
		const out = detectExpiredProviders(
			build({ anilist: { kind: 'expired', account: acc({ username: 'shiro' }) } })
		);
		expect(out).toEqual([{ provider: 'anilist', username: 'shiro' }]);
	});

	it('returns expired providers in priority order (AniList → MAL → InHouse)', () => {
		const out = detectExpiredProviders(
			build({
				mal: { kind: 'expired', account: acc({ username: 'mal-user' }) },
				anilist: { kind: 'expired', account: acc({ username: 'al-user' }) }
			})
		);
		expect(out.map((e) => e.provider)).toEqual(['anilist', 'mal']);
	});

	it('does NOT flag an error-with-account state (different surface — chip dot + /account)', () => {
		expect(
			detectExpiredProviders(
				build({
					anilist: { kind: 'error', account: acc({ username: 'shiro' }), message: 'boom' }
				})
			)
		).toEqual([]);
	});

	it('does NOT flag connecting / disconnected providers', () => {
		expect(
			detectExpiredProviders(
				build({ anilist: { kind: 'connecting' }, mal: { kind: 'disconnected' } })
			)
		).toEqual([]);
	});
});
