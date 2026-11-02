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

  `shape` comes from the row style ($lib/rows/prefs `actionShape`).
  `label` is the pill above. `icon` rests on the icon alone, in a 32px
  circle, and widens to that same pill on hover and on keyboard focus,
  the icon shrinking to its size in the pill as it goes. The label is
  in the button throughout, clipped at rest, never hidden, so the
  accessible name is the full label whether or not it is on show.

  The open width is reserved, not overlaid: the slot is as wide as the
  open pill (its ::after sets the label from `data-label`, unpainted)
  and the button opens over it, positioned out of flow. The header
  decides how its line wraps from the widths it measures; with the
  width held, opening changes no width, so the title and the status
  never move and no line rewraps under the pointer. The cost is that
  the line wraps as early as it did for the labelled pill, and the
  slot's unpainted rest of the width trails the line, where nothing
  follows it. Only the button takes the pointer, so the reserve never
  opens it or takes a click.
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
		icon?: 'refresh' | 'clear-history' | null;
		tone?: 'default' | 'quiet';
		busy?: boolean;
		shape?: RowActionShape;
		onclick: () => void;
	} = $props();
</script>

<span class="row-action-slot" data-shape={shape} data-label={label}
	><button
		type="button"
		class="row-action"
		class:quiet={tone === 'quiet'}
		class:busy
		aria-disabled={busy ? 'true' : undefined}
		onclick={() => {
			if (!busy) onclick();
		}}
	>
		{#if icon}<Icon name={icon} size={14} />{/if}
		<span class="row-action-label">{label}</span>
	</button></span
>

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
	.row-action:hover:not([aria-disabled='true']),
	[data-shape='icon'] .row-action:focus-visible:not([aria-disabled='true']) {
		background: var(--bone-100);
		border-color: var(--bone-100);
		color: var(--ink-000);
	}
	.row-action.quiet {
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.row-action.quiet:hover:not([aria-disabled='true']),
	[data-shape='icon'] .row-action.quiet:focus-visible:not([aria-disabled='true']) {
		background: color-mix(in oklab, var(--accent-oxblood) 18%, transparent);
		border-color: var(--accent-oxblood);
		color: var(--bone-100);
	}
	.row-action[aria-disabled='true'] {
		cursor: default;
		opacity: 0.7;
	}
	/* The icon shape: at rest a 32px circle over the slot's reserved
	   width, open on hover and keyboard focus. The pill's measures are
	   shared with the slot's sizer so the open pill and the room held
	   for it cannot drift apart. */
	.row-action-slot {
		--ra-pad: var(--space-3);
		--ra-gap: var(--space-2);
		--ra-icon: 14px;
	}
	.row-action-slot[data-shape='icon'] {
		position: relative;
		display: inline-block;
		flex: none;
		block-size: var(--space-6);
	}
	/* The open pill's width, in flow and never painted. visibility keeps
	   the generated words out of the accessibility tree: the name is
	   the button's own label, read once. */
	.row-action-slot[data-shape='icon']::after {
		content: attr(data-label);
		display: block;
		visibility: hidden;
		box-sizing: border-box;
		block-size: var(--space-6);
		padding-inline: calc(var(--ra-pad) + var(--ra-icon) + var(--ra-gap)) var(--ra-pad);
		border: 1px solid transparent;
		font-family: var(--font-body);
		font-size: 0.8125rem;
		font-weight: 600;
		letter-spacing: 0.14em;
		line-height: calc(var(--space-6) - 2px);
		text-transform: uppercase;
		white-space: nowrap;
	}
	/* At rest. Its transitions are the closing ones: the label leaves
	   first, then the shape draws in on an in-out curve whose slow start
	   keeps a pointer grazing off and back from making it flicker. */
	.row-action-slot[data-shape='icon'] .row-action {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		inline-size: var(--space-6);
		/* 8px start: 1px edge + 8px + the icon's 16px at rest centre it
		   in the 32px circle. */
		padding-inline: var(--space-2) var(--ra-pad);
		gap: var(--ra-gap);
		justify-content: flex-start;
		overflow: hidden;
		transition:
			inline-size var(--dur-med) var(--ease-in-out),
			padding-inline-start var(--dur-med) var(--ease-in-out),
			background-color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	/* 16/14: the icon reads at 16px alone, and settles to the pill's
	   14px as the label arrives. `scale`, not `transform`, so the busy
	   spin (a transform) turns it at either size. */
	.row-action-slot[data-shape='icon'] .row-action :global(svg) {
		flex: none;
		scale: 1.1429;
		transition: scale var(--dur-med) var(--ease-in-out);
	}
	.row-action-slot[data-shape='icon'] .row-action-label {
		flex: none;
		opacity: 0;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	/* Open. The shape leads; the label fades in over the last part of
	   the width, so the pill reveals it rather than the words overtaking
	   it. No elastic overshoot here: it would paint past the reserve. */
	.row-action-slot[data-shape='icon'] .row-action:is(:hover, :focus-visible) {
		inline-size: 100%;
		padding-inline-start: var(--ra-pad);
		transition:
			inline-size var(--dur-med) var(--ease-out-soft),
			padding-inline-start var(--dur-med) var(--ease-out-soft),
			background-color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.row-action-slot[data-shape='icon'] .row-action:is(:hover, :focus-visible) :global(svg) {
		scale: 1;
		transition: scale var(--dur-med) var(--ease-out-soft);
	}
	.row-action-slot[data-shape='icon'] .row-action:is(:hover, :focus-visible) .row-action-label {
		opacity: 1;
		transition: opacity var(--dur-fast) var(--ease-out-soft) calc(var(--dur-med) - var(--dur-fast));
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
