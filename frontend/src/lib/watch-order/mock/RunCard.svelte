<!--
  A poster card in the watch-order rail and on the franchise shelf:
  PosterCard's anatomy at a smaller width, with the entry's position as
  an oversized numeral over the poster's foot. A key line above the
  title, reserved on every card so titles share a baseline, says
  "You are here" on the entry being viewed (marked, not linked: its
  accent rule stays drawn), or where a branch, a loop or a side entry
  starts; a hairline in the gap before the card marks that start.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import MetaLine from './MetaLine.svelte';
	import { metaLine, numeral } from '../meta';
	import type { MockEntry, MockRun } from './types';

	let {
		run,
		entry,
		position,
		label,
		current = false,
		keyText = null,
		divider = false,
		inLoop = false,
		today
	}: {
		run: MockRun;
		entry: MockEntry;
		position: number | null;
		/** The title shown: whole, or without the run's shared words. */
		label: string;
		current?: boolean;
		keyText?: string | null;
		divider?: boolean;
		inLoop?: boolean;
		today: string;
	} = $props();

	const poster = $derived(imageProxyUrl(entry.poster));
	// Format and year on one line, progress or count on the next: at
	// this width a single line wraps between its segments.
	const meta = $derived(metaLine(entry, run.progress[entry.id], today));
	const head = $derived(meta.slice(0, 2));
	const tail = $derived(meta.slice(2));
</script>

{#snippet body()}
	<span class="poster">
		{#if poster}
			<img src={poster} alt="" loading="lazy" decoding="async" />
		{:else}
			<span class="poster-placeholder" aria-hidden="true">{entry.title}</span>
		{/if}
		{#if position !== null}
			<span class="numeral" aria-hidden="true">{numeral(position)}</span>
		{/if}
		<span class="accent-rule" aria-hidden="true"></span>
	</span>
	<span class="card-body">
		<span class="key" class:key-here={current}>{keyText ?? ''}</span>
		<span class="card-title">{label}</span>
		{#if label !== entry.title}<span class="sr-only">{entry.title}</span>{/if}
		<MetaLine segments={head} />
		{#if tail.length > 0}<MetaLine segments={tail} />{/if}
	</span>
{/snippet}

{#if current}
	<div
		class="run-card current"
		class:divider
		class:in-loop={inLoop}
		style:--accent={accentFor(entry.id)}
		aria-current="page"
		title={entry.title}
	>
		{@render body()}
	</div>
{:else}
	<a
		class="run-card"
		class:divider
		class:in-loop={inLoop}
		style:--accent={accentFor(entry.id)}
		href={resolve('/anime/[id]', { id: entry.id })}
		title={entry.title}
	>
		{@render body()}
	</a>
{/if}

<style>
	.run-card {
		position: relative;
		display: block;
		color: inherit;
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	a.run-card:hover {
		transform: translateY(-4px);
	}
	.poster {
		position: relative;
		display: block;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: var(--radius-card);
		background: var(--ink-100);
		box-shadow: var(--shadow-card-rest);
		transition: box-shadow var(--dur-med) var(--ease-out-soft);
	}
	a.run-card:hover .poster {
		box-shadow: var(--shadow-card-hover);
	}
	.poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	/* The foot scrim the Continue card uses under its "EP N", so the
	   numeral reads on any poster without a chip. */
	.poster::after {
		content: '';
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: 45%;
		background: linear-gradient(
			180deg,
			transparent,
			color-mix(in oklab, var(--ink-000) 80%, transparent)
		);
		pointer-events: none;
	}
	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-3);
		font-size: var(--type-meta);
		text-align: center;
		color: var(--bone-200);
	}
	.numeral {
		position: absolute;
		z-index: 1;
		inset-inline-start: var(--space-3);
		inset-block-end: var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: 1.75rem;
		font-weight: 700;
		line-height: 1;
		color: var(--bone-100);
		filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.65));
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	/* The accent colours the numeral only under the pointer, as on the
	   Continue card; at rest it is never text on dark, so the entry
	   being viewed is marked by its drawn rule and ring instead. */
	a.run-card:hover .numeral {
		color: var(--accent);
	}
	.accent-rule {
		position: absolute;
		z-index: 1;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: 2px;
		background: var(--accent);
		transform: scaleX(0);
		transform-origin: inline-start;
		transition: transform var(--dur-med) var(--ease-out-soft);
	}
	a.run-card:hover .accent-rule,
	a.run-card:focus-visible .accent-rule,
	.current .accent-rule {
		transform: scaleX(1);
	}
	.current .poster {
		box-shadow:
			var(--shadow-card-rest),
			0 0 0 1px color-mix(in oklab, var(--accent) 70%, transparent);
	}
	a.run-card:focus-visible {
		box-shadow: none;
	}
	a.run-card:focus-visible .poster {
		box-shadow:
			var(--shadow-card-rest),
			0 0 0 2px var(--bone-100);
	}
	.card-body {
		display: grid;
		gap: var(--space-1);
		padding-block-start: var(--space-3);
	}
	/* A panel gutter: the hairline sits in the middle of the rail's
	   gap, the height of the poster. */
	.divider::before {
		content: '';
		position: absolute;
		inset-inline-start: calc(-1 * var(--space-5) / 2);
		inset-block: 0 auto;
		block-size: calc(100% - var(--space-4));
		inline-size: 1px;
		background: var(--ink-300);
	}
	/* A loop's members share a rule over their posters that runs on
	   across the gap, so the loop reads as one bracketed stretch. */
	.in-loop::after {
		content: '';
		position: absolute;
		inset-block-start: calc(-1 * var(--space-2));
		inset-inline: 0 calc(-1 * var(--space-5));
		block-size: 1px;
		background: var(--ink-300);
	}
	.key {
		display: block;
		min-block-size: 1lh;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		line-height: 1.4;
		color: var(--bone-300);
	}
	.key-here {
		color: var(--bone-100);
	}
	.card-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-size: 0.9375rem;
		font-weight: 500;
		line-height: 1.3;
		color: color-mix(in oklab, var(--bone-100) 90%, transparent);
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
