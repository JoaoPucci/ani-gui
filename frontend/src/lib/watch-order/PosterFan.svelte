<!--
  The franchise's poster stack, fanned on one arc (./stack-fan): the
  strip's link to the franchise page and the player's franchise card
  both draw it. Its box holds every card whole, at rest and fanned out
  (./stack-fan's fanBox), so the text beside it never sits under a card. The element that holds it
  carries `data-fan-host`: hovering or focusing that element opens the
  fan outwards along the arc. With no poster the stack is three blank
  frames, the front one carrying `total` (none when it is null).
  Under reduced motion the fan is drawn open and does not move; in a
  right-to-left page it spreads the other way.
-->
<script lang="ts">
	import { imageProxyUrl } from '$lib/api';
	import { reducedMotion } from '$lib/motion/reduced-motion.svelte';
	import { numeral } from './meta';
	import { CARD_INLINE_REM, FAN_RADIUS_REM, fanBox, fanPoses, type StackCard } from './stack-fan';

	let {
		cards,
		total
	}: {
		cards: readonly StackCard[];
		/** The count the front blank frame carries; none when null. */
		total: number | null;
	} = $props();

	const box = fanBox();
	const open = $derived(fanPoses(cards.length, 'open'));
	const rest = $derived(reducedMotion.current ? open : fanPoses(cards.length, 'rest'));
	// A poster that fails to load is drawn as a blank frame, not an empty card.
	let broken: Record<string, boolean> = $state({});
	const showsPoster = (c: StackCard) =>
		c.kind === 'poster' && !broken[c.entry.id] && imageProxyUrl(c.entry.poster) !== null;
</script>

<span
	class="fan"
	style:--radius="{FAN_RADIUS_REM}rem"
	style:--card="{CARD_INLINE_REM}rem"
	style:--box-inline="{box.inline}rem"
	style:--box-block="{box.block}rem"
	style:--foot="{box.foot}rem"
	aria-hidden="true"
>
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
			{:else if card.kind === 'blank' && i === cards.length - 1 && total !== null}
				<span class="blank-count" data-testid="fan-count">{numeral(total)}</span>
			{/if}
		</span>
	{/each}
</span>

<style>
	/* The fan's spread follows the reading direction. */
	.fan {
		--flip: 1;
		position: relative;
		display: block;
		flex: none;
		inline-size: var(--box-inline);
		block-size: var(--box-block);
	}
	:global([dir='rtl']) .fan {
		--flip: -1;
	}
	/* Every card stands at the middle of the box and is rotated about
	   the arc's centre, FAN_RADIUS_REM below its foot, so the
	   transition between the rest and open angles moves it along the
	   arc. The box is ./stack-fan's fanBox, which holds every card whole
	   at rest and fanned out. */
	.fan-card {
		position: absolute;
		inset-block-end: var(--foot);
		inset-inline-start: calc(50% - var(--card) / 2);
		inline-size: var(--card);
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
	:global([data-fan-host]:hover) .fan-card,
	:global([data-fan-host]:focus-visible) .fan-card {
		transform: translateY(var(--open-y)) rotate(calc(var(--open-r) * var(--flip)));
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	:global([data-fan-host]:hover) .fan-card:not(.blank),
	:global([data-fan-host]:focus-visible) .fan-card:not(.blank) {
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
	:global([data-fan-host]:hover) .blank-count,
	:global([data-fan-host]:focus-visible) .blank-count {
		transform: rotate(calc(-1 * var(--open-r) * var(--flip)));
	}
	.fan-card img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
</style>
