<!--
  Accounts management surface. Reached from Settings → Accounts or
  the topbar chip.

  AniList + MyAnimeList cards. Both flow through the same
  `connect(provider)` adapter; PKCE method is picked per provider
  (S256 for AniList, plain for MAL) via `pkceForProvider`.

  All visible strings via Paraglide per AGENTS.md §6. Imperative
  connect-flow logic delegated to $lib/account/connect-flow to keep
  the page itself a thin adapter — per AGENTS.md §2.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages';
	import { imageProxyUrl, settingsGet, settingsPut, type Config } from '$lib/api';
	import { accountStore } from '$lib/account/store.svelte';
	import { parsePrimaryProvider } from '$lib/account/chip-descriptor';
	import { primaryAccountStore } from '$lib/account/primary-store.svelte';
	import { applyPrimarySelection } from '$lib/account/set-primary';
	import { pkceForProvider } from '$lib/account/pkce-for-provider';
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

	// Persisted settings — only `primary_account` is read/written here.
	// Loaded once on mount; the picker writes the whole Config back
	// (settings_put is a full round-trip, matching /settings).
	let config = $state<Config | null>(null);

	// Providers the user can pick as primary: any with a known identity
	// (connected / expired / error-with-account), mirroring the chip's
	// "surviving identity" rule. The picker only matters with 2+.
	const identityProviders = $derived(
		(['anilist', 'mal'] as Provider[]).filter((p) => {
			const s = accountStore.byProvider[p];
			return s.kind === 'connected' || s.kind === 'expired' || (s.kind === 'error' && !!s.account);
		})
	);

	// Which connected provider currently leads (the one whose card shows
	// the Primary flag). Driven by the shared store so the flag moves the
	// instant the user clicks — no wait for the persisted config. Falls
	// back to the first identity provider (AniList-first) when no explicit
	// choice is set or the chosen one isn't connected.
	const effectivePrimary = $derived.by(() => {
		const ids = identityProviders;
		if (ids.length === 0) return null;
		const chosen = primaryAccountStore.value;
		return chosen && ids.includes(chosen) ? chosen : ids[0];
	});

	onMount(() => {
		accountStore.hydrate();
		void settingsGet()
			.then((c) => {
				config = c;
				primaryAccountStore.set(parsePrimaryProvider(c.primary_account));
			})
			.catch(() => {});
	});

	// True while a primary-tracker write is in flight. Gates the radios
	// so a second pick can't race the first: without it, clicking again
	// before settingsPut resolves passes a stale `config` into the
	// helper and the slower response wins (Codex P2 #3413276568).
	let savingPrimary = $state(false);

	async function setPrimary(value: string) {
		if (savingPrimary) return;
		savingPrimary = true;
		try {
			// Orchestration (guard / optimistic store update / persist /
			// rollback-on-failure) lives in the tested helper; the page
			// just adopts whichever config it returns.
			config = await applyPrimarySelection(config, value, {
				persist: settingsPut,
				applyToStore: (p) => primaryAccountStore.set(p),
				onError: () => toastStore.push({ kind: 'error', message: m.account_primary_save_error() })
			});
		} finally {
			savingPrimary = false;
		}
	}

	async function connect(provider: Provider) {
		// Snapshot the pre-click state so a failed reconnect-from-
		// expired (or connect-from-error-with-account) can restore the
		// UI to it instead of collapsing to `disconnected` — the
		// persisted token is still on disk and hydrate() would resurrect
		// it on next launch anyway (Codex P2 #3370011851).
		const prev = accountStore.byProvider[provider];
		accountStore.setConnecting(provider);
		const r = await connectAccount(provider, {
			generateState: randomState,
			generatePkce: () => pkceForProvider(provider),
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
			// Codex P2 #3372942245: surface the underlying keychain
			// failure so Linux users without a usable libsecret/kwallet
			// see "install your OS keyring" instead of the generic
			// sign-in error.
			const message =
				r.reason === 'encryption_unavailable'
					? m.account_persist_error_keychain_unavailable()
					: m.account_connect_error_unknown();
			accountStore.setError(provider, message);
			toastStore.push({ kind: 'error', message });
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

	{#each [{ provider: 'anilist' as Provider, name: m.account_provider_anilist() }, { provider: 'mal' as Provider, name: m.account_provider_mal() }] as { provider, name } (provider)}
		{@const state = accountStore.byProvider[provider]}
		<section class="provider-card" data-state={state.kind}>
			<header class="provider-head">
				<h2 class="provider-name">{name}</h2>
				<div class="head-meta">
					{#if identityProviders.length >= 2 && effectivePrimary === provider}
						<span class="primary-flag" title={m.account_primary_hint()}>
							{m.account_primary_badge()}
						</span>
					{/if}
					<span class="state-badge state-badge-{state.kind}">
						{stateBadgeKind(state)}
					</span>
				</div>
			</header>

			{#if state.kind === 'connected' || state.kind === 'expired' || (state.kind === 'error' && state.account)}
				<div class="connected-row">
					{#if state.account && imageProxyUrl(state.account.avatar_url)}
						<img
							class="avatar"
							src={imageProxyUrl(state.account.avatar_url)}
							alt=""
							width="48"
							height="48"
						/>
					{/if}
					<div class="user-meta">
						<p class="username">
							<span class="username-prefix">{m.account_card_username_prefix()}</span>
							<strong>{state.account?.username}</strong>
						</p>
					</div>
				</div>
			{/if}

			<div class="actions">
				{#if state.kind === 'disconnected'}
					<button type="button" class="btn btn-primary" onclick={() => connect(provider)}>
						{m.account_card_action_connect()}
					</button>
				{:else if state.kind === 'error' && !state.account}
					<!-- Codex P2 #3371530183: error-with-no-account covers the
					     orphan-file case where hydrate() couldn't read the
					     keychain. Offer Disconnect so the user can call
					     clearToken and clean up before reconnecting; the
					     Connect button stays so they can try again now if
					     it was a transient failure. -->
					<button type="button" class="btn btn-primary" onclick={() => connect(provider)}>
						{m.account_card_action_connect()}
					</button>
					<button type="button" class="btn" onclick={() => disconnect(provider)}>
						{m.account_card_action_disconnect()}
					</button>
				{:else if state.kind === 'connecting'}
					<button type="button" class="btn" disabled>
						{m.account_card_status_connecting()}
					</button>
					<button type="button" class="btn" onclick={cancelConnect}>
						{m.account_card_action_cancel()}
					</button>
				{:else if state.kind === 'expired' || (state.kind === 'error' && state.account)}
					<button type="button" class="btn btn-primary" onclick={() => connect(provider)}>
						{m.account_card_action_reconnect()}
					</button>
					<button type="button" class="btn" onclick={() => disconnect(provider)}>
						{m.account_card_action_disconnect()}
					</button>
					{#if identityProviders.length >= 2 && effectivePrimary !== provider}
						<button
							type="button"
							class="btn"
							disabled={savingPrimary}
							title={m.account_primary_hint()}
							onclick={() => setPrimary(provider)}
						>
							{m.account_primary_make()}
						</button>
					{/if}
				{:else}
					<button type="button" class="btn" onclick={() => disconnect(provider)}>
						{m.account_card_action_disconnect()}
					</button>
					{#if identityProviders.length >= 2 && effectivePrimary !== provider}
						<button
							type="button"
							class="btn"
							disabled={savingPrimary}
							title={m.account_primary_hint()}
							onclick={() => setPrimary(provider)}
						>
							{m.account_primary_make()}
						</button>
					{/if}
				{/if}
			</div>
		</section>
	{/each}

	<footer class="page-foot">
		<p class="privacy-line">
			{m.account_privacy_consent_line()}
			<button type="button" class="inline-link" onclick={() => openExternal(PRIVACY_URL)}>
				{m.account_privacy_link_label()}
			</button>.
		</p>
	</footer>
</main>

<style>
	/* Family alignment with /settings + /about (M3 design pass):
	   - mono micro eyebrow with brand-colored "key" word
	   - display title in bone-100
	   - body subtitle in bone-200
	   - cards layered on ink-100 with bone-500 hairline + brand-tinted
	     border lifts on focus / state
	   The original styling here referenced --surface-* / --ink-3xx /
	   --radius-N tokens that don't exist in tokens.css, which is why
	   the page rendered as undefined fallbacks. */
	.account-page {
		max-inline-size: 60rem;
		margin-inline: auto;
		padding-block: var(--space-7);
		padding-inline: var(--space-7);
		display: flex;
		flex-direction: column;
		gap: var(--space-7);
	}

	.page-head {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.eyebrow {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--brand);
	}

	.display {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-display-s, 2rem);
		font-weight: 700;
		line-height: 1.1;
		letter-spacing: -0.01em;
		color: var(--bone-100);
	}

	.subtitle {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		line-height: 1.55;
		color: var(--bone-200);
		max-inline-size: 56ch;
	}

	.provider-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		background: var(--ink-100);
		border: 1px solid var(--bone-500, var(--ink-200));
		border-radius: var(--radius-card, 8px);
		padding: var(--space-5) var(--space-6);
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}

	.provider-card[data-state='connected'] {
		border-color: color-mix(in oklab, var(--accent-jade) 40%, var(--bone-500, var(--ink-200)));
	}

	.provider-card[data-state='expired'] {
		border-color: color-mix(in oklab, var(--accent-ochre) 40%, var(--bone-500, var(--ink-200)));
	}

	.provider-card[data-state='error'] {
		border-color: color-mix(in oklab, var(--accent-oxblood) 40%, var(--bone-500, var(--ink-200)));
	}

	.provider-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid var(--bone-600, var(--ink-200));
	}

	.provider-name {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-display-m);
		font-weight: 600;
		color: var(--bone-100);
	}

	.state-badge {
		display: inline-flex;
		align-items: center;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		padding: var(--space-1) var(--space-3);
		border-radius: var(--radius-pill);
		background: var(--ink-050);
		color: var(--bone-300);
		border: 1px solid var(--bone-600, var(--ink-200));
	}

	.state-badge-connected {
		background: color-mix(in oklab, var(--accent-jade) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-jade) 35%, var(--bone-600, var(--ink-200)));
		color: var(--accent-jade);
	}

	.state-badge-expired {
		background: color-mix(in oklab, var(--accent-ochre) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-ochre) 35%, var(--bone-600, var(--ink-200)));
		color: var(--accent-ochre);
	}

	.state-badge-error {
		background: color-mix(in oklab, var(--accent-oxblood) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-oxblood) 35%, var(--bone-600, var(--ink-200)));
		color: var(--accent-oxblood);
	}

	.connected-row {
		display: flex;
		align-items: center;
		gap: var(--space-4);
	}

	.avatar {
		/* AniList ships 230x230 squares composed edge-to-edge — a full
		   pill clips characters/faces. Soft-rounded square keeps the
		   source crop intact and matches the card-corner family. */
		inline-size: 3rem;
		block-size: 3rem;
		border-radius: var(--radius-card, 8px);
		object-fit: cover;
		flex-shrink: 0;
		border: 1px solid var(--bone-600, var(--ink-200));
	}

	.user-meta {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.username {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-100);
	}

	.username-prefix {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-400);
		margin-inline-end: var(--space-2);
	}

	.actions {
		display: flex;
		gap: var(--space-3);
		flex-wrap: wrap;
	}

	.btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		background: transparent;
		border: 1px solid var(--bone-500, var(--ink-200));
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}

	.btn:hover:not(:disabled) {
		color: var(--bone-100);
		border-color: var(--bone-400);
	}

	.btn:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 2px var(--ink-000),
			0 0 0 4px var(--brand);
	}

	.btn:disabled {
		cursor: not-allowed;
		opacity: 0.55;
	}

	.btn-primary {
		color: var(--brand-ink);
		background: var(--brand);
		border-color: var(--brand);
	}

	.btn-primary:hover:not(:disabled) {
		color: var(--brand-ink);
		background: color-mix(in oklab, var(--brand) 90%, white);
		border-color: color-mix(in oklab, var(--brand) 90%, white);
	}

	.head-meta {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
	}

	/* "Primary" flag on the leading provider's card — same pill family as
	   the state badge but brand-tinted to read as the active choice. */
	.primary-flag {
		display: inline-flex;
		align-items: center;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		padding: var(--space-1) var(--space-3);
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--brand) 16%, var(--ink-050));
		border: 1px solid color-mix(in oklab, var(--brand) 45%, var(--bone-600, var(--ink-200)));
		color: var(--brand);
	}

	.page-foot {
		display: flex;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
		padding-block-start: var(--space-5);
		border-block-start: 1px solid var(--bone-600, var(--ink-200));
	}

	.privacy-line {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-400);
	}

	.inline-link {
		background: none;
		border: none;
		padding: 0;
		color: var(--brand);
		cursor: pointer;
		font: inherit;
		text-decoration: underline;
		text-underline-offset: 2px;
		text-decoration-color: color-mix(in oklab, var(--brand) 45%, transparent);
		transition: text-decoration-color var(--dur-fast) var(--ease-out-soft);
	}

	.inline-link:hover {
		text-decoration-color: var(--brand);
	}
</style>
