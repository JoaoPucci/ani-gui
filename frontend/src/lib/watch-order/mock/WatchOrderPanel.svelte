<!--
  The show page's watch-order strip (mockup). Two arrangements:

  - `rail`: the whole run as a Strip of RunCards, scrolled so the entry
    being viewed sits late in the window with what came before it in
    sight; branches, a loop and side entries open with a key and a
    hairline (railItems);
  - `trio`: what comes before, this entry's place, and what comes
    after, mirrored about the centre (StripTrio).

  The rail's header carries the second round's line for the entry
  being watched and a text link to the franchise page; the trio
  carries both in its footer.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import Strip from '$lib/components/Strip.svelte';
	import { m } from '$lib/paraglide/messages';
	import MockSwitch from './MockSwitch.svelte';
	import RunCard from './RunCard.svelte';
	import StripTrio from './StripTrio.svelte';
	import StripWatching from './StripWatching.svelte';
	import { numeral, railKeyText } from '../meta';
	import { loopSize, railItems } from '../order';
	import type { WatchOrderMockPrefs } from './prefs';
	import { shortTitle } from '../short-title';
	import type { MockEntry, MockRun } from './types';
	import { stripView } from '../strip-view';
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

	const here = $derived(stripView(run, id, prefs.order));
	const items = $derived(railItems(run, prefs.order));
	const loop = $derived(loopSize(run, id, prefs.order));
	const titles = $derived(run.entries.map((e) => e.title));
	const onNow = $derived(currentIn(run, run.progress, prefs.order));
	const caption = $derived.by(() => {
		if (here.position !== null && here.total !== null)
			return m.watchorder_strip_position({
				position: numeral(here.position),
				total: numeral(here.total)
			});
		if (here.position !== null) return numeral(here.position);
		if (here.kind === 'side') return m.watchorder_strip_side_story();
		return here.inCycle
			? m.watchorder_strip_unnumbered_cycle({ count: loop })
			: m.watchorder_strip_unnumbered_branch();
	});
	const label = (e: MockEntry) => shortTitle(e.title, titles);

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
		<StripWatching entry={onNow.entry} episode={onNow.episode} {label} />
	{/if}
	<a class="text-btn" href={resolve('/anime/[id]/franchise', { id })}
		>{m.watchorder_strip_franchise_link()}</a
	>
{/snippet}

<div class="watch-order" bind:this={wrap}>
	{#if prefs.switcher}
		<div class="switch-row">
			<MockSwitch
				{prefs}
				keys={['order', 'strip', 'cta', 'stackHover', 'footer', 'stackPosters']}
				{onchoose}
			/>
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
		<StripTrio {run} {id} view={here} {prefs} {today} {caption} {label} />
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
	.watch-order :global(.strip-header .eyebrow) {
		flex: none;
		white-space: nowrap;
	}
	/* The rail's own bottom margin is the page's section rhythm. */
	.watch-order :global(.strip) {
		margin-block-end: 0;
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
</style>
