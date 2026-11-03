<!--
  One row of the notification panel.

  Anatomy: a kind glyph; the lead (title up to two lines, the time in
  mono at the inline end, a meta line under it); a column at the inline
  end that is always there, holding the dismiss control when the row
  may be dismissed; and, under the lead, the row's one action, always
  visible. Nothing appears on hover, so nothing moves: the dismiss
  column is reserved on every row and the action has its own line.

  The lead is the row's big target. Off the player it links to the
  notice's page; on the player it opens the notice in place
  (aria-expanded), so nothing navigates away from the episode.

  Tone is told with restraint: a failed event colours its glyph
  oxblood, a live condition amber, a resolved one swaps to a jade check
  and steps its text down a tier; news stays bone. Unread is a bone dot
  and a heavier title, and the word "Unread." for screen readers.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { slide } from '$lib/motion/transitions';
	import Icon from '$lib/components/Icon.svelte';
	import NoticeGlyph from './NoticeGlyph.svelte';
	import NoticeBody from './NoticeBody.svelte';
	import { copyOf, detailPath, when } from './copy';
	import { canDismiss, toneOf, type Notice } from './model';
	import { rowDomId } from './panel';

	let {
		notice,
		unread,
		inPlace,
		expanded = false,
		onToggle,
		onFollow,
		onAct,
		onDismiss
	}: {
		notice: Notice;
		unread: boolean;
		inPlace: boolean;
		expanded?: boolean;
		onToggle: () => void;
		onFollow: () => void;
		onAct: () => void;
		onDismiss: () => void;
	} = $props();

	const copy = $derived(copyOf(notice));
	const tone = $derived(toneOf(notice));
	const id = $derived(rowDomId(notice.key));
</script>

