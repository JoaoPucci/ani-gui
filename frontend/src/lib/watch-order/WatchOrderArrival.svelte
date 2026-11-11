<!--
  The show page's watch-order strip, or its franchise link alone, and
  the room held for either while the franchise walk runs, so neither
  pushes the page when it lands (`./arrival` has the rules).

  The held room is an outline of what the walk's first hop foretold.
  When the walk answers with something to draw, the outline goes at
  once and the strip fades in where it stood, from the outline's
  height to its own; when it answers with nothing, the outline fades,
  then folds away. Where the outline is above the viewport, it goes at
  once and the page scrolls back by the height the change gave back,
  so what the reader is reading stays put. Every step is a
  `$lib/motion` transition, so reduced motion makes it instant.
-->
<script lang="ts">
	import { tick } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, reducible, slide } from '$lib/motion/transitions';
	import StripFranchiseLink from './StripFranchiseLink.svelte';
	import StripReserve from './StripReserve.svelte';
	import StripTrio from './StripTrio.svelte';
	import {
		arriveCss,
		arriveOf,
		heldShape,
		holdScroll,
		leaveOf,
		type FirstHop,
		type Gone
	} from './arrival';
	import type { FranchiseLink, WatchOrderPage } from './page-model';

	let {
		model,
		link,
		hop,
		answered,
		id,
		today
	}: {
		model: WatchOrderPage | null;
		link: FranchiseLink | null;
		/** The walk's first hop, if it came in time; null if not. */
		hop: FirstHop | null;
		/** Whether the walk (and the order setting) have answered. */
		answered: boolean;
		id: string;
		today: string;
	} = $props();

	const held = $derived(heldShape(hop, answered));
	const drawn = $derived(model !== null || link !== null);
	let heldEl = $state<HTMLElement | null>(null);
	let drawnEl = $state<HTMLElement | null>(null);
	/** Where the held room was when the walk answered, measured before it goes. */
	let gone: Gone | null = null;

	$effect.pre(() => {
		if (held !== null || heldEl === null) return;
		const rect = heldEl.getBoundingClientRect();
		const was: Gone = { top: rect.top, height: rect.height };
		gone = was;
		if (was.top >= 0) return;
		const hold = holdScroll(
			{
				get scrollY() {
					return window.scrollY;
				},
				scrollBy: (x, y) => window.scrollBy(x, y),
				scrollTo: (x, y) => window.scrollTo(x, y)
			},
			was
		);
		void tick().then(() => hold(drawnEl?.getBoundingClientRect().height ?? 0));
	});

	const leave = reducible((node: Element) => {
		const l = leaveOf(drawn, gone);
		return slide(node, { delay: l.fade, duration: l.collapse, easing: cubicOut });
	});
	const leaveFade = reducible((node: Element) =>
		fade(node, { duration: leaveOf(drawn, gone).fade })
	);
	const arrive = reducible((node: Element) => {
		const a = arriveOf(gone);
		if (a.fade === 0) return { duration: 0 };
		const to = node.getBoundingClientRect().height;
		return { duration: a.fade, easing: cubicOut, css: (t: number) => arriveCss(a.from, to, t) };
	});
</script>

{#if model}
	<div class="watch-order" bind:this={drawnEl} in:arrive>
		<StripTrio {model} {id} {today} />
	</div>
{:else if link}
	<div class="watch-order watch-order-link" bind:this={drawnEl} in:arrive>
		<StripFranchiseLink run={link.run} front={link.front} {id} complete={link.complete} />
	</div>
{:else if held}
	<div
		class="watch-order-held"
		bind:this={heldEl}
		out:leave
		aria-hidden="true"
		data-testid="wo-reserve"
	>
		<div out:leaveFade><StripReserve shape={held} /></div>
	</div>
{/if}

<style>
	/* Padding, not margin, so a block's measured height is all the room
	   it takes, and the held room and the strip take the same. */
	.watch-order,
	.watch-order-held {
		padding-block-start: var(--space-7);
	}
	/* The franchise link without a strip: on the page's text column,
	   its hover plate reaching a little past it as the strip's band does. */
	.watch-order-link {
		padding-inline: var(--space-8);
	}
	.watch-order-link > :global(.stack) {
		margin-inline-start: calc(-1 * var(--space-3));
	}
</style>
