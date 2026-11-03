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
	import { seedConnectedList } from '$lib/account/connect-seed';
	import { pkceForProvider } from '$lib/account/pkce-for-provider';
	import {
		buildAuthUrl,
		cancelOAuth,
		exchangeCode,
		fetchMe,
		openOAuth,
		openExternal,
		persistAccount,
		clearPersistedAccount,
		dropListCache,
		dropProviderCache
	} from '$lib/account/api';
	import { bearerFor, disconnectAccount } from '$lib/account/connect-flow';
	import {
		cancelConnect as cancelConnectAction,
		connectProvider
	} from '$lib/account/connect-action';
	import { handleChipDisconnect as handleDisconnect } from '$lib/account/chip-disconnect';
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
		void accountStore.hydrate();
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
	// helper and the slower response wins.
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

	// Connect / Sign in again, Cancel and Disconnect live in tested
	// $lib modules; each settles the provider whatever its bridge calls
	// do, a rejection included.
	function connect(provider: Provider) {
		return connectProvider(provider, {
			store: accountStore,
			flow: {
				generateState: randomState,
				generatePkce: () => pkceForProvider(provider),
				buildAuthUrl,
				openOAuth,
				exchangeCode,
				fetchMe,
				persistAccount
			},
			seedConnectedList,
			pushToast: (t) => toastStore.push(t)
		});
	}

	function cancelConnect(provider: Provider) {
		// Without a cancel the `connecting` state only resolves after
		// openOAuth's 5-minute server timeout. The OAuth server is a
		// process singleton, so the request names no provider; the
		// provider is for abandoning the sign-in if it cannot be sent.
		return cancelConnectAction(provider, {
			store: accountStore,
			cancelOAuth,
			pushToast: (t) => toastStore.push(t)
		});
	}

	function disconnect(provider: Provider) {
		return handleDisconnect(
			provider,
			accountStore.byProvider[provider],
			{
				disconnectAccount,
				// Supersede any in-flight boot refresh before the async clear
				// (disconnectAccount calls this first) so a refresh resolving
				// mid-disconnect can't re-persist/reconnect.
				beginAccountChange: () => accountStore.beginAccountChange(provider),
				clearPersistedAccount,
				dropListCache,
				dropProviderCache
			},
			{
				setError: (p, msg) => accountStore.setError(p, msg),
				setDisconnected: (p) => accountStore.setDisconnected(p),
				pushToast: (t) => toastStore.push(t),
				unknownErrorMessage: () => m.account_connect_error_unknown(),
				tokenClearFailedMessage: () => m.account_disconnect_error_token_clear(),
				reread: (p) => accountStore.reread(p),
				stateOf: (p) => accountStore.byProvider[p]
			}
		);
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
					<!-- Error-with-no-account covers the
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
					<button type="button" class="btn" onclick={() => cancelConnect(provider)}>
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
	   - cards layered on ink-100 with ink-300 hairline + brand-tinted
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
		font-size: clamp(2rem, 4vw, var(--type-display-l));
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
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card, 8px);
		padding: var(--space-5) var(--space-6);
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}

	.provider-card[data-state='connected'] {
		border-color: color-mix(in oklab, var(--accent-jade) 40%, var(--ink-300));
	}

	.provider-card[data-state='expired'] {
		border-color: color-mix(in oklab, var(--accent-ochre) 40%, var(--ink-300));
	}

	.provider-card[data-state='error'] {
		border-color: color-mix(in oklab, var(--accent-oxblood) 40%, var(--ink-300));
	}

	.provider-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid var(--ink-200);
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
		border: 1px solid var(--ink-200);
	}

	.state-badge-connected {
		background: color-mix(in oklab, var(--accent-jade) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-jade) 35%, var(--ink-200));
		color: var(--accent-jade);
	}

	.state-badge-expired {
		background: color-mix(in oklab, var(--accent-ochre) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-ochre) 35%, var(--ink-200));
		color: var(--accent-ochre);
	}

	.state-badge-error {
		background: color-mix(in oklab, var(--accent-oxblood) 14%, var(--ink-050));
		border-color: color-mix(in oklab, var(--accent-oxblood) 35%, var(--ink-200));
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
		border: 1px solid var(--ink-200);
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
		border: 1px solid var(--ink-300);
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
		border: 1px solid color-mix(in oklab, var(--brand) 45%, var(--ink-200));
		color: var(--brand);
	}

	.page-foot {
		display: flex;
		justify-content: space-between;
		gap: var(--space-4);
		flex-wrap: wrap;
		padding-block-start: var(--space-5);
		border-block-start: 1px solid var(--ink-200);
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
