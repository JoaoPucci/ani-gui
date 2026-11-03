<!--
  What a notice has to say beyond its row: its paragraphs, each with an
  optional label, and its one action. Shared by the detail page and by
  a row opened in place on the player. Every string is drawn as text.
-->
<script lang="ts">
	import type { NoticeCopy } from './copy';

	let {
		copy,
		compact = false,
		onAct
	}: {
		copy: NoticeCopy;
		compact?: boolean;
		onAct: () => void;
	} = $props();
</script>

<div class="nb" class:compact>
	{#each copy.body as block, i (i)}
		<div class="nb-block">
			{#if block.label}
				<p class="nb-label">{block.label}</p>
			{/if}
			<p class="nb-text">{block.text}</p>
		</div>
	{/each}
	{#if copy.action}
		<div class="nb-actions">
			<button type="button" class="nb-action" onclick={onAct}>{copy.action.label}</button>
		</div>
	{/if}
</div>

<style>
	.nb {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-inline-size: 60ch;
	}
	.nb.compact {
		gap: var(--space-3);
	}
	.nb-block {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	/* A label over evidence: the panel-gutter rule of a manga page,
	   once, then the words. */
	.nb-label {
		margin: 0;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-meta);
		color: var(--bone-300);
	}
	.nb-label::after {
		content: '';
		flex: 1 1 auto;
		block-size: 1px;
		background: var(--ink-200);
	}
	.nb-text {
		margin: 0;
		white-space: pre-line;
		overflow-wrap: anywhere;
		font-size: var(--type-body);
		line-height: 1.55;
		color: var(--bone-200);
	}
	.compact .nb-text {
		font-size: var(--type-meta);
	}
	.nb-actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.nb-action {
		padding: var(--space-2) var(--space-4);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		background: var(--ink-100);
		color: var(--bone-100);
		font-size: var(--type-meta);
		font-weight: 500;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.nb-action:hover {
		background: var(--ink-200);
		border-color: var(--bone-300);
	}
	.nb-action:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 2px;
	}
</style>
