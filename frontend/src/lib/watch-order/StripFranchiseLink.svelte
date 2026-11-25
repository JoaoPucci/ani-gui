<!--
  The strip's way to the franchise page, at the end of its footer, or
  on its own on a show page with no strip (`$lib/api` `LinkWire`): posters from different parts of the franchise,
  fanned on one arc (./PosterFan), beside "See the whole franchise"
  and what the page holds. Hovering or focusing the link opens the fan
  and lays a faint plate under the whole link, which is one target.
  With no poster the stack is three blank frames, the front one
  carrying the number of titles the franchise page lists (none for a
  franchise read in part).
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { WatchOrderStack } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import PosterFan from './PosterFan.svelte';
	import { stackCards } from './stack-fan';
	import { stackPartsText, stackTotal } from './strip-copy';
	import { m } from '$lib/paraglide/messages';

	let {
		stack,
		id,
		complete
	}: {
		/** The stack's posters and counts, fronted by a main-story entry. */
		stack: WatchOrderStack;
		id: string;
		/** False for a franchise read in part: the stack gives no counts. */
		complete: boolean;
	} = $props();

	const summary = $derived(stack.summary);
	const parts = $derived(stackPartsText(summary, complete));
	const total = $derived(stackTotal(summary, complete));
	const cards = $derived(stackCards(summary.posters, 3));
</script>

<a
	class="stack"
	data-fan-host
	data-onboard="watch-order-franchise"
	href={resolve('/anime/[id]/franchise', { id })}
>
	<PosterFan {cards} {total} />
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

<style>
	.stack:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 2px;
		/* The outline is the ring: the app's (tokens.css) drew a second. */
		box-shadow: none;
	}

	/* The arrow follows the reading direction. */
	.stack {
		--flip: 1;
	}
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
	.stack:hover .arrow,
	.stack:focus-visible .arrow {
		color: var(--bone-100);
		transform: scaleX(var(--flip)) translateX(2px);
	}

	/* The whole link is one target. Its padding is the highlight's
	   area. */
	.stack {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-4);
		max-inline-size: 100%;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-card);
		color: inherit;
		transition:
			background-color var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	/* The highlight: a faint plate one step up the page's ink under the
	   whole link, the hover the strip's own tiles use. Focus shows the
	   same state as hover, with the focus ring on top. */
	.stack:hover,
	.stack:focus-visible {
		background: var(--ink-100);
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
