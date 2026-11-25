<!--
  The franchise page for the show with this id: the backend's answer
  for it ($lib/watch-order/loader), which reads the franchise, the
  watch history and the order setting together and decides everything
  the page draws. A franchise that cannot be read, or answers with no
  main story, says so, with Try again. A franchise that is the show
  alone says that instead.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import type { FranchiseView as FranchiseAnswer } from '$lib/api';
	import { pageTrail } from '$lib/page-trail';
	import { m } from '$lib/paraglide/messages';
	import FranchiseView from '$lib/watch-order/FranchiseView.svelte';
	import { franchiseTrail } from '$lib/watch-order/franchise-page';
	import { latestRead } from '$lib/watch-order/latest-read';
	import { visitRead, type WatchOrderAnswer } from '$lib/watch-order/loader';
	import { followLink } from '$lib/watch-order/link-reread';
	import { onHistoryLinked } from '$lib/history/link-deps';
	import { watchOrderSources } from '$lib/watch-order/sources';

	const id = $derived(page.params.id ?? '');

	// One read per show and per Try again. Only the latest read's
	// answer is drawn, so an older one landing late never replaces it,
	// and a read superseded, or still out when the page goes, is
	// cancelled ($lib/watch-order/latest-read).
	let data = $state<WatchOrderAnswer<FranchiseAnswer> | null>(null);
	let attempt = $state(0);
	const reads = latestRead<WatchOrderAnswer<FranchiseAnswer> | null>((d) => (data = d));
	const readPage = (showId: string) => (signal: AbortSignal) =>
		visitRead((s) => watchOrderSources.franchise(showId, s), signal);
	$effect(() => {
		const current = id;
		void attempt;
		reads.start(readPage(current));
	});
	// A history link answered while the page is on screen reads the page
	// again, the page staying drawn meanwhile, or the show afresh while
	// its first read is out ($lib/watch-order/link-reread).
	const stopHearing = onHistoryLinked(() => {
		if (id) followLink(reads, data, readPage(id));
	});
	onDestroy(() => {
		stopHearing();
		reads.stop();
	});

	const view = $derived(data?.view ?? null);
	const model = $derived(view?.page ?? null);
	// The topbar names the show the page was opened from, once the
	// franchise has been read.
	$effect(() => {
		if (model) pageTrail.name(page.url.pathname, franchiseTrail(model));
	});
</script>

{#if model?.alone}
	<p class="none">{m.watchorder_franchise_none()}</p>
{:else if model && view}
	{#key id}
		<FranchiseView {model} order={view.order} />
	{/key}
{:else if data}
	<div class="failed">
		<p>{m.watchorder_franchise_failed()}</p>
		<button type="button" class="btn btn-outline" onclick={() => attempt++}
			>{m.watchorder_franchise_retry()}</button
		>
	</div>
{/if}

<style>
	.none {
		margin: 0;
		padding: var(--space-7) var(--space-8);
		color: var(--bone-300);
	}
	.failed {
		display: grid;
		justify-items: start;
		gap: var(--space-4);
		padding: var(--space-7) var(--space-8);
		color: var(--bone-300);
	}
	/* The show page's outline button (routes/anime/[id]), restated: its
	   styles are scoped to that page. */
	.btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.btn:hover {
		transform: translateY(-1px);
	}
	.btn-outline {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.btn-outline:hover {
		border-color: var(--bone-100);
	}
</style>
