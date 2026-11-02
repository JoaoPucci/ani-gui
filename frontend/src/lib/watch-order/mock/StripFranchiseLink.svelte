<!--
  The strip's way to the franchise page (mockup, second round), at the
  end of the strip's footer, in two variants:

  - `button`: the show page's outline button (.btn.btn-outline) at its
    full size, with the number of entries the franchise page lists and
    the app's arrow;
  - `stack`: three posters from different parts of the franchise,
    fanned, beside what the page holds. No border of its own, so it
    does not read as a sibling of the watching card beside it. The
    posters spread a little on hover or focus.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import { m } from '$lib/paraglide/messages';
	import { franchiseSummary } from './strip-view';
	import type { MockRun } from './types';

	let { run, id, variant }: { run: MockRun; id: string; variant: 'button' | 'stack' } = $props();

	const summary = $derived(franchiseSummary(run));
	const parts = $derived(
		[
			m.watchorder_strip_franchise_stack_main({ count: summary.main }),
			summary.films > 0 ? m.watchorder_strip_franchise_stack_films({ count: summary.films }) : null,
			summary.sides > 0 ? m.watchorder_strip_franchise_stack_sides({ count: summary.sides }) : null,
			run.recaps.length > 0
				? m.watchorder_strip_franchise_stack_recaps({ count: run.recaps.length })
				: null
		].filter((p) => p !== null)
	);
</script>

{#if variant === 'button'}
	<a class="btn-franchise" href={resolve('/anime/[id]/franchise', { id })}>
		<span>{m.watchorder_strip_franchise_link()}</span>
		<span class="count">{m.watchorder_franchise_count({ count: summary.total })}</span>
		<span class="arrow" aria-hidden="true"><Icon name="arrow-right" size={16} /></span>
	</a>
{:else}
	<a class="stack" href={resolve('/anime/[id]/franchise', { id })}>
		<span class="fan" aria-hidden="true">
			{#each summary.posters as p, i (p.id)}
				<span class="fan-card" style:--i={i}>
					{#if imageProxyUrl(p.poster)}
						<img src={imageProxyUrl(p.poster)} alt="" loading="lazy" decoding="async" />
					{/if}
				</span>
			{/each}
		</span>
		<span class="stack-text">
			<span class="stack-title">{m.watchorder_strip_franchise_stack_title()}</span>
			<span class="stack-meta">
				{#each parts as part, i (i)}{#if i > 0}<span class="dot" aria-hidden="true">·</span
						>{/if}<span>{part}</span>{/each}
			</span>
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
	.stack:hover .arrow {
		color: var(--bone-100);
		transform: scaleX(var(--flip)) translateX(2px);
	}

	.stack {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-4);
		max-inline-size: 100%;
		padding: var(--space-1) var(--space-2);
		margin: calc(-1 * var(--space-1)) calc(-1 * var(--space-2));
		border-radius: var(--radius-card);
		color: inherit;
		transition: background-color var(--dur-fast) var(--ease-out-soft);
	}
	.stack:hover {
		background: var(--ink-100);
	}
	.fan {
		position: relative;
		inline-size: 4.75rem;
		block-size: 3.25rem;
	}
	.fan-card {
		position: absolute;
		inset-block-start: 0;
		inset-inline-start: calc(var(--i) * 1.125rem);
		inline-size: 2.3rem;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: 4px;
		background: var(--ink-200);
		box-shadow:
			0 0 0 1px var(--ink-000),
			var(--shadow-card-rest);
		transform: rotate(calc((var(--i) - 1) * 5deg * var(--flip)));
		transform-origin: 50% 100%;
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	.stack:hover .fan-card,
	.stack:focus-visible .fan-card {
		transform: rotate(calc((var(--i) - 1) * 9deg * var(--flip)))
			translateX(calc((var(--i) - 1) * 3px * var(--flip)));
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
	.stack-meta {
		display: flex;
		flex-wrap: wrap;
		column-gap: var(--space-2);
		font-size: var(--type-micro);
		color: var(--bone-300);
	}
	.dot {
		color: var(--bone-400);
	}
</style>
