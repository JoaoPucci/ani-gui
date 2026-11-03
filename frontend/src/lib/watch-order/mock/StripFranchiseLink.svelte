<!--
  The strip's way to the franchise page (mockup), at the end of the
  strip's footer, in two variants:

  - `button`: the show page's outline button (.btn.btn-outline) at its
    full size, with the number of entries the franchise page lists and
    the app's arrow;
  - `stack` (the maintainer's choice, third round): posters from
    different parts of the franchise, fanned on one arc, beside what
    the page holds. Hovering or focusing the link opens the fan
    outwards along the arc (stack-fan.ts) and shows the whole link as
    one target, in one of three highlights (`hover`). With no poster
    the stack is three blank frames, the front one carrying the
    franchise's entry count. Under reduced motion the fan is drawn
    open and does not move.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import { reducedMotion } from '$lib/motion/reduced-motion.svelte';
	import { m } from '$lib/paraglide/messages';
	import { numeral } from '../meta';
	import { FAN_RADIUS_REM, fanPoses, stackCards } from './stack-fan';
	import { franchiseSummary } from '../strip-view';
	import type { MockRun } from './types';

	let {
		run,
		id,
		variant,
		hover = 'surface',
		limit = 3
	}: {
		run: MockRun;
		id: string;
		variant: 'button' | 'stack';
		hover?: 'surface' | 'underline' | 'border';
		limit?: number;
	} = $props();

	const summary = $derived(franchiseSummary(run));
	const parts = $derived(
		[
			m.watchorder_strip_franchise_stack_main({ count: summary.main }),
			summary.films > 0 ? m.watchorder_strip_franchise_stack_films({ count: summary.films }) : null,
			summary.sides > 0 ? m.watchorder_strip_franchise_stack_sides({ count: summary.sides }) : null,
			summary.recaps > 0
				? m.watchorder_strip_franchise_stack_recaps({ count: summary.recaps })
				: null
		].filter((p) => p !== null)
	);
	const cards = $derived(stackCards(summary.posters, limit));
	const open = $derived(fanPoses(cards.length, 'open'));
	const rest = $derived(reducedMotion.current ? open : fanPoses(cards.length, 'rest'));
	// A poster that fails to load is drawn as a blank frame, not an empty card.
	let broken: Record<string, boolean> = $state({});
	const showsPoster = (c: (typeof cards)[number]) =>
		c.kind === 'poster' && !broken[c.entry.id] && imageProxyUrl(c.entry.poster) !== null;
</script>

{#if variant === 'button'}
	<a class="btn-franchise" href={resolve('/anime/[id]/franchise', { id })}>
		<span>{m.watchorder_strip_franchise_link()}</span>
		<span class="count">{m.watchorder_franchise_count({ count: summary.total })}</span>
		<span class="arrow" aria-hidden="true"><Icon name="arrow-right" size={16} /></span>
	</a>
{:else}
	<a class="stack hl-{hover}" href={resolve('/anime/[id]/franchise', { id })}>
		<span class="fan" style:--radius="{FAN_RADIUS_REM}rem" aria-hidden="true">
			{#each cards as card, i (i)}
				<span
					class="fan-card"
					class:blank={!showsPoster(card)}
					style:--rest-r="{rest[i].rotate}deg"
					style:--open-r="{open[i].rotate}deg"
					style:--rest-y="{cards.length === 1 ? rest[i].y : 0}rem"
					style:--open-y="{cards.length === 1 ? open[i].y : 0}rem"
				>
					{#if card.kind === 'poster' && showsPoster(card)}
						<img
							src={imageProxyUrl(card.entry.poster)}
							alt=""
							loading="lazy"
							decoding="async"
							onerror={() => (broken = { ...broken, [card.entry.id]: true })}
						/>
					{:else if card.kind === 'blank' && i === cards.length - 1}
						<span class="blank-count">{numeral(summary.total)}</span>
					{/if}
				</span>
			{/each}
		</span>
		<span class="stack-text">
			<span class="stack-title">{m.watchorder_strip_franchise_stack_title()}</span>
			<span class="stack-meta"
				><span class="meta-inner"
					>{#each parts as part, i (i)}<span class="part">{part}</span>{/each}</span
				></span
			>
		</span>
		<span class="arrow" aria-hidden="true"><Icon name="arrow-right" size={18} /></span>
	</a>
{/if}

<style>
	/* The show page's .btn.btn-outline, at the same size. */
	.btn-franchise {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		border: 1px solid var(--bone-300);
		border-radius: var(--radius-control);
		white-space: nowrap;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.btn-franchise:hover {
		border-color: var(--bone-100);
		transform: translateY(-1px);
	}
	.btn-franchise:focus-visible,
	.stack:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 2px;
	}
	.count {
		padding-inline-start: var(--space-3);
		border-inline-start: 1px solid var(--ink-300);
		font-weight: 500;
		color: var(--bone-300);
	}

	/* Arrows and the fan's spread follow the reading direction. */
	.btn-franchise,
	.stack {
		--flip: 1;
	}
	:global([dir='rtl']) .btn-franchise,
	:global([dir='rtl']) .stack {
		--flip: -1;
	}
	.arrow {
		display: inline-flex;
		color: var(--bone-300);
		transform: scaleX(var(--flip));
		transition:
			transform var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	.btn-franchise:hover .arrow,
	.stack:hover .arrow,
	.stack:focus-visible .arrow {
		color: var(--bone-100);
		transform: scaleX(var(--flip)) translateX(2px);
	}

	/* The whole link is one target. Its padding is the highlight's
	   area; the negative margin keeps the posters and text on the
	   footer's edge while nothing is highlighted. */
	.stack {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-4);
		max-inline-size: 100%;
		padding: var(--space-2) var(--space-3);
		margin: calc(-1 * var(--space-2)) calc(-1 * var(--space-3));
		border-radius: var(--radius-card);
		color: inherit;
		transition:
			background-color var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	/* Highlight, three options. Focus shows the same state as hover,
	   with the focus ring on top. */
	.hl-surface:hover,
	.hl-surface:focus-visible {
		background: var(--ink-100);
	}
	.hl-border {
		box-shadow: inset 0 0 0 1px transparent;
	}
	.hl-border:hover,
	.hl-border:focus-visible {
		box-shadow: inset 0 0 0 1px var(--bone-400);
	}
	.stack-title {
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 0.25em;
		text-decoration-color: transparent;
		transition: text-decoration-color var(--dur-fast) var(--ease-out-soft);
	}
	.hl-underline .stack-title {
		text-decoration-color: color-mix(in oklab, var(--bone-100) 25%, transparent);
	}
	.hl-underline:hover .stack-title,
	.hl-underline:focus-visible .stack-title {
		text-decoration-color: var(--bone-100);
	}
	/* The fan: every card stands at the middle of the box and is
	   rotated about the arc's centre, FAN_RADIUS_REM below its foot,
	   so the transition between the rest and open angles moves it
	   along the arc. */
	.fan {
		position: relative;
		inline-size: 5.5rem;
		block-size: 3.4rem;
	}
	.fan-card {
		position: absolute;
		inset-block-end: 0.1rem;
		inset-inline-start: calc(50% - 1.15rem);
		inline-size: 2.3rem;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: 4px;
		background: var(--ink-200);
		box-shadow:
			0 0 0 1px var(--ink-000),
			var(--shadow-card-rest);
		transform-origin: 50% calc(100% + var(--radius));
		transform: translateY(var(--rest-y)) rotate(calc(var(--rest-r) * var(--flip)));
		/* Back to rest without overshoot; out with the card easing's. */
		transition:
			transform var(--dur-med) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.stack:hover .fan-card,
	.stack:focus-visible .fan-card {
		transform: translateY(var(--open-y)) rotate(calc(var(--open-r) * var(--flip)));
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.stack:hover .fan-card:not(.blank),
	.stack:focus-visible .fan-card:not(.blank) {
		box-shadow:
			0 0 0 1px var(--ink-000),
			var(--shadow-card-hover);
	}
	/* No poster anywhere in the franchise: blank frames, the front one
	   carrying the entry count in the numeral motif. */
	.fan-card.blank {
		display: grid;
		place-items: center;
		background: var(--ink-200);
		box-shadow:
			inset 0 0 0 1px var(--ink-300),
			0 0 0 1px var(--ink-000);
	}
	.blank-count {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: 1.25rem;
		font-weight: 700;
		line-height: 1;
		color: var(--bone-100);
		padding-block-end: 2px;
		border-block-end: 2px solid var(--accent, var(--bone-400));
		/* Upright whatever the card's angle: the numeral is read, not fanned. */
		transform: rotate(calc(-1 * var(--rest-r) * var(--flip)));
		transition: transform var(--dur-med) var(--ease-out-soft);
	}
	.stack:hover .blank-count,
	.stack:focus-visible .blank-count {
		transform: rotate(calc(-1 * var(--open-r) * var(--flip)));
	}
	.fan-card img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.stack-text {
		display: grid;
		gap: 2px;
		min-inline-size: 0;
	}
	.stack-title {
		font-size: var(--type-body);
		font-weight: 600;
		color: var(--bone-100);
	}
	/* What the page holds, one part per item. Each part carries its
	   separating dot in the gutter before it; the gutter hangs outside
	   the clipped box, so a part that opens a line shows no dot. */
	.stack-meta {
		display: block;
		overflow: hidden;
		font-size: var(--type-micro);
		color: var(--bone-300);
	}
	.meta-inner {
		display: flex;
		flex-wrap: wrap;
		margin-inline-start: -1.25em;
	}
	.part {
		position: relative;
		padding-inline-start: 1.25em;
	}
	.part::before {
		content: '·';
		position: absolute;
		inset-inline-start: 0.45em;
		color: var(--bone-400);
	}
</style>
