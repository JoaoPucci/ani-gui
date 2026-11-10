<!--
  A row of poster cards still loading: its header, and placeholder
  cards that take the room the cards they stand in for will take — a
  poster, a title wrapped to two lines (PosterCard clamps there, and a
  strip is as tall as its tallest card, which in a row of twenty almost
  always has a wrapped title) and the meta line, on the same line boxes
  as PosterCard's. So a row is the same height loading and loaded, and
  filling it in moves nothing below it.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import Strip from '$lib/components/Strip.svelte';

	interface Props {
		eyebrow: string;
		caption: string;
		/** The surface the loaded row sits on (Strip's `surface`). */
		surface?: 'plain' | 'band';
		/** The loaded row's header actions, when it has them: drawn here
		 *  too, so the header is the same height loading and loaded. */
		headerActions?: Snippet;
	}

	let { eyebrow, caption, surface = 'plain', headerActions }: Props = $props();
</script>

<Strip {eyebrow} {caption} {surface} {headerActions}>
	{#each Array.from({ length: 8 }, (_, k) => k) as i (i)}
		<div class="skel-card" aria-hidden="true">
			<div class="skel-poster"></div>
			<div class="skel-body">
				<span class="skel-title" style="--bar: 85%;">&nbsp;</span>
				<span class="skel-title" style="--bar: 55%;">&nbsp;</span>
				<span class="skel-meta">&nbsp;</span>
			</div>
		</div>
	{/each}
</Strip>

<style>
	/* The strip sizes a placeholder card like the cards it stands in
	   for; the body repeats PosterCard's card-body, card-title and
	   card-meta boxes (padding, font size, line height), each line
	   carried by a non-breaking space with a pulsing bar drawn over it
	   from the line's start edge. */
	.skel-card {
		display: block;
	}
	.skel-body {
		display: block;
		padding-block-start: var(--space-3);
	}
	.skel-title,
	.skel-meta {
		position: relative;
	}
	.skel-title::before,
	.skel-meta::before {
		content: '';
		position: absolute;
		inset-inline-start: 0;
		inset-block: 0;
		margin-block: auto;
		inline-size: var(--bar, 100%);
		background: var(--ink-100);
		border-radius: 2px;
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.skel-title {
		display: block;
		font-size: 0.9375rem;
		line-height: 1.3;
	}
	.skel-title::before {
		block-size: 0.75rem;
	}
	.skel-meta {
		display: inline-flex;
		inline-size: 40%;
		margin-block-start: var(--space-2);
		font-size: 0.75rem;
	}
	.skel-meta::before {
		block-size: 0.6rem;
	}
	.skel-poster {
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.55;
		}
	}
</style>
