<!--
  Franchise page — a 0.19.0 design mockup, drawn from fixture data and
  shown only while Diagnostics turns the watch-order mockups on.
-->
<script lang="ts">
	import { page } from '$app/state';
	import { browserStorage } from '$lib/hero/prefs';
	import { m } from '$lib/paraglide/messages';
	import FranchiseView from '$lib/watch-order/mock/FranchiseView.svelte';
	import {
		loadMockPrefs,
		saveMockPrefs,
		type WatchOrderMockPrefs
	} from '$lib/watch-order/mock/prefs';
	import { stripRunFor } from '$lib/watch-order/mock/strip-run';

	const id = $derived(page.params.id ?? '');
	const today = new Date().toISOString().slice(0, 10);
	let prefs = $state(loadMockPrefs(browserStorage()));
	const run = $derived(prefs.enabled ? stripRunFor(id) : null);
	function choose(next: WatchOrderMockPrefs) {
		prefs = next;
		saveMockPrefs(browserStorage(), next);
	}
</script>

{#if run}
	{#key id}
		<FranchiseView {run} {id} {prefs} {today} onchoose={choose} />
	{/key}
{:else}
	<p class="off">{m.watchorder_franchise_off()}</p>
{/if}

<style>
	.off {
		padding: var(--space-7) var(--space-8);
		color: var(--bone-300);
	}
</style>
