<!--
  SearchProgress — an indeterminate progress bar that lives at the top
  of the search route's content area while a query is in flight.

  Why this exists: today, the /search route renders skeletons only on
  the first query (when `results === null`). On a subsequent query
  the previous result set stays on screen until the new one arrives —
  the page gives no visible cue that work is happening, so a user can
  mistake "we're searching" for "we're done." A label exists but is
  easy to miss; a thin moving bar across the top of the content area
  matches the YouTube / GitHub stale-while-revalidate convention and
  is unmissable without being heavy.

  Renders only when `busy === true`. The {#if} fully unmounts the bar
  when idle so the keyframe restarts on the next search rather than
  drifting through paused state.
-->
<script lang="ts">
	interface Props {
		busy: boolean;
	}
	let { busy }: Props = $props();
</script>

{#if busy}
	<div class="search-progress" role="presentation" aria-hidden="true">
		<div class="search-progress-bar"></div>
	</div>
{/if}

<style>
	.search-progress {
		/* Pinned to the viewport edge directly below the topbar.
		   `.page` has `max-inline-size: var(--content-max-wide)` and
		   `margin-inline: auto`, so a sticky bar inside the route
		   would inherit that centered-narrow box. Fixed-positioning
		   gives the bar the whole viewport width, the YouTube /
		   GitHub feel the user asked for. */
		position: fixed;
		/* Topbar's actual rendered height (padding 1rem block + the
		   2.75rem search input + 1px border-block-end) lands at
		   ~4.75rem. Position the bar so its 3px straddles the
		   topbar's 1px border instead of stacking below it — the
		   "two lines" look reads weird. */
		inset-block-start: calc(var(--topbar-h) + 0.375rem);
		/* Start after the left rail; rail width comes from the .shell
		   grid template in +layout.svelte. */
		inset-inline-start: var(--rail-width);
		inset-inline-end: 0;
		block-size: 3px;
		background: color-mix(in oklab, var(--ink-300) 60%, transparent);
		overflow: hidden;
		/* Above the topbar (z-index 15 in +layout.svelte) so even
		   the topbar's translucent backdrop bleed doesn't dim the
		   orange. */
		z-index: 20;
		pointer-events: none;
	}

	.search-progress-bar {
		block-size: 100%;
		inline-size: 30%;
		background: linear-gradient(
			to right,
			transparent 0%,
			var(--accent-persimmon) 50%,
			transparent 100%
		);
		animation: search-progress-slide 1.4s var(--ease-in-out, ease-in-out) infinite;
		transform: translateX(-100%);
		will-change: transform;
	}

	@keyframes search-progress-slide {
		0% {
			transform: translateX(-100%);
		}
		100% {
			transform: translateX(400%);
		}
	}

	@media (max-inline-size: 720px) {
		/* `.shell` collapses to one column at this breakpoint — the
		   rail moves into normal flow above the main area, so the
		   topbar is no longer at the viewport's top edge. Fixed
		   positioning relative to the viewport would land inside
		   the mobile chrome instead of below the topbar. Switch to
		   sticky inside `.page` (where the component is rendered)
		   so the bar rides the topbar's bottom regardless of where
		   the chrome ended up. Negative margins cancel `.page`'s
		   padding so the bar spans the route content edge-to-edge. */
		.search-progress {
			position: sticky;
			inset-block-start: 0;
			inset-inline-start: 0;
			inset-inline-end: 0;
			margin-block-start: calc(var(--space-7) * -1);
			margin-inline: calc(var(--space-8) * -1);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.search-progress-bar {
			/* Static accent bar when the OS asks us not to animate.
			   Still visible, just not slithering. */
			animation: none;
			transform: translateX(0);
			inline-size: 100%;
			opacity: 0.6;
			background: var(--accent-persimmon);
		}
	}
</style>
