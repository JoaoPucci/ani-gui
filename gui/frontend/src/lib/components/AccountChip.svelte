<!--
  AccountChip — topbar chip showing the active tracker session.

  Renders only when at least one provider has a known identity
  (connected, expired, or error-with-account). Clicking opens an
  anchored popover with the provider name, username, optional
  warning ("Session expired"), and quick Manage / Disconnect
  actions. Popover patterns mirror DownloadDock so the topbar
  behaves consistently across docks.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import Icon from './Icon.svelte';
	import { accountStore } from '$lib/account/store.svelte';
	import { chipDescriptor, type ChipState } from '$lib/account/chip-descriptor';
	import { clearPersistedAccount, dropListCache, dropProviderCache } from '$lib/account/api';
	import { disconnectAccount } from '$lib/account/connect-flow';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { m } from '$lib/paraglide/messages';

	const descriptor: ChipState = $derived(chipDescriptor(accountStore.byProvider));
	const visible = $derived(descriptor.kind === 'connected');

	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);

	$effect(() => {
		if (!open) return;
		const onPointerDown = (e: PointerEvent) => {
			const target = e.target as Node | null;
			if (!target) return;
			if (trigger?.contains(target)) return;
			const pop = document.getElementById('account-chip-pop');
			if (pop?.contains(target)) return;
			open = false;
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape') open = false;
		};
		document.addEventListener('pointerdown', onPointerDown);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('pointerdown', onPointerDown);
			document.removeEventListener('keydown', onKey);
		};
	});

	function providerLabel(provider: 'anilist' | 'mal' | 'inhouse'): string {
		if (provider === 'anilist') return m.account_provider_anilist();
		if (provider === 'mal') return m.account_provider_mal();
		return provider;
	}

	function warningLabel(warning: 'expired' | 'error' | null): string | null {
		if (warning === 'expired') return m.account_chip_warning_expired();
		if (warning === 'error') return m.account_chip_warning_error();
		return null;
	}

	async function onManage() {
		open = false;
		await goto(resolve('/account'));
	}

	async function onDisconnect() {
		if (descriptor.kind !== 'connected') return;
		const provider = descriptor.provider;
		const prev = accountStore.byProvider[provider];
		open = false;
		const r = await disconnectAccount(provider, prev, {
			clearPersistedAccount,
			dropListCache,
			dropProviderCache
		});
		if (r.kind === 'token_clear_failed') {
			accountStore.setError(provider, m.account_connect_error_unknown());
			toastStore.push({ kind: 'error', message: m.account_disconnect_error_token_clear() });
			return;
		}
		accountStore.setDisconnected(provider);
	}
</script>

{#if visible && descriptor.kind === 'connected'}
	<div class="account-chip">
		<button
			bind:this={trigger}
			type="button"
			class="account-chip-trigger"
			class:warn={descriptor.warning !== null}
			aria-haspopup="menu"
			aria-expanded={open}
			aria-label={m.account_chip_aria_label({ username: descriptor.username })}
			onclick={() => (open = !open)}
		>
			<span class="account-chip-avatar" aria-hidden="true">
				{#if descriptor.avatarUrl}
					<img src={descriptor.avatarUrl} alt="" loading="lazy" decoding="async" />
				{:else}
					<Icon name="account" size={18} />
				{/if}
				{#if descriptor.warning !== null}
					<span class="account-chip-dot" data-kind={descriptor.warning}></span>
				{/if}
			</span>
			<span class="account-chip-name">{descriptor.username}</span>
		</button>

		{#if open}
			<div
				id="account-chip-pop"
				class="account-chip-pop"
				role="menu"
				aria-label={m.account_chip_pop_aria_label()}
			>
				<header class="account-chip-pop-header">
					<span class="account-chip-pop-provider">{providerLabel(descriptor.provider)}</span>
					<span class="account-chip-pop-username">{descriptor.username}</span>
					{#if warningLabel(descriptor.warning)}
						<span class="account-chip-pop-warning" data-kind={descriptor.warning}
							>{warningLabel(descriptor.warning)}</span
						>
					{/if}
				</header>
				<button type="button" class="account-chip-pop-action" role="menuitem" onclick={onManage}>
					{m.account_chip_pop_manage()}
				</button>
				<button
					type="button"
					class="account-chip-pop-action danger"
					role="menuitem"
					onclick={onDisconnect}
				>
					{m.account_chip_pop_disconnect()}
				</button>
			</div>
		{/if}
	</div>
{/if}

<style>
	.account-chip {
		position: relative;
		display: inline-flex;
		align-items: center;
	}
	.account-chip-trigger {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding-block: var(--space-2);
		padding-inline: var(--space-3);
		background: transparent;
		border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
		border-radius: var(--radius-pill, 999px);
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	.account-chip-trigger:hover {
		background: var(--surface-hover, rgba(255, 255, 255, 0.04));
	}
	.account-chip-trigger.warn {
		border-color: var(--amber, #d29a3a);
	}
	.account-chip-avatar {
		position: relative;
		inline-size: 28px;
		block-size: 28px;
		border-radius: var(--radius-card, 8px);
		overflow: hidden;
		background: var(--surface-2, rgba(255, 255, 255, 0.06));
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: 0 0 auto;
	}
	.account-chip-avatar img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.account-chip-dot {
		position: absolute;
		inset-block-end: 0;
		inset-inline-end: 0;
		inline-size: 8px;
		block-size: 8px;
		border-radius: 50%;
		border: 2px solid var(--bg, #0e0d0c);
	}
	.account-chip-dot[data-kind='expired'] {
		background: var(--amber, #d29a3a);
	}
	.account-chip-dot[data-kind='error'] {
		background: var(--danger, #c25d4e);
	}
	.account-chip-name {
		font-size: 0.875rem;
		max-inline-size: 14ch;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	@media (max-width: 720px) {
		.account-chip-name {
			display: none;
		}
	}

	.account-chip-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		min-inline-size: 14rem;
		padding: var(--space-2);
		background: var(--surface, #1a1816);
		border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
		border-radius: var(--radius-card, 12px);
		box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
		z-index: 60;
	}
	.account-chip-pop-header {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2);
		border-block-end: 1px solid var(--border, rgba(255, 255, 255, 0.06));
		margin-block-end: var(--space-1);
	}
	.account-chip-pop-provider {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-muted, rgba(255, 255, 255, 0.55));
	}
	.account-chip-pop-username {
		font-size: 0.95rem;
		font-weight: 600;
	}
	.account-chip-pop-warning {
		font-size: 0.75rem;
		margin-block-start: var(--space-1);
	}
	.account-chip-pop-warning[data-kind='expired'] {
		color: var(--amber, #d29a3a);
	}
	.account-chip-pop-warning[data-kind='error'] {
		color: var(--danger, #c25d4e);
	}
	.account-chip-pop-action {
		display: block;
		inline-size: 100%;
		text-align: start;
		padding-block: var(--space-2);
		padding-inline: var(--space-2);
		background: transparent;
		border: 0;
		border-radius: var(--radius-button, 8px);
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	.account-chip-pop-action:hover {
		background: var(--surface-hover, rgba(255, 255, 255, 0.06));
	}
	.account-chip-pop-action.danger {
		color: var(--danger, #c25d4e);
	}
</style>
