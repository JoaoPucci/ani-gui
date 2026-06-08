/**
 * Per-provider account state. Svelte 5 rune store; pattern matches
 * `lib/download/store.svelte.ts`.
 *
 * State is sourced from Electron's safeStorage on boot, mutated by
 * connect / disconnect / sync actions, and read by:
 *
 *   - /account page — surface state per provider
 *   - PR #2 home Watch Later rail — gate render on hasAny
 *   - PR #2 AccountChip — avatar + amber dot
 */

import type { PersistedAccount, Provider, ProviderState } from './types';
import { readPersistedAccount } from './api';

class AccountStore {
	byProvider = $state<Record<Provider, ProviderState>>({
		anilist: { kind: 'disconnected' },
		mal: { kind: 'disconnected' },
		inhouse: { kind: 'disconnected' }
	});

	/**
	 * Read every provider's persisted token (via Electron safeStorage)
	 * and seed the store. Called once at cold launch from
	 * `+layout.svelte`'s onMount.
	 *
	 * AniList 1-year JWT expiry is checked here: if `expires_at_epoch_s`
	 * is in the past, state is `expired` (renderer surfaces a Sign-in-
	 * again CTA rather than letting the next API call fail with 401).
	 */
	hydrate(): void {
		const providers: Provider[] = ['anilist', 'mal', 'inhouse'];
		const next = { ...this.byProvider };
		for (const p of providers) {
			const r = readPersistedAccount(p);
			if (r.ok) {
				next[p] = isExpired(r.account)
					? { kind: 'expired', account: r.account }
					: { kind: 'connected', account: r.account, lastSyncedAt: null };
				continue;
			}
			if (r.kind === 'not_found') {
				next[p] = { kind: 'disconnected' };
				continue;
			}
			// Codex P2 #3371530183: the token file is on disk but the
			// keychain is unreachable (libsecret/Keychain outage,
			// decrypt failure, basic_text reject from #3370070913).
			// Surface as error with no account; the page's error-no-
			// account branch now exposes Disconnect so the user can
			// call clearToken and remove the orphan file before
			// reconnecting. The message includes the underlying kind
			// (encryption_unavailable / decrypt_error / …) for the
			// diagnostics log; the page renders a friendlier copy.
			next[p] = {
				kind: 'error',
				account: null,
				message: `Keychain read failed: ${r.detail}`
			};
		}
		this.byProvider = next;
	}

	get connected(): Provider[] {
		return (Object.keys(this.byProvider) as Provider[]).filter(
			(p) => this.byProvider[p].kind === 'connected'
		);
	}

	get hasAny(): boolean {
		return this.connected.length > 0;
	}

	get hasErrored(): boolean {
		return (Object.values(this.byProvider) as ProviderState[]).some(
			(s) => s.kind === 'expired' || s.kind === 'error'
		);
	}

	setConnecting(provider: Provider): void {
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'connecting' } };
	}

	setConnected(provider: Provider, account: PersistedAccount): void {
		this.byProvider = {
			...this.byProvider,
			[provider]: { kind: 'connected', account, lastSyncedAt: Date.now() }
		};
	}

	setDisconnected(provider: Provider): void {
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'disconnected' } };
	}

	setExpired(provider: Provider, account: PersistedAccount): void {
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'expired', account } };
	}

	setError(provider: Provider, message: string): void {
		// Codex P2 #3370096597: preserve the account from prior
		// `error`-with-account too, not just connected / expired. If
		// the user is already in error-with-account and a Disconnect
		// attempt hits token_clear_failed, the bearer is still on disk
		// — dropping the account here would collapse the UI to bare
		// Connect even though the user needs Disconnect to retry the
		// clear. Pull the account from any prior state that carries
		// one.
		const prev = this.byProvider[provider];
		let account: PersistedAccount | null = null;
		if (prev.kind === 'connected' || prev.kind === 'expired') account = prev.account;
		else if (prev.kind === 'error') account = prev.account;
		this.byProvider = {
			...this.byProvider,
			[provider]: { kind: 'error', account, message }
		};
	}

	/** Bump the lastSyncedAt timestamp after a successful resync. */
	markSynced(provider: Provider): void {
		const prev = this.byProvider[provider];
		if (prev.kind !== 'connected') return;
		this.byProvider = {
			...this.byProvider,
			[provider]: { ...prev, lastSyncedAt: Date.now() }
		};
	}
}

function isExpired(account: PersistedAccount): boolean {
	if (account.expires_at_epoch_s <= 0) return false; // unknown → treat as valid
	const nowSec = Math.floor(Date.now() / 1000);
	return account.expires_at_epoch_s <= nowSec;
}

export const accountStore = new AccountStore();
