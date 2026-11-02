<!--
  The show page's watch-order strip (mockup). Two arrangements:

  - `rail`: the whole run as a Strip of RunCards, scrolled so the entry
    being viewed sits late in the window with what came before it in
    sight; branches, a loop and side entries open with a key and a
    hairline (railItems);
  - `trio`: what comes before, this entry's place, and what comes
    after, as three columns of landscape tiles.

  Both share the Strip eyebrow ("Watch order — 04 of 05"), the "You're
  on … · Resume" line and the link to the franchise page.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import Strip from '$lib/components/Strip.svelte';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import MetaLine from './MetaLine.svelte';
	import MockSwitch from './MockSwitch.svelte';
	import RunCard from './RunCard.svelte';
	import { metaLine, numeral, railKeyText } from './meta';
	import { loopSize, placement, railItems } from './order';
	import type { WatchOrderMockPrefs } from './prefs';
	import { shortTitle } from './short-title';
	import type { MockEntry, MockRun } from './types';
	import { currentIn, scrollLeftFor } from './view';

	let {
		run,
		id,
		prefs,
		today,
		onchoose
	}: {
		run: MockRun;
		id: string;
		prefs: WatchOrderMockPrefs;
		today: string;
		onchoose: (next: WatchOrderMockPrefs) => void;
	} = $props();

	const here = $derived(placement(run, id, prefs.order));
	const items = $derived(railItems(run, prefs.order));
	const loop = $derived(loopSize(run, id, prefs.order));
	const titles = $derived(run.entries.map((e) => e.title));
	const onNow = $derived(currentIn(run, prefs.order));
	const caption = $derived.by(() => {
		if (here.position !== null && here.total !== null)
			return m.watchorder_strip_position({
				position: numeral(here.position),
				total: numeral(here.total)
			});
		if (here.position !== null) return numeral(here.position);
		return here.inCycle
			? m.watchorder_strip_unnumbered_cycle({ count: loop })
			: m.watchorder_strip_unnumbered_branch();
	});
	const label = (e: MockEntry) =>
		prefs.titles === 'short' ? shortTitle(e.title, titles) : e.title;

	let wrap: HTMLDivElement | undefined = $state();
	// Bring the entry being viewed into the rail, three quarters of the
	// way along, so the entries before it stay in sight. The scroller
	// is Strip's own; the offset is computed, not scrolled into view,
	// so the page never moves vertically.
	$effect(() => {
		void id;
		void prefs.order;
		const card = wrap?.querySelector<HTMLElement>('[aria-current="page"]');
		const scroller = card?.closest<HTMLElement>('.strip-scroll');
		if (!card || !scroller) return;
		const start =
			card.getBoundingClientRect().left -
			scroller.getBoundingClientRect().left +
			scroller.scrollLeft;
		scroller.scrollLeft = scrollLeftFor(scroller, start, card.offsetWidth, 0.75);
	});
</script>

