<!--
  The franchise page for the show with this id: its franchise and the
  watch history read together ($lib/watch-order/loader), drawn in the
  order the settings choose once that read has settled. A franchise
  that cannot be read says so, with Try again.
-->
<script lang="ts">
	import { page } from '$app/state';
	import { historyList, kitsuFranchise, settingsGet, type Config } from '$lib/api';
	import { pageTrail } from '$lib/page-trail';
	import { m } from '$lib/paraglide/messages';
	import FranchiseView from '$lib/watch-order/FranchiseView.svelte';
	import { localDay } from '$lib/watch-order/entry';
	import { franchisePage, franchiseTrail } from '$lib/watch-order/franchise-page';
	import { loadWatchOrder, type WatchOrderData } from '$lib/watch-order/loader';
	import { settledOrder } from '$lib/watch-order/page-model';

	const id = $derived(page.params.id ?? '');
	const today = localDay(new Date());

	let config = $state<Config | null>(null);
	let configFailed = $state(false);
	void settingsGet()
		.then((c) => (config = c))
		.catch(() => (configFailed = true));

	// One read per show and per Try again; an answer for a show the
	// page has since left is dropped.
	let data = $state<WatchOrderData | null>(null);
	let attempt = $state(0);
	$effect(() => {
		const current = id;
		void attempt;
		data = null;
		void loadWatchOrder(current, { franchise: kitsuFranchise, history: historyList }).then((d) => {
			if (current === id) data = d;
		});
	});

	const order = $derived(settledOrder(config, configFailed));
	const model = $derived(data && order ? franchisePage(data, id, order) : null);
	// The topbar names the show the page was opened from, once the
	// franchise has been read.
	$effect(() => {
		if (model) pageTrail.name(page.url.pathname, franchiseTrail(model, id));
	});
</script>

{#if model}
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
	.failed {
		display: grid;
		justify-items: start;
		gap: var(--space-4);
		padding: var(--space-7) var(--space-8);
		color: var(--bone-300);
	}
</style>
