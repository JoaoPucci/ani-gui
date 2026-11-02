<!--
  A mockup card's meta line in PosterCard's voice: uppercase and
  tracked, the figure bright and the word faint, with a check before
  "Watched". A unit ("EP") joins the figure before it, without a dot.
  The dot sits inside the segment it introduces, so a wrapped line
  never ends on one; "Watched" takes none, its check already sets it
  apart.
-->
<script lang="ts">
	import type { MetaSegment } from './meta';

	let { segments }: { segments: readonly MetaSegment[] } = $props();
</script>

<span class="meta">
	{#each segments as seg, i (i)}
		<span
			class="seg"
			class:num={seg.kind === 'num' || seg.kind === 'watched'}
			class:unit={seg.kind === 'unit'}
		>
			{#if i > 0 && seg.kind !== 'unit' && seg.kind !== 'watched'}<span
					class="dot"
					aria-hidden="true">·</span
				>{/if}
			{#if seg.kind === 'watched'}
				<svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true">
					<path
						d="M3 8.5l3 3 7-7"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			{/if}
			{seg.text}
		</span>
	{/each}
</span>

<style>
	.meta {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-2);
		font-family: var(--font-body);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		font-weight: 500;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		line-height: 1.4;
		color: color-mix(in oklab, var(--bone-100) 45%, transparent);
	}
	.seg {
		display: inline-flex;
		align-items: center;
		gap: 0.3em;
		white-space: nowrap;
	}
	.num {
		color: var(--bone-100);
	}
	.unit {
		letter-spacing: var(--tracking-micro);
	}
	.dot {
		margin-inline-end: calc(var(--space-2) - 0.3em);
		color: color-mix(in oklab, var(--bone-100) 35%, transparent);
	}
</style>
