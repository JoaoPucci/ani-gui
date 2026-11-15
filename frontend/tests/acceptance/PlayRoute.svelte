<!--
  The player route as SvelteKit renders it: the route's layout around
  its page. A navigation to another show's player changes the route
  param under both, as SvelteKit's does.
-->
<script lang="ts">
	import PlayLayout from '../../src/routes/play/[id]/+layout.svelte';
	import PlayPage from '../../src/routes/play/[id]/+page.svelte';
</script>

<PlayLayout>
	<PlayPage />
</PlayLayout>
