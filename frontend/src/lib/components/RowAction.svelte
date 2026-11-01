<!--
  RowAction — a labelled action in a home row's header (Strip's
  `headerActions`). One shape for every row: a hairline pill set in the
  eyebrow's type, so it reads as part of the row's title line rather
  than as a toolbar button parked at the window's edge.

  The visible label is the accessible name: no aria-label or title
  stands in for it, so what a screen reader announces is what a voice
  user says and a sighted user reads (WCAG 2.5.3).

  `tone="quiet"` is for an action that removes something: it rests in
  the caption's colour so it never outshouts the row, and warms to the
  destructive accent on hover. `busy` disables the action and turns its
  icon; the page says what is happening in the row's status line.
-->
<script lang="ts">
	import Icon from './Icon.svelte';

	let {
		label,
		icon = null,
		tone = 'default',
		busy = false,
		onclick
	}: {
		label: string;
		icon?: 'refresh' | null;
		tone?: 'default' | 'quiet';
		busy?: boolean;
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	class="row-action"
	class:quiet={tone === 'quiet'}
	class:busy
	disabled={busy}
	{onclick}
>
	{#if icon}<Icon name={icon} size={14} />{/if}
	<span>{label}</span>
</button>

<style>
	.row-action {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		/* 32px: the smallest hit area the row headers allow. The header
		   reserves this height on every row, so a row with an action
		   and a row without one start their cards at the same depth. */
		block-size: var(--space-6);
		padding-inline: var(--space-3);
		border: 1px solid color-mix(in oklab, var(--bone-100) 30%, transparent);
		border-radius: var(--radius-pill);
		background: transparent;
		color: var(--bone-200);
		font-family: var(--font-body);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		line-height: 1;
		text-transform: uppercase;
		white-space: nowrap;
		cursor: pointer;
		/* --dur-fast is 0ms under prefers-reduced-motion (tokens.css). */
		transition:
			background-color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.row-action:hover:not(:disabled) {
		background: var(--bone-100);
		border-color: var(--bone-100);
		color: var(--ink-000);
	}
	.row-action.quiet {
		border-color: color-mix(in oklab, var(--bone-100) 22%, transparent);
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.row-action.quiet:hover:not(:disabled) {
		background: color-mix(in oklab, var(--accent-oxblood) 18%, transparent);
		border-color: var(--accent-oxblood);
		color: var(--bone-100);
	}
	.row-action:disabled {
		cursor: default;
		opacity: 0.7;
	}
	/* Keyboard focus takes the app's global ring (tokens.css). */
	.row-action.busy :global(svg) {
		animation: row-action-spin 0.8s linear infinite;
	}
	@keyframes row-action-spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.row-action.busy :global(svg) {
			animation: none;
		}
	}
</style>
