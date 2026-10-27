import { describe, expect, it, vi } from 'vitest';
import { AccountApiError } from './api';
import { handleRefusedBearer, type RefusedBearerDeps } from './read-refusal';
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
		await handleRefusedBearer(d, 'mal');
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
			await handleRefusedBearer(d, 'mal');
			expect(d.setExpired).toHaveBeenCalledWith('mal', account);
			expect(d.noteRenewalFailed).toHaveBeenCalledWith('mal', failure);
		}
	);

	it('marks a sign-in with no way to renew it expired, and claims no renewal', async () => {
		// Nothing was renewed, so nothing was refused renewal: the notice
		// for an expired sign-in with no refresh token is the one to show.
		const account = acc(null);
		const d = deps({ kind: 'connected', account, lastSyncedAt: 0 });
		await handleRefusedBearer(d, 'anilist' as Provider);
		expect(d.refreshTokens).not.toHaveBeenCalled();
		expect(d.setExpired).toHaveBeenCalledWith('anilist', account);
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});

	it('leaves an already expired sign-in alone: its renewal has already been told', async () => {
		const d = deps({ kind: 'expired', account: acc('rt') });
		await handleRefusedBearer(d, 'mal');
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
		await handleRefusedBearer(d, 'mal');
		// The account the read used is gone: marking it expired would
		// put a notice on a sign-in the user already changed.
		expect(d.setConnected).not.toHaveBeenCalled();
		expect(d.setExpired).not.toHaveBeenCalled();
		expect(d.noteRenewalFailed).not.toHaveBeenCalled();
	});
});