{#snippet lead()}
	<span class="nr-title" id="{id}-title">
		{#if unread}<span class="sr-only">{m.notices_unread_sr()}</span>{/if}
		{#if tone === 'good'}<span class="sr-only">{m.notices_resolved_sr()}</span>{/if}
		{copy.title}
	</span>
	<time class="nr-time" datetime={new Date(notice.at).toISOString()}>{when(notice.at)}</time>
	<span class="nr-meta">{copy.meta}</span>
	<span class="nr-mark" class:turned={expanded} aria-hidden="true">
		{#if inPlace}
			<svg viewBox="0 0 16 16" width="14" height="14">
				<path
					d="M4 6 L8 10 L12 6"
					fill="none"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="square"
				/>
			</svg>
		{:else}
			<Icon name="arrow-right" size={14} />
		{/if}
	</span>
{/snippet}

<li data-row class="nr tone-{tone}" class:unread>
	<span class="nr-glyph"><NoticeGlyph kind={notice.kind} settled={tone === 'good'} /></span>
	{#if inPlace}
		<button
			type="button"
			class="nr-lead"
			aria-expanded={expanded}
			aria-controls="{id}-more"
			onclick={onToggle}>{@render lead()}</button
		>
	{:else}
		<!-- eslint-disable svelte/no-navigation-without-resolve -- detailPath builds the route's own path, its key encoded -->
		<a class="nr-lead" href={detailPath(notice.key)} onclick={onFollow}>{@render lead()}</a>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	{/if}
	<span class="nr-end">
		{#if canDismiss(notice)}
			<button
				type="button"
				class="nr-dismiss"
				aria-label={m.notices_dismiss_aria({ title: copy.title })}
				title={m.notices_dismiss()}
				onclick={onDismiss}
			>
				<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
					<path
						d="M4 4 L12 12 M12 4 L4 12"
						fill="none"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="square"
					/>
				</svg>
			</button>
		{/if}
	</span>
	{#if inPlace && expanded}
		<div class="nr-more" id="{id}-more" transition:slide={{ duration: 200 }}>
			<NoticeBody copy={{ ...copy, action: null }} compact {onAct} />
		</div>
	{/if}
	{#if copy.action}
		<div class="nr-foot">
			<button type="button" class="nr-action" aria-describedby="{id}-title" onclick={onAct}
				>{copy.action.label}</button
			>
		</div>
	{/if}
</li>

<style>
	.nr {
		position: relative;
		display: grid;
		grid-template-columns: 1.25rem minmax(0, 1fr) 1.75rem;
		column-gap: var(--space-3);
		padding-block: var(--space-3);
		padding-inline: var(--space-4) var(--space-2);
		border-radius: var(--radius-control);
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.nr:hover,
	.nr:focus-within {
		background: color-mix(in oklab, var(--bone-100) 4%, var(--ink-050));
	}
	/* Unread: a bone dot in the gutter, on the title's first line. Not
	   the brand colour, which sits too close to the oxblood of a
	   failure. */
	.nr.unread::before {
		content: '';
		position: absolute;
		inset-inline-start: 0.375rem;
		inset-block-start: calc(var(--space-3) + 0.55em);
		inline-size: 6px;
		block-size: 6px;
		border-radius: 50%;
		background: var(--bone-100);
	}
	.nr-glyph {
		grid-column: 1;
		grid-row: 1;
		display: inline-flex;
		padding-block-start: 0.125rem;
		color: var(--bone-300);
	}
	.tone-failure .nr-glyph {
		color: var(--accent-oxblood);
	}
	.tone-live .nr-glyph {
		color: var(--accent-ochre);
	}
	.tone-good .nr-glyph {
		color: var(--accent-jade);
	}
	.nr-lead {
		grid-column: 2;
		grid-row: 1;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-areas:
			'title time'
			'meta mark';
		column-gap: var(--space-3);
		row-gap: 0.125rem;
		align-items: baseline;
		min-inline-size: 0;
		margin: 0;
		padding: 0;
		background: none;
		border: 0;
		border-radius: 2px;
		text-align: start;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	.nr-lead:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 3px;
	}
	.nr-title {
		grid-area: title;
		min-inline-size: 0;
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		overflow-wrap: anywhere;
		font-size: var(--type-meta);
		line-height: 1.35;
		font-weight: 450;
		color: var(--bone-200);
	}
	.unread .nr-title {
		font-weight: 600;
		color: var(--bone-100);
	}
	.tone-good .nr-title {
		color: var(--bone-300);
	}
	.nr-time {
		grid-area: time;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-variant-numeric: tabular-nums;
		color: var(--bone-400);
		white-space: nowrap;
	}
	.unread .nr-time {
		color: var(--bone-300);
	}
	.nr-meta {
		grid-area: meta;
		min-inline-size: 0;
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		overflow-wrap: anywhere;
		font-size: var(--type-micro);
		line-height: 1.45;
		color: var(--bone-300);
	}
	.tone-good .nr-meta {
		color: var(--bone-400);
	}
	.nr-mark {
		grid-area: mark;
		display: inline-flex;
		justify-self: end;
		align-self: start;
		margin-block-start: 0.125rem;
		color: var(--bone-400);
		transition:
			transform var(--dur-med) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.nr-mark :global(svg) {
		display: block;
	}
	.nr-mark:dir(rtl) {
		transform: scaleX(-1);
	}
	.nr-mark.turned {
		transform: rotate(180deg);
	}
	.nr-lead:hover .nr-mark,
	.nr-lead:focus-visible .nr-mark {
		color: var(--bone-100);
	}
	/* Reserved on every row, dismissable or not, so rows line up and
	   nothing shifts when a row gains or loses its control. */
	.nr-end {
		grid-column: 3;
		grid-row: 1;
		display: flex;
		align-items: flex-start;
		justify-content: center;
	}
	.nr-dismiss {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.75rem;
		block-size: 1.75rem;
		margin-block-start: -0.25rem;
		padding: 0;
		border-radius: var(--radius-control);
		color: var(--bone-400);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.nr-dismiss:hover,
	.nr-dismiss:focus-visible {
		color: var(--bone-100);
		background: color-mix(in oklab, var(--bone-100) 8%, transparent);
	}
	.nr-more {
		grid-column: 2;
		padding-block: var(--space-3) var(--space-1);
		min-inline-size: 0;
	}
	.nr-foot {
		grid-column: 2;
		display: flex;
		margin-block-start: var(--space-2);
		min-inline-size: 0;
	}
	/* The one action: quiet and outlined, so a list is not a column of
	   loud buttons. The detail page carries the loud version. */
	.nr-action {
		max-inline-size: 100%;
		padding: 0.25rem 0.625rem;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		font-size: var(--type-micro);
		font-weight: 500;
		line-height: 1.4;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.nr-action:hover {
		background: color-mix(in oklab, var(--bone-100) 8%, transparent);
		border-color: var(--bone-300);
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
		border: 0;
	}
</style>
