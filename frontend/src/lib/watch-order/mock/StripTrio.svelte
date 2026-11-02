<!--
  The strip's before-and-after arrangement (mockup, second round): what
  comes before, this entry's place, and what comes after, mirrored
  about the centre so the position sits between its neighbours rather
  than beside one of them. The before column reads towards the centre
  (poster on its inline-end side, text aligned to it), the after column
  away from it. The grid has three rows (keys, the join or fork line,
  the entries), so the three keys share a baseline whatever sits under
  them. An empty column drops its key and keeps the line saying why it
  is empty.

  Everything sits on a faint band, one step of the page's own ink, so
  the columns read as one thing without becoming a card. Its footer
  holds the line for the entry being watched and the way to the
  franchise page.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import MetaLine from './MetaLine.svelte';
	import StripFranchiseLink from './StripFranchiseLink.svelte';
	import StripWatching from './StripWatching.svelte';
	import { metaLine, numeral } from './meta';
	import type { WatchOrderMockPrefs } from './prefs';
	import { columnCountText, columnKeyText, emptyColumnText } from './strip-copy';
	import { emptyColumn, watchingFor, type StripView } from './strip-view';
	import { columnKey } from './strip-wording';
	import type { MockEntry, MockRun } from './types';

	let {
		run,
		id,
		view,
		prefs,
		today,
		caption,
		label
	}: {
		run: MockRun;
		id: string;
		view: StripView;
		prefs: WatchOrderMockPrefs;
		today: string;
		caption: string;
		label: (e: MockEntry) => string;
	} = $props();

	const watching = $derived(watchingFor(run, id, view, prefs.order));
	const headingId = $props.id();
	const keyOf = (side: 'before' | 'after') => {
		const list = side === 'before' ? view.before : view.after;
		return columnKeyText(columnKey(prefs.wording, prefs.order, view.kind, side, list.length));
	};
</script>

{#snippet tile(e: MockEntry, mirror: boolean)}
	<a
		class="tile"
		class:mirror
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

{#snippet column(side: 'before' | 'after')}
	{@const list = side === 'before' ? view.before : view.after}
	{@const count = columnCountText(view.kind, side, list.length)}
	<p class="col-key key-{side}">{list.length > 0 ? keyOf(side) : ''}</p>
	<p class="col-count key-{side}">{count ?? ''}</p>
	<div class="col col-{side}">
		{#each list as e (e.id)}{@render tile(e, side === 'before')}{:else}
			<p class="col-empty">{emptyColumnText(emptyColumn(view, side))}</p>
		{/each}
	</div>
{/snippet}

<section class="trio" style:--accent={accentFor(id)} aria-labelledby="{headingId}-h">
	<div class="band">
		<h2 class="eyebrow" id="{headingId}-h">
			<span class="eyebrow-key">{m.watchorder_strip_eyebrow()}</span>
			<span class="sr-only">{caption}</span>
		</h2>

		<div class="grid">
			{@render column('before')}
			<p class="col-key col-key-here here-cell" aria-hidden="true">
				{m.watchorder_franchise_here()}
			</p>
			<p class="col-count here-cell" aria-hidden="true"></p>
			<div class="col col-here here-cell" aria-hidden="true">
				{#if view.kind === 'side'}
					<p class="here-word">{m.watchorder_strip_side_story()}</p>
				{:else if view.position !== null}
					<p class="here-numeral">{numeral(view.position)}</p>
					{#if view.total !== null}
						<p class="here-total">
							{m.watchorder_strip_of_total({ total: numeral(view.total) })}
						</p>
					{/if}
				{:else}
					<p class="here-word">{caption}</p>
				{/if}
			</div>
			{@render column('after')}
		</div>

		<footer class="trio-footer">
			{#if watching}
				<StripWatching
					entry={watching.entry}
					episode={watching.episode}
					variant={prefs.resume}
					{label}
				/>
			{/if}
			<StripFranchiseLink {run} {id} variant={prefs.cta} />
		</footer>
	</div>
</section>

<style>
	.trio {
		padding-inline: var(--space-8);
	}
	/* The band reaches a little past the page's content edge, so what
	   it holds keeps the column the page's text starts on. */
	.band {
		margin-inline: calc(-1 * var(--space-5));
		padding: var(--space-5) var(--space-5) var(--space-4);
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-050) 72%, transparent);
	}
	.band > .eyebrow {
		margin-block-end: var(--space-4);
		white-space: nowrap;
	}

	/* Three rows: keys, join/fork lines, entries. The outer columns
	   share the free width equally, so the centre is the band's
	   centre. */
	.grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
		grid-template-rows: auto auto auto;
		grid-auto-flow: column;
		column-gap: var(--space-6);
	}
	.col-key,
	.col-count {
		margin: 0;
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
	}
	.col-key {
		font-weight: 600;
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.col-count {
		padding-block: 2px var(--space-3);
		font-weight: 500;
		letter-spacing: 0.04em;
		color: var(--bone-400);
	}
	.key-before,
	.col-before .col-empty {
		text-align: end;
	}
	.col-key-here {
		text-align: center;
		color: var(--bone-100);
	}
	/* The centre's two hairlines run through all three rows, so the
	   keys stay apart at the narrowest window. */
	.here-cell {
		min-inline-size: 7.5rem;
		padding-inline: var(--space-5);
		border-inline: 1px solid var(--ink-200);
	}
	.col {
		display: grid;
		gap: var(--space-3);
		align-content: start;
		min-inline-size: 0;
	}
	.col-empty {
		margin: 0;
		font-size: var(--type-meta);
		color: var(--bone-400);
	}

	/* The centre: the oversized numeral motif, ruled in the entry's
	   accent. */
	.col-here {
		justify-items: center;
		gap: var(--space-1);
	}
	.here-numeral {
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
	.here-total,
	.here-word {
		margin: 0;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		text-align: center;
		color: var(--bone-300);
	}
	.here-word {
		max-inline-size: 9rem;
		padding-block: var(--space-3);
		border-block-end: 2px solid var(--accent);
		color: var(--bone-100);
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
	.tile.mirror {
		grid-template-columns: minmax(0, 1fr) 3.5rem;
		text-align: end;
	}
	.tile.mirror .tile-poster {
		order: 2;
	}
	.tile.mirror :global(.meta) {
		justify-content: flex-end;
	}
	/* The meta line stays one line in the strip, so a wrapped line
	   never starts on a dot; the title takes the clamp. */
	.tile :global(.meta) {
		flex-wrap: nowrap;
	}
	.tile:hover {
		background: var(--ink-100);
	}
	.tile:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 2px;
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
		transform-origin: center;
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

	/* The watching line at the start, the franchise link at the end. */
	/* wrap-reverse: when the two do not fit one row, the franchise link
	   (last in reading order) takes the upper row. */
	.trio-footer {
		display: flex;
		flex-wrap: wrap-reverse;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3) var(--space-5);
		margin-block-start: var(--space-5);
		padding-block-start: var(--space-4);
		border-block-start: 1px solid var(--ink-200);
	}
	.trio-footer > :global(:last-child) {
		margin-inline-start: auto;
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
		.here-cell {
			min-inline-size: 5.5rem;
			padding-inline: var(--space-3);
		}
		.grid {
			column-gap: var(--space-4);
		}
	}
</style>
