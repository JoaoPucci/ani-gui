<!--
  ConfirmDialog — small two-button confirmation modal. Mirrors the
  visual treatment of DownloadConfirm (backdrop blur, ink-050 card,
  display-font title, mono eyebrow) but trimmed to a single decision
  point. Used for destructive actions where a `Cancel / Confirm`
  prompt is enough — per-row history delete and clear-all today, more
  later.

  Esc cancels; clicking the backdrop cancels. Confirm focuses on
  open so Enter activates it directly.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';

	interface Props {
		open: boolean;
		/** Short label above the title — same micro-typography role the
		 *  DownloadConfirm eyebrow plays. Stays terse ("Confirm",
		 *  "Remove"). */
		eyebrow?: string;
		/** Headline — the actual question being asked. Should be a full
		 *  sentence; the buttons answer it. */
		title: string;
		/** Optional supporting sentence beneath the title. Keep short. */
		body?: string | null;
		confirmLabel: string;
		cancelLabel?: string;
		/** When true, the confirm button takes the oxblood danger tint
		 *  instead of the brand. Use for irreversible actions. */
		destructive?: boolean;
		/** When true, the confirm button shows a busy state and both
		 *  buttons are disabled. Bound by the caller around the async
		 *  confirm. */
		busy?: boolean;
		onConfirm: () => void;
		onCancel: () => void;
	}
	let {
		open,
		eyebrow,
		title,
		body = null,
		confirmLabel,
		cancelLabel,
		destructive = false,
		busy = false,
		onConfirm,
		onCancel
	}: Props = $props();

	let confirmBtn: HTMLButtonElement | undefined = $state();

	$effect(() => {
		// Auto-focus the confirm button when the modal opens so Enter
		// fires it. Has to wait a tick for the element to mount.
		if (open && confirmBtn) {
			confirmBtn.focus();
		}
	});

	function onBackdropClick() {
		if (!busy) onCancel();
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Escape' && !busy) {
			e.preventDefault();
			onCancel();
		}
	}
</script>

<svelte:window onkeydown={onKey} />

{#if open}
	<!-- Backdrop closes on click, role=dialog traps screen readers,
	     aria-modal keeps assistive tech focused inside. -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="cd-backdrop"
		role="dialog"
		aria-modal="true"
		aria-labelledby="cd-title"
		tabindex="-1"
		onclick={onBackdropClick}
	>
		<div
			class="cd-card"
			role="document"
			onclick={(e) => e.stopPropagation()}
			onkeydown={(e) => e.stopPropagation()}
			tabindex="-1"
		>
			<header class="cd-head">
				{#if eyebrow}
					<p class="cd-eyebrow" aria-hidden="true">
						<span class="cd-eyebrow-rule"></span>
						<span class="cd-eyebrow-key">{eyebrow}</span>
					</p>
				{/if}
				<h2 id="cd-title" class="cd-title">{title}</h2>
				{#if body}
					<p class="cd-body">{body}</p>
				{/if}
			</header>
			<footer class="cd-foot">
				<button type="button" class="cd-btn cd-btn-quiet" onclick={onCancel} disabled={busy}>
					{cancelLabel ?? m.confirm_button_cancel()}
				</button>
				<button
					bind:this={confirmBtn}
					type="button"
					class="cd-btn"
					class:cd-btn-accent={!destructive}
					class:cd-btn-danger={destructive}
					onclick={onConfirm}
					disabled={busy}
				>
					{confirmLabel}
				</button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.cd-backdrop {
		position: fixed;
		inset: 0;
		background: color-mix(in oklab, var(--ink-000) 70%, transparent);
		backdrop-filter: blur(4px);
		display: grid;
		place-items: center;
		z-index: 100;
		animation: cd-fade var(--dur-fast) var(--ease-out-soft) both;
	}
	@keyframes cd-fade {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	.cd-card {
		inline-size: min(28rem, calc(100vw - var(--space-6)));
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-card-hover);
		padding: var(--space-5);
		animation: cd-rise var(--dur-med) var(--ease-out-elastic) both;
	}
	@keyframes cd-rise {
		from {
			opacity: 0;
			transform: translateY(8px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	.cd-head {
		margin-block-end: var(--space-5);
	}
	.cd-eyebrow {
		margin: 0 0 var(--space-2);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.cd-eyebrow-rule {
		display: inline-block;
		inline-size: 2rem;
		block-size: 1px;
		background: var(--accent-oxblood);
	}
	.cd-eyebrow-key {
		color: var(--accent-oxblood);
	}
	.cd-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-h4);
		font-weight: 600;
		color: var(--bone-100);
		line-height: var(--leading-tight);
	}
	.cd-body {
		margin: var(--space-3) 0 0;
		font-size: var(--type-small);
		line-height: var(--leading-relaxed);
		color: var(--bone-300);
	}
	.cd-foot {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-3);
	}
	.cd-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-pill);
		border: 1px solid transparent;
		font-family: inherit;
		font-size: var(--type-small);
		font-weight: 500;
		line-height: 1;
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.cd-btn:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
	.cd-btn-quiet {
		background: transparent;
		color: var(--bone-200);
		border-color: var(--ink-300);
	}
	.cd-btn-quiet:hover:not(:disabled) {
		background: var(--ink-100);
		color: var(--bone-100);
	}
	.cd-btn-accent {
		background: var(--brand);
		color: var(--brand-ink);
	}
	.cd-btn-accent:hover:not(:disabled) {
		background: color-mix(in oklab, var(--brand) 88%, white 12%);
	}
	.cd-btn-danger {
		background: var(--accent-oxblood);
		color: var(--bone-100);
	}
	.cd-btn-danger:hover:not(:disabled) {
		background: color-mix(in oklab, var(--accent-oxblood) 88%, white 12%);
	}
</style>
