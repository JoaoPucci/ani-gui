import { describe, expect, it, vi } from 'vitest';
import { handleChipDisconnect } from './chip-disconnect';
import type { ProviderState } from './types';

function makeState(): ProviderState {
	return { kind: 'disconnected' };
}

describe.skip('handleChipDisconnect', () => {
	it('calls disconnectAccount with the persisted-account clear deps', async () => {
		const disconnectAccount = vi.fn().mockResolvedValue({ kind: 'ok' });
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'unknown',
			tokenClearFailedMessage: () => 'token clear failed'
		};
		const deps = {
			disconnectAccount,
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		await handleChipDisconnect('anilist', makeState(), deps, cb);
		expect(disconnectAccount).toHaveBeenCalledWith('anilist', expect.anything(), {
			clearPersistedAccount: deps.clearPersistedAccount,
			dropListCache: deps.dropListCache,
			dropProviderCache: deps.dropProviderCache
		});
	});

	it('happy path → setDisconnected, no error / no toast', async () => {
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'unknown',
			tokenClearFailedMessage: () => 'token clear failed'
		};
		const deps = {
			disconnectAccount: vi.fn().mockResolvedValue({ kind: 'ok' }),
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		await handleChipDisconnect('anilist', makeState(), deps, cb);
		expect(cb.setDisconnected).toHaveBeenCalledWith('anilist');
		expect(cb.setError).not.toHaveBeenCalled();
		expect(cb.pushToast).not.toHaveBeenCalled();
	});

	it('token_clear_failed → setError + error toast, NO setDisconnected', async () => {
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'unknown',
			tokenClearFailedMessage: () => 'token clear failed'
		};
		const deps = {
			disconnectAccount: vi.fn().mockResolvedValue({ kind: 'token_clear_failed' }),
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		await handleChipDisconnect('anilist', makeState(), deps, cb);
		expect(cb.setError).toHaveBeenCalledWith('anilist', 'unknown');
		expect(cb.pushToast).toHaveBeenCalledWith({ kind: 'error', message: 'token clear failed' });
		expect(cb.setDisconnected).not.toHaveBeenCalled();
	});

	it('passes through the previous ProviderState so disconnectAccount can restore on failure', async () => {
		const prev: ProviderState = {
			kind: 'expired',
			account: {
				access_token: 't',
				refresh_token: null,
				expires_at_epoch_s: 0,
				user_id: 'u',
				username: 'name',
				avatar_url: null
			}
		};
		const disconnectAccount = vi.fn().mockResolvedValue({ kind: 'ok' });
		const deps = {
			disconnectAccount,
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'u',
			tokenClearFailedMessage: () => 'tcf'
		};
		await handleChipDisconnect('mal', prev, deps, cb);
		expect(disconnectAccount.mock.calls[0]?.[1]).toBe(prev);
	});
});
