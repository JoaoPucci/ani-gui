<!--
  The strip's line for the entry the viewer is part-way through, when
  it is another entry of the run (mockup, second round). It goes to
  that entry's show page; it does not play. Two variants:

  - `chip`: a link that looks like the thing it opens, the entry's
    poster beside "You're watching", its title and episode;
  - `sentence`: the first round's line, reworded so the link says
    where it goes ("Go to its page") instead of "Resume".
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import type { MockEntry } from './types';

	let {
		entry,
		episode,
		variant,
		label
	}: {
		entry: MockEntry;
		episode: number;
		variant: 'chip' | 'sentence';
		label: (e: MockEntry) => string;
	} = $props();

	const href = $derived(resolve('/anime/[id]', { id: entry.id }));
</script>

{#if variant === 'chip'}
	<a class="chip" {href} style:--accent={accentFor(entry.id)} title={entry.title}>
		<span class="chip-poster">
			{#if imageProxyUrl(entry.poster)}
				<img src={imageProxyUrl(entry.poster)} alt="" loading="lazy" decoding="async" />
			{/if}
		</span>
		<span class="chip-text">
			<span class="chip-key">{m.watchorder_strip_watching_key()}</span>
			<span class="chip-title"
				>{label(entry)}<span class="chip-ep">{m.watchorder_meta_progress_open({ episode })}</span
				></span
			>
		</span>
		<span class="arrow" aria-hidden="true"><Icon name="arrow-right" size={16} /></span>
	</a>
{:else}
	<p class="sentence">
		<span class="sentence-text"
			>{m.watchorder_strip_watching_sentence({ title: label(entry), episode })}</span
		>
		<a class="text-btn" {href}>{m.watchorder_strip_watching_go()}</a>
	</p>
{/if}

<style>
	.chip {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-3);
		max-inline-size: min(100%, 26rem);
		padding: var(--space-1) var(--space-3) var(--space-1) var(--space-1);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		color: inherit;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background-color var(--dur-fast) var(--ease-out-soft);
	}
	.chip:hover {
		border-color: var(--bone-300);
		background: var(--ink-100);
	}
	.chip:focus-visible,
	.text-btn:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 2px;
	}
	.chip-poster {
		position: relative;
		inline-size: 1.75rem;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: 3px;
		background: var(--ink-200);
		border-block-end: 2px solid var(--accent);
	}
	.chip-poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.chip-text {
		display: grid;
		min-inline-size: 0;
	}
	.chip-key {
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.chip-title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--type-meta);
		font-weight: 500;
		color: var(--bone-100);
	}
	.chip-ep {
		margin-inline-start: var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
	}
	.arrow {
		display: inline-flex;
		color: var(--bone-300);
		transform: scaleX(var(--flip, 1));
	}
	:global([dir='rtl']) .chip {
		--flip: -1;
	}
	.chip:hover .arrow {
		color: var(--bone-100);
	}

	.sentence {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		margin: 0;
		min-inline-size: 0;
		font-size: var(--type-meta);
	}
	.sentence-text {
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.text-btn {
		flex: none;
		font-weight: 600;
		color: var(--bone-100);
		text-decoration: underline;
		text-decoration-color: color-mix(in oklab, var(--bone-100) 30%, transparent);
		text-underline-offset: 0.25em;
		transition: text-decoration-color var(--dur-fast) var(--ease-out-soft);
	}
	.text-btn:hover {
		text-decoration-color: var(--bone-100);
	}
</style>
