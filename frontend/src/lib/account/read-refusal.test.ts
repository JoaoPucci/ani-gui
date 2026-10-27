import { describe, expect, it, vi } from 'vitest';
import { AccountApiError } from './api';
import fc from 'fast-check';
import { handleRefusedBearer, refusalApplies, type RefusedBearerDeps } from './read-refusal';
import type { PersistedAccount, Provider, ProviderState } from './types';

function acc(refresh_token: string | null): PersistedAccount {
	return {
		access_token: 'old',
		refresh_token,
		expires_at_epoch_s: 4_000_000_000,
		user_id: '1',
		username: 'u',
		avatar_url: null
	};
}

function deps(state: ProviderState, over: Partial<RefusedBearerDeps> = {}): RefusedBearerDeps {
	return {
		state: () => state,
		changing: () => false,
		setConnected: vi.fn(),
		setExpired: vi.fn(),
		noteRenewalFailed: vi.fn(),
		refreshTokens: vi.fn().mockResolvedValue({
			access_token: 'new',
			refresh_token: 'rt2',
			expires_at_epoch_s: 4_000_000_100
		}),
		persistAccount: vi.fn().mockResolvedValue({ ok: true }),
		generation: () => 0,
		...over
	};
}

const refused = new AccountApiError(
	401,
	JSON.stringify({ kind: 'refresh_rejected', key: 'error.account.refresh_rejected' })
);
const down = new AccountApiError(
	502,
	JSON.stringify({ kind: 'upstream', key: 'error.network.upstream', status: 503 })
);

describe('handleRefusedBearer', () => {
	it('renews a renewable sign-in whose bearer was refused, and keeps it connected', async () => {
		const d = deps({ kind: 'connected', account: acc('rt'), lastSyncedAt: 0 });
		await handleRefusedBearer(d, 'mal', 'old');
		expect(d.refreshTokens).toHaveBeenCalledWith('mal', 'rt');
		expect(d.setConnected).toHaveBeenCalledWith(
			'mal',
			expect.objectContaining({ access_token: 'new' })
		);
		expect(d.setExpired).not.toHaveBeenCalled();
	});

	it.each([
		['refused', refused, 'rejected'],
		['not answered', down, 'unreachable']
	])(
		'marks the sign-in expired and says why when the renewal was %s',
		async (_n, error, failure) => {
			const account = acc('rt');
			const d = deps(
				{ kind: 'connected', account, lastSyncedAt: 0 },
				{ refreshTokens: vi.fn().mockRejectedValue(error) }
			);
			await handleRefusedBearer(d, 'mal', 'old');
			expect(d.setExpired).toHaveBeenCalledWith('mal', account);
			expect(d.noteRenewalFailed).toHaveBeenCalledWith('mal', failure);
		}
	);

	it('marks a sign-in with no way to renew it expired, and claims no renewal', async () => {
		// Nothing was renewed, so nothing was refused renewal: the notice
		// for an expired sign-in with no refresh token is the one to show.
		const account = acc(null);
		const d = deps({ kind: 'connected', account, lastSyncedAt: 0 });
		await handleRefusedBearer(d, 'anilist' as Provider, 'old');
		expect(d.refreshTokens).not.toHaveBeenCalled();
		expect(d.setExpired).toHaveBeenCalledWith('anilist', account);
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});

	it('leaves an already expired sign-in alone: its renewal has already been told', async () => {
		const d = deps({ kind: 'expired', account: acc('rt') });
		await handleRefusedBearer(d, 'mal', 'old');
		expect(d.refreshTokens).not.toHaveBeenCalled();
		expect(d.setExpired).not.toHaveBeenCalled();
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});

	it('drops the outcome when the account changed while renewing', async () => {
		let gen = 0;
		const d = deps(
			{ kind: 'connected', account: acc('rt'), lastSyncedAt: 0 },
			{
				generation: () => gen,
				refreshTokens: vi.fn().mockImplementation(async () => {
					gen = 1;
					throw refused;
				})
			}
		);
		await handleRefusedBearer(d, 'mal', 'old');
		// The account the read used is gone: marking it expired would
		// put a notice on a sign-in the user already changed.
		expect(d.setConnected).not.toHaveBeenCalled();
		expect(d.setExpired).not.toHaveBeenCalled();
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});
});

describe('a refusal belongs to the sign-in whose bearer was refused', () => {
	it('renews nothing while the account is being changed', async () => {
		// A 401 that arrives after a disconnect began would otherwise
		// renew the account being removed, and its save could land after
		// the disconnect's clear.
		const d = deps(
			{ kind: 'connected', account: acc('rt'), lastSyncedAt: 0 },
			{ changing: () => true }
		);
		await handleRefusedBearer(d, 'mal', 'old');
		expect(d.refreshTokens).not.toHaveBeenCalled();
		expect(d.setExpired).not.toHaveBeenCalled();
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});

	it('leaves a newer sign-in alone when the refused bearer was the old one', async () => {
		// The user reconnected while the read was out: the refusal is the
		// old sign-in's, not the one connected now.
		const d = deps({
			kind: 'connected',
			account: { ...acc('rt'), access_token: 'new' },
			lastSyncedAt: 0
		});
		await handleRefusedBearer(d, 'mal', 'old');
		expect(d.refreshTokens).not.toHaveBeenCalled();
		expect(d.setExpired).not.toHaveBeenCalled();
	});

	it('applies only to a connected sign-in holding that bearer, with no change under way', () => {
		const bearer = fc.constantFrom('old', 'new');
		const state = fc.oneof(
			fc.constant<ProviderState>({ kind: 'disconnected' }),
			fc.constant<ProviderState>({ kind: 'connecting' }),
			bearer.map<ProviderState>((t) => ({
				kind: 'expired',
				account: { ...acc('rt'), access_token: t }
			})),
			bearer.map<ProviderState>((t) => ({
				kind: 'connected',
				account: { ...acc('rt'), access_token: t },
				lastSyncedAt: 0
			})),
			fc.constant<ProviderState>({ kind: 'error', account: null, message: 'x' })
		);
		fc.assert(
			fc.property(state, fc.boolean(), bearer, (st, changing, refused) => {
				const expected =
					st.kind === 'connected' && !changing && st.account.access_token === refused;
				return refusalApplies(st, changing, refused) === expected;
			})
		);
	});
});
