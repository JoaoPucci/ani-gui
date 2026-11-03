<!--
  The strip's card for the entry the viewer is part-way through, when
  it is another entry of the run: a link that looks like the thing it
  opens, the entry's poster beside "You're watching", its title and
  episode. It goes to that entry's show page; it does not play.
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
		label,
		bare = false
	}: {
		entry: MockEntry;
		episode: number;
		label: (e: MockEntry) => string;
		/** No resting border (the centred footers). */
		bare?: boolean;
	} = $props();

	const href = $derived(resolve('/anime/[id]', { id: entry.id }));
</script>

<a class="chip" class:bare {href} style:--accent={accentFor(entry.id)} title={entry.title}>
	<span class="chip-poster">
		{#if imageProxyUrl(entry.poster)}
			<img src={imageProxyUrl(entry.poster)} alt="" loading="lazy" decoding="async" />
		{/if}
	</span>
	<span class="chip-text">
		<span class="chip-key">{m.watchorder_strip_watching_key()}</span>
		<span class="chip-line">
			<span class="chip-title">{label(entry)}</span>
			<span class="chip-ep">{m.watchorder_meta_progress_open({ episode })}</span>
		</span>
	</span>
	<span class="arrow" aria-hidden="true"><Icon name="arrow-right" size={16} /></span>
</a>

<style>
	.chip {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-3);
		max-inline-size: min(100%, 26rem);
		padding-block: var(--space-1);
		padding-inline: var(--space-1) var(--space-3);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		color: inherit;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background-color var(--dur-fast) var(--ease-out-soft);
	}
	.chip:hover,
	.chip:focus-visible {
		border-color: var(--bone-300);
		background: var(--ink-100);
	}
	/* Bare: no resting border, the same plate as the poster stack on
	   hover and focus, so the two read as one pair in the footer. */
	.chip.bare {
		border-color: transparent;
	}
	.chip.bare:hover,
	.chip.bare:focus-visible {
		border-color: transparent;
	}
	.chip:focus-visible {
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
	/* The title gives way before the episode does. */
	.chip-line {
		display: flex;
		align-items: baseline;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.chip-title {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--type-meta);
		font-weight: 500;
		color: var(--bone-100);
	}
	.chip-ep {
		flex: none;
		white-space: nowrap;
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
		transition:
			transform var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	:global([dir='rtl']) .chip {
		--flip: -1;
	}
	.chip:hover .arrow,
	.chip:focus-visible .arrow {
		color: var(--bone-100);
		transform: scaleX(var(--flip, 1)) translateX(2px);
	}
</style>
