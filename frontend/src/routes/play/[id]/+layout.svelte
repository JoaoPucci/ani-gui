<!--
  The player route mounts its page afresh for each show. The page loads
  the show it plays as it mounts, and SvelteKit would serve another id
  on this route with the page it already has: a switch to the show
  picked for a card ($lib/play/switch-show.ts), or a step back or
  forward between two shows' players, would keep the first show's
  title, episodes and stream state. Another episode of the same show
  keeps its page, so episode switches stay in place.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';

	let { children }: { children: Snippet } = $props();
</script>

{#key page.params.id}
	{@render children()}
{/key}
