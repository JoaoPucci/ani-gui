<!--
  Says a start or a stall to a screen reader. It sits outside the
  player frame: the frame is aria-busy for exactly the start and stall
  this announces, and a screen reader may hold back what changes inside
  a busy region until it clears. Stays mounted, so a change of its text
  is what gets announced.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { VeilMode } from '$lib/play/start-veil';

	let { mode }: { mode: VeilMode } = $props();
	const text = $derived(
		mode === 'start' ? m.playstart_loading_aria() : mode === 'stall' ? m.playstart_buffering() : ''
	);
</script>

<p class="start-announce" role="status">{text}</p>

<style>
	.start-announce {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		margin: 0;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
