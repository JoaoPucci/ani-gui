<!--
  The franchise page for the show with this id: its franchise and the
  watch history read together ($lib/watch-order/loader), drawn in the
  order the settings choose once that read has settled. A franchise
  that cannot be read, or answers with no main story, says so, with
  Try again. A franchise that is the show alone says that instead.
-->
<script lang="ts">
	import { page } from '$app/state';
	import { historyList, kitsuFranchise, settingsGet, type Config } from '$lib/api';
	import { pageTrail } from '$lib/page-trail';
	import { m } from '$lib/paraglide/messages';
	import FranchiseView from '$lib/watch-order/FranchiseView.svelte';
	import { localDay } from '$lib/watch-order/entry';
	import { franchisePage, franchiseTrail } from '$lib/watch-order/franchise-page';
	import { latestRead } from '$lib/watch-order/latest-read';
	import { loadWatchOrder, type WatchOrderData } from '$lib/watch-order/loader';
	import { settledOrder } from '$lib/watch-order/page-model';

	const id = $derived(page.params.id ?? '');
	const today = localDay(new Date());

	let config = $state<Config | null>(null);
	let configFailed = $state(false);
	void settingsGet()
		.then((c) => (config = c))
		.catch(() => (configFailed = true));

	// One read per show and per Try again. Only the latest read's
	// answer is drawn, so an older one landing late never replaces it
	// ($lib/watch-order/latest-read).
	let data = $state<WatchOrderData | null>(null);
	let attempt = $state(0);
	const reads = latestRead<WatchOrderData>((d) => (data = d));
	$effect(() => {
		const current = id;
		void attempt;
		reads.start(() => loadWatchOrder(current, { franchise: kitsuFranchise, history: historyList }));
	});

	const order = $derived(settledOrder(config, configFailed));
	const model = $derived(data && order ? franchisePage(data, id, order) : null);
	// The topbar names the show the page was opened from, once the
	// franchise has been read.
	$effect(() => {
		if (model) pageTrail.name(page.url.pathname, franchiseTrail(model, id));
	});
</script>

{#if model?.alone}
	<p class="none">{m.watchorder_franchise_none()}</p>
{:else if model}
	{#key id}
		<FranchiseView {model} {today} />
	{/key}
{:else if data && order}
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
</style>
