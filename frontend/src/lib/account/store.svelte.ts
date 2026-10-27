/**
 * Per-provider account state. Svelte 5 rune store; pattern matches
 * `lib/download/store.svelte.ts`.
 *
 * State is sourced from Electron's safeStorage on boot, mutated by
 * connect / disconnect / sync actions, and read by:
 *
 *   - /account page — surface state per provider
 *   - home Watch Later rail — gate render on hasAny
 *   - AccountChip — avatar + amber dot
 */

import type { PersistedAccount, Provider, ProviderState } from './types';
import { persistAccount, readPersistedAccount, refreshTokens } from './api';
import { refreshExpiredAccounts } from './refresh-flow';
import { beginOwnWrite, ownWrites } from './own-writes';
import { NO_RENEWAL_FAILURES, type RenewalFailure, type RenewalFailures } from './renewal-failure';
import {
	failuresAfterHydrate,
	failuresAfterRetry,
	hydratedState,
	isSignInChange,
	withFailure
} from './hydrated-state';

class AccountStore {
	byProvider = $state<Record<Provider, ProviderState>>({
		anilist: { kind: 'disconnected' },
		mal: { kind: 'disconnected' },
		inhouse: { kind: 'disconnected' }
	});

	/**
	 * Per provider, what the last failed renewal said about the sign-in,
	 * until the sign-in changes. Read by the sign-in notice.
	 */
	renewalFailure = $state<RenewalFailures>({ ...NO_RENEWAL_FAILURES });

	/** Record a failed renewal. */
	noteRenewalFailed(provider: Provider, failure: RenewalFailure): void {
		this.renewalFailure = withFailure(this.renewalFailure, provider, failure);
	}

	/** Forget a provider's renewal failure: its sign-in changed. */
	private clearRenewalFailure(provider: Provider): void {
		this.renewalFailure = withFailure(this.renewalFailure, provider, null);
	}

	/**
	 * Monotonic per-provider counter, bumped on every account state
	 * change and synchronously at the start of an async disconnect
	 * ([`beginAccountChange`]). A boot-time refresh captures it before
	 * its network await and re-checks after, so a disconnect / re-auth
	 * that raced the refresh supersedes the write even before the
	 * `byProvider` snapshot updates.
	 */
	accountGeneration: Record<Provider, number> = { anilist: 0, mal: 0, inhouse: 0 };

	/**
	 * True between `beginAccountChange(provider)` and the next resolved
	 * state transition for that provider. A disconnect bumps the
	 * generation up front but leaves `byProvider` as `connected` until its
	 * async clear finishes; a refresh entering that window would see a
	 * stable (already-bumped) generation and pass its checks, so the
	 * generation counter alone can't gate it. Consumers (fresh-bearer)
	 * treat a changing provider as non-refreshable so a post-disconnect
	 * refresh can't enqueue a resurrecting write.
	 */
	accountChanging: Record<Provider, boolean> = { anilist: false, mal: false, inhouse: false };

	private bumpGeneration(provider: Provider): void {
		this.accountGeneration = {
			...this.accountGeneration,
			[provider]: this.accountGeneration[provider] + 1
		};
	}

	/** Mark / clear the in-progress-change flag for a provider. */
	private setChanging(provider: Provider, changing: boolean): void {
		this.accountChanging = { ...this.accountChanging, [provider]: changing };
	}

	/**
	 * Signal that an async account mutation (e.g. disconnect) is starting
	 * for `provider`, so an in-flight token refresh is superseded before
	 * the mutation's async clear updates `byProvider`. Also marks the provider
	 * as changing until the next resolved state transition, so a refresh
	 * entering the disconnect window (byProvider still `connected`) is held
	 * off. Synchronous and side-effect-free beyond those flags.
	 */
	beginAccountChange(provider: Provider): void {
		this.bumpGeneration(provider);
		this.setChanging(provider, true);
	}

