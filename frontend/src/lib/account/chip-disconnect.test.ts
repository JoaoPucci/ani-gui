import { describe, expect, it, vi } from 'vitest';
import { handleChipDisconnect } from './chip-disconnect';
import type { ProviderState } from './types';

function makeState(): ProviderState {
	return { kind: 'disconnected' };
}

describe('handleChipDisconnect', () => {
	it('calls disconnectAccount with the persisted-account clear deps', async () => {
		const disconnectAccount = vi.fn().mockResolvedValue({ kind: 'ok' });
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'unknown',
			tokenClearFailedMessage: () => 'token clear failed',
			reread: vi.fn().mockResolvedValue(true),
			stateOf: (): ProviderState => ({ kind: 'disconnected' })
		};
		const deps = {
			disconnectAccount,
			beginAccountChange: vi.fn(),
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		await handleChipDisconnect('anilist', makeState(), deps, cb);
		expect(disconnectAccount).toHaveBeenCalledWith('anilist', expect.anything(), {
			beginAccountChange: deps.beginAccountChange,
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
			tokenClearFailedMessage: () => 'token clear failed',
			reread: vi.fn().mockResolvedValue(true),
			stateOf: (): ProviderState => ({ kind: 'disconnected' })
		};
		const deps = {
			disconnectAccount: vi.fn().mockResolvedValue({ kind: 'ok' }),
			beginAccountChange: vi.fn(),
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
			tokenClearFailedMessage: () => 'token clear failed',
			reread: vi.fn().mockResolvedValue(true),
			stateOf: (): ProviderState => ({ kind: 'disconnected' })
		};
		const deps = {
			disconnectAccount: vi.fn().mockResolvedValue({ kind: 'token_clear_failed' }),
			beginAccountChange: vi.fn(),
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
			beginAccountChange: vi.fn(),
			clearPersistedAccount: vi.fn(),
			dropListCache: vi.fn(),
			dropProviderCache: vi.fn()
		};
		const cb = {
			setError: vi.fn(),
			setDisconnected: vi.fn(),
			pushToast: vi.fn(),
			unknownErrorMessage: () => 'u',
			tokenClearFailedMessage: () => 'tcf',
			reread: vi.fn().mockResolvedValue(true),
			stateOf: (): ProviderState => ({ kind: 'disconnected' })
		};
		await handleChipDisconnect('mal', prev, deps, cb);
		expect(disconnectAccount.mock.calls[0]?.[1]).toBe(prev);
	});

	describe('a disconnect whose bridge call rejects', () => {
		function rejecting(stateAfterReread: ProviderState, applied = true) {
			const order: string[] = [];
			const cb = {
				setError: vi.fn((p: string, msg: string) => order.push(`setError ${p} ${msg}`)),
				setDisconnected: vi.fn(),
				pushToast: vi.fn((t: { message: string }) => order.push(`toast ${t.message}`)),
				unknownErrorMessage: () => 'unknown',
				tokenClearFailedMessage: () => 'token clear failed',
				reread: vi.fn(async (p: string) => {
					order.push(`reread ${p}`);
					return applied;
				}),
				stateOf: () => stateAfterReread
			};
			const deps = {
				disconnectAccount: vi.fn().mockRejectedValue(new Error('ipc gone')),
				beginAccountChange: vi.fn(),
				clearPersistedAccount: vi.fn(),
				dropListCache: vi.fn(),
				dropProviderCache: vi.fn()
			};
			return { cb, deps, order };
		}
		const signedIn: ProviderState = {
			kind: 'connected',
			account: {
				access_token: 't',
				refresh_token: null,
				expires_at_epoch_s: 0,
				user_id: 'u',
				username: 'name',
				avatar_url: null
			},
			lastSyncedAt: null
		};

		it('settles the change at once, then reads the disk, and says the clear failed when the sign-in is still there', async () => {
			const { cb, deps, order } = rejecting(signedIn);
			await expect(handleChipDisconnect('anilist', signedIn, deps, cb)).resolves.toBeUndefined();
			expect(order).toEqual([
				'setError anilist unknown',
				'reread anilist',
				'toast token clear failed'
			]);
			expect(cb.pushToast).toHaveBeenCalledWith({ kind: 'error', message: 'token clear failed' });
			expect(cb.setDisconnected).not.toHaveBeenCalled();
		});

		it('says nothing failed when the disk shows the clear landed', async () => {
			const { cb, deps } = rejecting({ kind: 'disconnected' });
			await handleChipDisconnect('anilist', signedIn, deps, cb);
			expect(cb.setError).toHaveBeenCalled();
			expect(cb.pushToast).not.toHaveBeenCalled();
		});

		it('says the clear failed when the disk could not be read again', async () => {
			const { cb, deps } = rejecting({ kind: 'disconnected' }, false);
			await handleChipDisconnect('anilist', signedIn, deps, cb);
			expect(cb.pushToast).toHaveBeenCalledWith({ kind: 'error', message: 'token clear failed' });
		});
	});
});
