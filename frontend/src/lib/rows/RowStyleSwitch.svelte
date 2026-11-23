<!--
  Row style switch: how the home rows present their header actions.
  The home page renders it while the diagnostics page's toggle is on,
  in the same visual language as the hero framing switch. A choice goes
  to the page through `onchoose`; the page applies it to every row at
  once and keeps it for the next load.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { ROW_STYLES, type RowPrefs, type RowStyle } from './prefs';

	let {
		prefs,
		onchoose
	}: {
		prefs: RowPrefs;
		/** A choice made in the switch; the page applies and keeps it. */
		onchoose: (next: RowPrefs) => void;
	} = $props();

	const styleLabel: Record<RowStyle, () => string> = {
		inline: m.rows_style_inline,
		band: m.rows_style_band
	};
</script>

<div class="row-style-switch" role="group" aria-label={m.rows_style_group_label()}>
	{#each ROW_STYLES as s (s)}
		<button
			type="button"
			aria-pressed={prefs.style === s}
			onclick={() => onchoose({ ...prefs, style: s })}>{styleLabel[s]()}</button
		>
	{/each}
</div>

<style>
	/* Placed by the page at the head of the rows it governs, in flow
	   (not fixed), so it scrolls away with them and never covers a row
	   header the way an overlay would. Same pill as the hero switch. */
	.row-style-switch {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		padding: 3px;
		border-radius: var(--radius-control);
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		box-shadow: var(--shadow-card-rest);
		font-family: var(--font-body);
		font-size: var(--type-micro);
		line-height: 1;
		color: var(--bone-300);
	}
	button {
		padding: 6px var(--space-2);
		border: 0;
		border-radius: calc(var(--radius-control) - 2px);
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		white-space: nowrap;
		transition:
			background-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	button:hover {
		color: var(--bone-100);
	}
	button[aria-pressed='true'] {
		background: var(--bone-100);
		color: var(--ink-000);
	}
</style>
