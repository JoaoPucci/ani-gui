<!--
  Accounts management surface. Reached from Settings → Accounts or
  the eventual topbar chip (lands in PR #2 alongside Watch Later).

  Single AniList card today; MAL lands in PR #3 (placeholder reserved
  here so the layout is stable when it ships).

  All visible strings via Paraglide per AGENTS.md §6. Imperative
  connect-flow logic delegated to $lib/account/connect-flow to keep
  the page itself a thin adapter — per AGENTS.md §2.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { m } from '$lib/paraglide/messages';
	import { imageProxyUrl } from '$lib/api';
	import { accountStore } from '$lib/account/store.svelte';
	import { Pkce } from '$lib/account/pkce';
	import {
		buildAuthUrl,
		cancelOAuth,
		exchangeCode,
		fetchAndCacheList,
		fetchMe,
		openOAuth,
		openExternal,
		persistAccount,
		clearPersistedAccount,
		dropListCache,
		dropProviderCache
	} from '$lib/account/api';
	import {
		bearerFor,
		connectAccount,
		connectErrorKey,
		disconnectAccount,
		restoreAfterFailedConnect
	} from '$lib/account/connect-flow';
	import type { Provider, ProviderState } from '$lib/account/types';
	import { toastStore } from '$lib/toasts/store.svelte';

	const PRIVACY_URL = 'https://github.com/JoaoPucci/ani-gui/blob/master/docs/PRIVACY.md';

	onMount(() => {
		accountStore.hydrate();
	});

	async function connectAniList() {
		const provider: Provider = 'anilist';
		// Snapshot the pre-click state so a failed reconnect-from-
		// expired (or connect-from-error-with-account) can restore the
		// UI to it instead of collapsing to `disconnected` — the
		// persisted token is still on disk and hydrate() would resurrect
		// it on next launch anyway (Codex P2 #3370011851).
		const prev = accountStore.byProvider[provider];
		accountStore.setConnecting(provider);
		const r = await connectAccount(provider, {
			generateState: randomState,
			generatePkce: () => {
				const p = Pkce.s256();
				return { verifier: p.verifier, challenge: p.challenge, method: p.method };
			},
			buildAuthUrl,
			openOAuth,
			exchangeCode,
			fetchMe,
			persistAccount
		});
		if (r.kind === 'connected') {
			accountStore.setConnected(provider, r.account);
			// Codex P2 #3370057922: seed user_list_cache for the rails +
			// account stats that read from it (Watch Later in PR #2,
			// progress/score in PR #4). The bearer is good and the token
			// is already persisted; if this errors the connection is
			// still valid — we just leave the cache empty and the next
			// rail/sync mount will warm it. Best-effort, not fatal.
			try {
				await fetchAndCacheList(provider, r.account.access_token);
			} catch {
				/* non-fatal — next mount warms the cache */
			}
			return;
		}
		applyRestoreState(provider, restoreAfterFailedConnect(prev));
		if (r.kind === 'oauth_error' || r.kind === 'state_mismatch') {
			const key =
				r.kind === 'state_mismatch'
					? 'oauth_error'
					: connectErrorKey((r as { reason: string }).reason);
			toastStore.push({ kind: 'error', message: connectErrorMessageFor(key) });
			return;
		}
		if (r.kind === 'persist_failed') {
			accountStore.setError(provider, m.account_connect_error_unknown());
			return;
		}
		const status = (r as { status?: number }).status;
		const message = status
			? `${m.account_connect_error_unknown()} (${status})`
			: m.account_connect_error_unknown();
		toastStore.push({ kind: 'error', message });
	}

	function applyRestoreState(provider: Provider, restored: ProviderState): void {
		// Thin dispatch from the restore helper to the matching store
		// setter. Kept here (one switch) rather than inside the store
		// so the helper stays a pure data->data function the tests can
		// pin without mocking the store.
		switch (restored.kind) {
			case 'expired':
				accountStore.setExpired(provider, restored.account);
				return;
			case 'error':
				if (restored.account) {
					// setError pulls the account from the prior state in
					// the store — set it back as expired-with-account
					// first so setError finds it, then promote to error.
					accountStore.setExpired(provider, restored.account);
				}
				accountStore.setError(provider, restored.message);
				return;
			default:
				accountStore.setDisconnected(provider);
		}
	}

	async function cancelConnect() {
		// Codex P2 #3370087029: without this the `connecting` state
		// only resolves after openOAuth's 5-minute server timeout — the
		// user can't recover until then. cancelOAuth stops the
		// loopback server immediately; the awaiting connectAccount
		// then resolves as oauth_error with kind === 'cancelled',
		// and the prior `disconnected` state is restored via the
		// existing restoreAfterFailedConnect path. (The OAuth server
		// is process-singleton across providers, so we don't need a
		// provider arg — MAL in PR #3 reuses the same server.)
		await cancelOAuth();
	}

	async function disconnect(provider: Provider) {
		const prev = accountStore.byProvider[provider];
		const r = await disconnectAccount(provider, prev, {
			clearPersistedAccount,
			dropListCache,
			dropProviderCache
		});
		if (r.kind === 'token_clear_failed') {
			// Codex P2 #3369988183: the bearer is still on disk; telling
			// the user they're disconnected would be a lie because the
			// next hydrate() restores the account. Leave state as-is and
			// surface the failure as an error.
			accountStore.setError(provider, m.account_connect_error_unknown());
			toastStore.push({ kind: 'error', message: m.account_disconnect_error_token_clear() });
			return;
		}
		accountStore.setDisconnected(provider);
	}

	function connectErrorMessageFor(key: string): string {
		switch (key) {
			case 'port_busy':
				return m.account_connect_error_port_busy();
			case 'timeout':
				return m.account_connect_error_timeout();
			case 'cancelled':
				return m.account_connect_error_cancelled();
			case 'oauth_error':
				return m.account_connect_error_oauth_error();
			case 'no_bridge':
				return m.account_connect_error_no_bridge();
			default:
				return m.account_connect_error_unknown();
		}
	}

	function randomState(): string {
		// CSRF state — 32 bytes of base64url. Doesn't need to be
		// crypto-grade unique across the universe; just unguessable
		// within the 5-min OAuth window.
		const bytes = crypto.getRandomValues(new Uint8Array(32));
		return btoa(String.fromCharCode(...bytes))
			.replace(/\+/g, '-')
			.replace(/\//g, '_')
			.replace(/=+$/, '');
	}

	function stateBadgeKind(state: ProviderState): string {
		switch (state.kind) {
			case 'connected':
				return m.account_card_status_connected();
			case 'connecting':
				return m.account_card_status_connecting();
			case 'expired':
				return m.account_card_status_expired();
			case 'error':
				return m.account_card_status_error();
			default:
				return m.account_card_status_disconnected();
		}
	}
	// Used by template, kept exported so connect-flow tests can pin
	// the helper without test-only exports leaking from +page.svelte.
	void bearerFor;
</script>

<svelte:head>
	<title>{m.account_eyebrow_key()}</title>
</svelte:head>

<main class="account-page">
	<header class="page-head">
		<p class="eyebrow">{m.account_eyebrow_key()}</p>
		<h1 class="display">{m.account_title()}</h1>
		<p class="subtitle">{m.account_subtitle()}</p>
	</header>

	<!-- AniList -->
	{#if true}
		{@const anilistState = accountStore.byProvider.anilist}
		<section class="provider-card" data-state={anilistState.kind}>
			<header class="provider-head">
				<h2 class="provider-name">{m.account_provider_anilist()}</h2>
				<span class="state-badge state-badge-{anilistState.kind}">
					{stateBadgeKind(anilistState)}
				</span>
			</header>

			{#if anilistState.kind === 'connected' || anilistState.kind === 'expired' || (anilistState.kind === 'error' && anilistState.account)}
				<div class="connected-row">
					{#if anilistState.account && imageProxyUrl(anilistState.account.avatar_url)}
						<img
							class="avatar"
							src={imageProxyUrl(anilistState.account.avatar_url)}
							alt=""
							width="48"
							height="48"
						/>
					{/if}
					<div class="user-meta">
						<p class="username">
							<span class="username-prefix">{m.account_card_username_prefix()}</span>
							<strong>{anilistState.account?.username}</strong>
						</p>
					</div>
				</div>
			{/if}

			<div class="actions">
				{#if anilistState.kind === 'disconnected'}
					<button type="button" class="btn btn-primary" onclick={connectAniList}>
						{m.account_card_action_connect()}
					</button>
				{:else if anilistState.kind === 'error' && !anilistState.account}
					<!-- Codex P2 #3371530183: error-with-no-account covers the
					     orphan-file case where hydrate() couldn't read the
					     keychain. Offer Disconnect so the user can call
					     clearToken and clean up before reconnecting; the
					     Connect button stays so they can try again now if
					     it was a transient failure. -->
					<button type="button" class="btn btn-primary" onclick={connectAniList}>
						{m.account_card_action_connect()}
					</button>
					<button type="button" class="btn" onclick={() => disconnect('anilist')}>
						{m.account_card_action_disconnect()}
					</button>
				{:else if anilistState.kind === 'connecting'}
					<button type="button" class="btn" disabled>
						{m.account_card_status_connecting()}
					</button>
					<button type="button" class="btn" onclick={cancelConnect}>
						{m.account_card_action_cancel()}
					</button>
				{:else if anilistState.kind === 'expired' || (anilistState.kind === 'error' && anilistState.account)}
					<button type="button" class="btn btn-primary" onclick={connectAniList}>
						{m.account_card_action_reconnect()}
					</button>
					<button type="button" class="btn" onclick={() => disconnect('anilist')}>
						{m.account_card_action_disconnect()}
					</button>
				{:else}
					<button type="button" class="btn" onclick={() => disconnect('anilist')}>
						{m.account_card_action_disconnect()}
					</button>
				{/if}
			</div>
		</section>
	{/if}

	<!-- MAL: placeholder for PR #3 -->
	<section class="provider-card provider-disabled">
		<header class="provider-head">
			<h2 class="provider-name">{m.account_provider_mal()}</h2>
			<span class="state-badge state-badge-coming-soon">
				{m.account_provider_mal_coming_soon_label()}
			</span>
		</header>
		<p class="provider-disabled-hint">{m.account_provider_mal_coming_soon_hint()}</p>
	</section>

	<footer class="page-foot">
		<p class="privacy-line">
			{m.account_privacy_consent_line()}
			<button type="button" class="inline-link" onclick={() => openExternal(PRIVACY_URL)}>
				{m.account_privacy_link_label()}
			</button>.
		</p>
		<a class="back-link" href={resolve('/settings')}>← {m.account_eyebrow_key()}</a>
	</footer>
</main>

<style>
	.account-page {
		padding-block: var(--space-7);
		padding-inline: clamp(var(--space-4), 4vw, var(--space-7));
		max-inline-size: 56rem;
		margin-inline: auto;
		color: var(--ink-100);
	}

	.page-head {
		margin-block-end: var(--space-7);
	}

	.eyebrow {
		font-family: var(--font-display);
		font-size: 0.875rem;
		font-weight: 500;
		letter-spacing: 0.16em;
		text-transform: uppercase;
		color: var(--ink-400);
		margin: 0 0 var(--space-2);
	}

	.display {
		font-family: var(--font-display);
		font-size: clamp(1.875rem, 4vw, 2.5rem);
		font-weight: 600;
		line-height: 1.1;
		letter-spacing: -0.01em;
		margin: 0 0 var(--space-3);
	}

	.subtitle {
		font-family: var(--font-body);
		font-size: 1rem;
		line-height: 1.6;
		color: var(--ink-300);
		max-inline-size: 38rem;
		margin: 0;
	}

	.provider-card {
		background: var(--surface-1);
		border: 1px solid var(--border-1);
		border-radius: var(--radius-2);
		padding: var(--space-6);
		margin-block-end: var(--space-5);
	}

	.provider-card[data-state='expired'] {
		border-color: var(--accent-amber, #c89a48);
	}

	.provider-card[data-state='error'] {
		border-color: var(--accent-oxblood, #c44);
	}

	.provider-card.provider-disabled {
		opacity: 0.55;
	}

	.provider-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		margin-block-end: var(--space-4);
	}

	.provider-name {
		font-family: var(--font-display);
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0;
	}

	.state-badge {
		font-family: var(--font-display);
		font-size: 0.75rem;
		font-weight: 500;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		padding: 0.25rem 0.5rem;
		border-radius: var(--radius-1);
		background: var(--surface-2);
		color: var(--ink-400);
	}

	.state-badge-connected {
		background: var(--surface-success, #2a3a2a);
		color: var(--ink-100);
	}

	.state-badge-expired {
		background: var(--surface-warn, #3a3320);
		color: var(--ink-100);
	}

	.state-badge-error {
		background: var(--surface-error, #3a2424);
		color: var(--ink-100);
	}

	.connected-row {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		margin-block-end: var(--space-4);
	}

	.avatar {
		inline-size: 3rem;
		block-size: 3rem;
		border-radius: 50%;
		object-fit: cover;
		flex-shrink: 0;
	}

	.username {
		margin: 0;
	}

	.username-prefix {
		color: var(--ink-400);
		margin-inline-end: var(--space-2);
	}

	.actions {
		display: flex;
		gap: var(--space-3);
		flex-wrap: wrap;
	}

	.btn {
		font-family: var(--font-display);
		font-size: 0.9375rem;
		font-weight: 500;
		padding: 0.5rem 1rem;
		border-radius: var(--radius-1);
		border: 1px solid var(--border-1);
		background: var(--surface-2);
		color: var(--ink-100);
		cursor: pointer;
		transition: background-color var(--dur-fast) var(--ease-out-soft);
	}

	.btn:hover {
		background: var(--surface-3);
	}

	.btn:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}

	.btn-primary {
		background: var(--accent-oxblood, #8c2a2a);
		border-color: var(--accent-oxblood, #8c2a2a);
		color: white;
	}

	.btn-primary:hover {
		background: var(--accent-oxblood-hover, #a03434);
	}

	.provider-disabled-hint {
		font-size: 0.9375rem;
		color: var(--ink-400);
		margin: 0;
	}

	.page-foot {
		margin-block-start: var(--space-7);
		padding-block-start: var(--space-5);
		border-block-start: 1px solid var(--border-1);
		display: flex;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
	}

	.privacy-line {
		font-size: 0.875rem;
		color: var(--ink-400);
		margin: 0;
	}

	.inline-link {
		background: none;
		border: none;
		padding: 0;
		color: var(--accent-oxblood, #c66);
		cursor: pointer;
		font: inherit;
		text-decoration: underline;
		text-underline-offset: 2px;
	}

	.back-link {
		font-size: 0.875rem;
		color: var(--ink-400);
		text-decoration: none;
	}

	.back-link:hover {
		color: var(--ink-100);
	}
</style>
