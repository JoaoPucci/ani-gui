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
  destructive accent on hover. `busy` makes the action unavailable and
  turns its icon; where the row has a status line the page says there
  what is happening (a failed row has none, and relies on the page's
  announcement). Unavailable is aria-disabled, not `disabled`: a
  disabled button drops keyboard focus to the page in Chromium, so a
  keyboard user who pressed it would start again from the top.
-->
<script lang="ts">
	import Icon from './Icon.svelte';
	import type { RowActionShape } from '$lib/rows/prefs';

	let {
		label,
		icon = null,
		tone = 'default',
		busy = false,
		shape = 'label',
		onclick
	}: {
		label: string;
		icon?: 'refresh' | null;
		tone?: 'default' | 'quiet';
		busy?: boolean;
		shape?: RowActionShape;
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	class="row-action"
	class:quiet={tone === 'quiet'}
	class:busy
	data-shape={shape}
	aria-disabled={busy ? 'true' : undefined}
	onclick={() => {
		if (!busy) onclick();
	}}
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
		/* The edge is what marks this as a button on a line of uppercase
		   text: 40% bone is about 3.4:1 on the page and on a band. */
		--row-action-border: color-mix(in oklab, var(--bone-100) 40%, transparent);
		border: 1px solid var(--row-action-border);
		border-radius: var(--radius-pill);
		background: transparent;
		color: var(--bone-200);
		font-family: var(--font-body);
		/* The eyebrow's type (tokens.css .eyebrow), so the header line reads
		   as one voice. */
		font-size: 0.8125rem;
		font-weight: 600;
		letter-spacing: 0.14em;
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
	.row-action:hover:not([aria-disabled='true']) {
		background: var(--bone-100);
		border-color: var(--bone-100);
		color: var(--ink-000);
	}
	.row-action.quiet {
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.row-action.quiet:hover:not([aria-disabled='true']) {
		background: color-mix(in oklab, var(--accent-oxblood) 18%, transparent);
		border-color: var(--accent-oxblood);
		color: var(--bone-100);
	}
	.row-action[aria-disabled='true'] {
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
