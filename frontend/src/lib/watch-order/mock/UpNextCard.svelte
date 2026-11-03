<!--
  The player's "Up next" card at a finale (mockup). It takes the skip
  pill's corner, surface and accent border, and grows into a card:

  - `poster`: the next entry's poster beside its title and meta;
  - `still`: the next entry's banner above its title, wider.

  It waits for a click, which opens the next entry's show page; it
  never takes focus by itself, Esc dismisses it while it has focus,
  and it is announced once, politely.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import { settle } from '$lib/motion/settle';
	import MetaLine from './MetaLine.svelte';
	import { metaLine } from '../meta';
	import type { MockEntry, MockRun } from './types';
	import type { UpNext } from './view';

	let {
		run,
		upNext,
		variant,
		today,
		ondismiss
	}: {
		run: MockRun;
		upNext: Exclude<UpNext, { kind: 'none' }>;
		variant: 'poster' | 'still';
		today: string;
		ondismiss: () => void;
	} = $props();

	const entries = $derived(upNext.kind === 'choices' ? upNext.entries : [upNext.entry]);
	const accent = $derived(accentFor(entries[0].id));
	const key = $derived(
		upNext.kind === 'choices'
			? m.watchorder_upnext_choices()
			: upNext.kind === 'unaired'
				? m.watchorder_upnext_coming()
				: m.watchorder_upnext_eyebrow()
	);
	const image = (e: MockEntry) =>
		imageProxyUrl(variant === 'still' ? (e.cover ?? e.poster) : e.poster);

	// Filled after mount: a live region that already holds its text
	// when it appears is not announced by every screen reader.
	let announce = $state('');
	onMount(() => {
		announce = m.watchorder_upnext_announce({ title: entries[0].title });
	});

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.stopPropagation();
			ondismiss();
		}
	}
</script>

{#snippet inner(e: MockEntry, compact: boolean)}
	<span class="art" class:compact>
		{#if image(e)}<img src={image(e)} alt="" />{/if}
	</span>
	<span class="text">
		{#if !compact}<span class="key">{key}</span>{/if}
		<span class="title">{e.title}</span>
		<MetaLine segments={metaLine(e, run.progress[e.id], today)} />
	</span>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="up-next"
	class:still={variant === 'still'}
	style:--accent={accent}
	{onkeydown}
	transition:settle={{ duration: 200 }}
>
	<span class="sr-only" role="status">{announce}</span>
	<button
		type="button"
		class="dismiss"
		aria-label={m.watchorder_upnext_dismiss()}
		onclick={ondismiss}
	>
		<svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
			<path
				d="M4 4l8 8M12 4l-8 8"
				stroke="currentColor"
				stroke-width="1.8"
				stroke-linecap="round"
			/>
		</svg>
	</button>
	{#if upNext.kind === 'choices'}
		<p class="key list-key">{key}</p>
		<ul class="choices">
			{#each entries as e (e.id)}
				<li>
					<a class="row" href={resolve('/anime/[id]', { id: e.id })} title={e.title}>
						{@render inner(e, true)}
					</a>
				</li>
			{/each}
		</ul>
	{:else if upNext.kind === 'unaired'}
		<div class="row single">{@render inner(upNext.entry, false)}</div>
	{:else}
		<a
			class="row single"
			href={resolve('/anime/[id]', { id: upNext.entry.id })}
			title={upNext.entry.title}
		>
			{@render inner(upNext.entry, false)}
		</a>
	{/if}
</div>

<style>
	.up-next {
		position: absolute;
		z-index: 4;
		inset-inline-end: calc(1.5rem + var(--player-letterbox-x, 0px));
		inset-block-end: calc(5rem + var(--player-letterbox-y, 0px));
		inline-size: min(22rem, calc(100% - 3rem));
		padding: var(--space-3);
		color: var(--bone-100);
		background: rgb(15 15 17 / 0.92);
		border: 1px solid color-mix(in oklab, var(--accent) 70%, transparent);
		border-radius: var(--radius-card);
		box-shadow:
			0 1px 0 rgb(255 255 255 / 0.08) inset,
			0 8px 24px rgb(0 0 0 / 0.45),
			0 2px 8px rgb(0 0 0 / 0.3);
	}
	.up-next.still {
		inline-size: min(20rem, calc(100% - 3rem));
	}
	.row {
		display: grid;
		grid-template-columns: 4.5rem minmax(0, 1fr);
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-2);
		margin: calc(-1 * var(--space-2));
		border-radius: var(--radius-control);
		color: inherit;
		transition: background-color var(--dur-fast) var(--ease-out-soft);
	}
	a.row:hover {
		background: color-mix(in oklab, var(--accent) 92%, #000);
	}
	a.row:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 2px;
		box-shadow: none;
	}
	.still .row.single {
		grid-template-columns: minmax(0, 1fr);
	}
	.art {
		position: relative;
		display: block;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: calc(var(--radius-card) - 2px);
		background: var(--ink-100);
	}
	.art.compact {
		inline-size: 2.75rem;
	}
	.still .art {
		aspect-ratio: 16 / 9;
	}
	.still .art.compact {
		inline-size: 4.5rem;
	}
	.art img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.text {
		display: grid;
		gap: var(--space-1);
		min-inline-size: 0;
		padding-inline-end: var(--space-5);
	}
	.still .single .text {
		padding-inline-end: 0;
	}
	.key {
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: color-mix(in oklab, var(--bone-100) 75%, transparent);
	}
	.list-key {
		margin: 0 0 var(--space-3);
		padding-inline-end: var(--space-6);
	}
	/* Three lines: the card is narrow, and the words that tell the
	   next entry apart come last. */
	.title {
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-size: 1rem;
		font-weight: 600;
		line-height: 1.3;
	}
	.choices {
		display: grid;
		gap: var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.choices .row {
		grid-template-columns: auto minmax(0, 1fr);
	}
	.choices .title {
		font-size: var(--type-meta);
	}
	.dismiss {
		position: absolute;
		z-index: 1;
		inset-block-start: var(--space-2);
		inset-inline-end: var(--space-2);
		display: grid;
		place-items: center;
		inline-size: 1.75rem;
		block-size: 1.75rem;
		border-radius: var(--radius-pill);
		color: var(--bone-300);
		background: rgb(15 15 17 / 0.7);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.dismiss:hover {
		color: var(--bone-100);
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
