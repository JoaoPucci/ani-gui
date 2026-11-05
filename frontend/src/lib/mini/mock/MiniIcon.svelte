<!-- The mini player's mark: a window frame left open at its
     bottom-end corner, a small solid window in the gap. -->
<script lang="ts">
	let { size = 24 }: { size?: number } = $props();
</script>

<svg class="mini-icon" viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
	<path d="M3 5h18v7h-2V7H5v10h6v2H3z M13 14h8v6h-8z" fill="currentColor" />
</svg>

<style>
	.mini-icon:dir(rtl) {
		transform: scaleX(-1);
	}
</style>
