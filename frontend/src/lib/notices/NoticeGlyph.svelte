<!--
  A notice's kind glyph, in Icon.svelte's line discipline (1.5px
  stroke, square caps, mitred joins, currentColor) on a 16px grid. A
  resolved condition trades its glyph for a check: that it is over
  matters more than what it was about.
-->
<script lang="ts">
	import type { NoticeKind } from './model';

	let {
		kind,
		settled = false,
		size = 16
	}: { kind: NoticeKind; settled?: boolean; size?: number } = $props();
</script>

<svg
	viewBox="0 0 16 16"
	width={size}
	height={size}
	fill="none"
	stroke="currentColor"
	stroke-width="1.5"
	stroke-linecap="square"
	stroke-linejoin="miter"
	aria-hidden="true"
>
	{#if settled}
		<path d="M3 8.5 L6.5 12 L13 4.5" />
	{:else if kind === 'outage'}
		<!-- The source: a two-unit rack, its lower unit snapped by a
		     break line. About the far end, not the user's network. -->
		<path d="M2.5 2.5 H13.5 V6.5 H2.5 Z" />
		<path d="M5 4.5 H5" />
		<path d="M2.5 9.5 H6.5 L8 11 L7 12.5 L8.5 13.5 H2.5 Z" />
		<path d="M10 9.5 H13.5 V13.5 H11.5 L10.5 12.5 L11.5 11 Z" />
	{:else if kind === 'remote'}
		<!-- A word from the developer: a speech balloon, its tail at the
		     lower start, two lines of text inside. -->
		<path d="M2.5 3 H13.5 V10.5 H6.5 L3.5 13.5 V10.5 H2.5 Z" />
		<path d="M5 5.75 H11" />
		<path d="M5 8 H9" />
	{:else if kind === 'update'}
		<path d="M8 12 V3.5" />
		<path d="M4.5 7 L8 3.5 L11.5 7" />
		<path d="M3 13.5 H13" />
	{:else if kind === 'download-done'}
		<path d="M8 2.5 V9.5" />
		<path d="M5 6.5 L8 9.5 L11 6.5" />
		<path d="M3 13 H13" />
	{:else}
		<path d="M6 2.5 V8.5" />
		<path d="M3.5 6 L6 8.5 L8.5 6" />
		<path d="M3 13 H8" />
		<path d="M10.5 9.5 L14 13" />
		<path d="M14 9.5 L10.5 13" />
	{/if}
</svg>