	/**
	 * Read every provider's persisted token (via Electron safeStorage)
	 * and seed the store. Called at cold launch from `+layout.svelte`'s
	 * onMount, and again when the account page mounts. A provider whose
	 * re-read holds a different sign-in moves its generation, so a
	 * renewal still out for the old one is superseded.
	 *
	 * AniList 1-year JWT expiry is checked here: if `expires_at_epoch_s`
	 * is in the past, state is `expired` (renderer surfaces a Sign-in-
	 * again CTA rather than letting the next API call fail with 401).
	 */
	hydrate(): void {
		const providers: Provider[] = ['anilist', 'mal', 'inhouse'];
		const prev = this.byProvider;
		const next = { ...prev };
		const nowSec = Math.floor(Date.now() / 1000);
		for (const p of providers) {
			next[p] = hydratedState(prev[p], readPersistedAccount(p), nowSec);
			if (isSignInChange(prev[p], next[p], ownWrites(p))) this.bumpGeneration(p);
		}
		this.renewalFailure = failuresAfterHydrate(prev, next, this.renewalFailure);
		this.byProvider = next;
		// Every hydrate path (cold launch AND the account page) must give a
		// just-expired-but-refreshable provider a chance to refresh, so
		// hydrate() owns it rather than each caller remembering. Fire-and-forget — safe to ignore.
		void this.refreshExpired();
	}

	/**
	 * Refresh any provider `hydrate()` marked `expired` that still has a
	 * usable refresh token (MAL's ~1h access token), re-persisting the
	 * rotated tokens and flipping it back to `connected`. AniList carries
	 * no refresh token, so it stays expired → reauth. Best-effort and
	 * safe to `void` from the layout's onMount after hydrate.
	 */
	refreshExpired(): Promise<void> {
		return refreshExpiredAccounts({
			byProvider: () => this.byProvider,
			onRefreshed: (provider, account) => this.setConnected(provider, account),
			refreshTokens,
			persistAccount,
			// Post-await staleness guard: the per-provider generation counter
			// advances on any account change (and synchronously at the start
			// of an async disconnect), so a refresh that resolved after a
			// mid-flight disconnect / re-auth is dropped — even before the
			// store snapshot catches up.
			generation: (provider) => this.accountGeneration[provider],
			onRenewalFailed: (provider, failure) => this.noteRenewalFailed(provider, failure),
			ownWrite: beginOwnWrite,
			// Don't even start an expired refresh for a provider whose
			// disconnect is mid-flight.
			changing: (provider) => this.accountChanging[provider]
		});
	}

	/**
	 * Try the failed renewals of expired sign-ins again now, at the
	 * user's request. Their failures are forgotten first: the renewal is
	 * pending again, so its notice is withdrawn, and whatever it answers
	 * is shown as new rather than taken for the incident already seen.
	 */
	retryRenewal(): Promise<void> {
		this.renewalFailure = failuresAfterRetry(this.byProvider, this.renewalFailure);
		return this.refreshExpired();
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
		this.bumpGeneration(provider);
		this.setChanging(provider, false);
		// The failure stays while the new sign-in is under way: a
		// cancelled or failed connect restores the expired sign-in, and
		// its notice with it. A sign-in that succeeds clears it.
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'connecting' } };
	}

	setConnected(provider: Provider, account: PersistedAccount): void {
		this.bumpGeneration(provider);
		this.setChanging(provider, false);
		this.clearRenewalFailure(provider);
		this.byProvider = {
			...this.byProvider,
			[provider]: { kind: 'connected', account, lastSyncedAt: Date.now() }
		};
	}

	setDisconnected(provider: Provider): void {
		this.bumpGeneration(provider);
		this.setChanging(provider, false);
		this.clearRenewalFailure(provider);
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'disconnected' } };
	}

	setExpired(provider: Provider, account: PersistedAccount): void {
		this.bumpGeneration(provider);
		this.setChanging(provider, false);
		this.byProvider = { ...this.byProvider, [provider]: { kind: 'expired', account } };
	}

	setError(provider: Provider, message: string): void {
		this.setChanging(provider, false);
		// Preserve the account from prior
		// `error`-with-account too, not just connected / expired. If
		// the user is already in error-with-account and a Disconnect
		// attempt hits token_clear_failed, the bearer is still on disk
		// — dropping the account here would collapse the UI to bare
		// Connect even though the user needs Disconnect to retry the
		// clear. Pull the account from any prior state that carries
		// one.
		const prev = this.byProvider[provider];
		const account: PersistedAccount | null = 'account' in prev ? prev.account : null;
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

export const accountStore = new AccountStore();