{#snippet trailing()}
	{#if onNow && onNow.entry.id !== id}
		<span class="on-now">
			<span class="on-now-text"
				>{m.watchorder_strip_youre_on({ title: label(onNow.entry), episode: onNow.episode })}</span
			>
			<a class="text-btn" href={resolve('/anime/[id]', { id: onNow.entry.id })}
				>{m.watchorder_strip_resume()}</a
			>
		</span>
	{/if}
	<a class="text-btn" href={resolve('/anime/[id]/franchise', { id })}
		>{m.watchorder_strip_franchise_link()}</a
	>
{/snippet}

{#snippet tile(e: MockEntry)}
	<a
		class="tile"
		style:--accent={accentFor(e.id)}
		href={resolve('/anime/[id]', { id: e.id })}
		title={e.title}
	>
		<span class="tile-poster">
			{#if imageProxyUrl(e.poster)}
				<img src={imageProxyUrl(e.poster)} alt="" loading="lazy" decoding="async" />
			{/if}
		</span>
		<span class="tile-text">
			<span class="tile-title">{label(e)}</span>
			{#if label(e) !== e.title}<span class="sr-only">{e.title}</span>{/if}
			<MetaLine segments={metaLine(run, e, today)} />
		</span>
	</a>
{/snippet}

<div class="watch-order" bind:this={wrap}>
	{#if prefs.switcher}
		<div class="switch-row">
			<MockSwitch {prefs} keys={['order', 'strip', 'placement', 'titles']} {onchoose} />
		</div>
	{/if}
	{#if prefs.strip === 'rail'}
		<Strip
			eyebrow={m.watchorder_strip_eyebrow()}
			{caption}
			cardWidth="9.5rem"
			padBlock="var(--space-2) var(--space-3)"
			headerTrailing={trailing}
		>
			{#each items as item (item.entry.id)}
				<RunCard
					{run}
					entry={item.entry}
					position={item.position}
					label={label(item.entry)}
					current={item.entry.id === id}
					keyText={railKeyText(item.key, item.entry.id === id, label)}
					divider={item.divider}
					inLoop={item.inLoop}
					{today}
				/>
			{/each}
		</Strip>
	{:else}
		<section class="trio" aria-label={m.watchorder_strip_eyebrow()}>
			<header class="trio-header">
				<h2 class="eyebrow">
					<span class="eyebrow-key">{m.watchorder_strip_eyebrow()}</span>
					{#if here.position === null}
						<span class="eyebrow-rule" aria-hidden="true"></span>
						<span class="eyebrow-value">{caption}</span>
					{:else}
						<span class="sr-only">{caption}</span>
					{/if}
				</h2>
				<div class="trio-trailing">{@render trailing()}</div>
			</header>
			<div class="trio-grid">
				<div class="col">
					<p class="col-key">
						{here.before.length > 1
							? m.watchorder_strip_join({ count: here.before.length })
							: m.watchorder_strip_before()}
					</p>
					{#each here.before as e (e.id)}{@render tile(e)}{:else}
						<p class="col-empty">{m.watchorder_strip_first()}</p>
					{/each}
				</div>
				<div class="col col-this" style:--accent={accentFor(id)}>
					<p class="col-key col-key-here">{m.watchorder_franchise_here()}</p>
					<p class="this-numeral" aria-hidden="true">
						{here.position !== null ? numeral(here.position) : '—'}
					</p>
					{#if here.total !== null}
						<p class="this-total" aria-hidden="true">
							{m.watchorder_strip_of_total({ total: numeral(here.total) })}
						</p>
					{/if}
				</div>
				<div class="col">
					<p class="col-key">
						{here.after.length > 1
							? m.watchorder_strip_ways({ count: here.after.length })
							: m.watchorder_strip_after()}
					</p>
					{#each here.after as e (e.id)}{@render tile(e)}{:else}
						<p class="col-empty">{m.watchorder_strip_last()}</p>
					{/each}
				</div>
			</div>
		</section>
	{/if}
</div>

<style>
	.watch-order {
		margin-block-start: var(--space-7);
	}
	.switch-row {
		padding-inline: var(--space-8);
		margin-block-end: var(--space-4);
	}
	/* Strip's header, here carrying a resume line and a link: it wraps
	   rather than squeezing the eyebrow, which stays on one line in
	   every locale. */
	.watch-order :global(.strip-header) {
		flex-wrap: wrap;
		row-gap: var(--space-3);
	}
	.watch-order :global(.strip-header .eyebrow),
	.trio-header .eyebrow {
		flex: none;
		white-space: nowrap;
	}
	/* The rail's own bottom margin is the page's section rhythm. */
	.watch-order :global(.strip) {
		margin-block-end: 0;
	}
	.on-now {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-3);
		min-inline-size: 0;
	}
	.on-now-text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 28rem;
		font-size: var(--type-meta);
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.text-btn {
		flex: none;
		font-size: var(--type-meta);
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
	.on-now + .text-btn {
		margin-inline-start: var(--space-4);
	}

	.trio {
		padding-inline: var(--space-8);
	}
	.trio-header {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3) var(--space-5);
		margin-block-end: var(--space-4);
	}
	.trio-trailing {
		display: inline-flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.trio-grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
		gap: var(--space-6);
		align-items: start;
	}
	.col {
		display: grid;
		gap: var(--space-3);
		align-content: start;
	}
	.col-key {
		margin: 0;
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.col-key-here {
		color: var(--bone-100);
	}
	.this-total {
		margin: 0;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.col-empty {
		margin: 0;
		font-size: var(--type-meta);
		color: var(--bone-400);
	}
	.col-this {
		justify-items: center;
		padding-inline: var(--space-5);
		border-inline: 1px solid var(--ink-200);
	}
	.this-numeral {
		margin: 0;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-numeral-xl);
		font-weight: 700;
		line-height: 1;
		color: var(--bone-100);
		border-block-end: 2px solid var(--accent);
		padding-block-end: var(--space-2);
	}
	.tile {
		display: grid;
		grid-template-columns: 3.5rem minmax(0, 1fr);
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-2);
		margin: calc(-1 * var(--space-2));
		border-radius: var(--radius-card);
		color: inherit;
		transition: background-color var(--dur-fast) var(--ease-out-soft);
	}
	.tile:hover {
		background: var(--ink-050);
	}
	.tile-poster {
		position: relative;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: calc(var(--radius-card) - 2px);
		background: var(--ink-100);
		box-shadow: var(--shadow-card-rest);
	}
	.tile-poster::after {
		content: '';
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: 2px;
		background: var(--accent);
		transform: scaleX(0);
		transform-origin: inline-start;
		transition: transform var(--dur-med) var(--ease-out-soft);
	}
	.tile:hover .tile-poster::after,
	.tile:focus-visible .tile-poster::after {
		transform: scaleX(1);
	}
	.tile-poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.tile-text {
		display: grid;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.tile-title {
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
	@media (max-width: 1100px) {
		.trio-grid {
			grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		}
		.col-this {
			display: none;
		}
	}
</style>
