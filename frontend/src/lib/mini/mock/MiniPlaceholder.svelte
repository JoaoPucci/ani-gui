<!-- The play page's frame while its playback is in the mini player
     (the "placeholder" option): the frame keeps its size, shows the
     last frame dimmed, and offers the way back. -->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import MiniIcon from './MiniIcon.svelte';

	let {
		still,
		covered,
		onPlayHere,
		onClose
	}: {
		still: string | null;
		/** On Wayland the mini may be behind this window. */
		covered: boolean;
		onPlayHere: () => void;
		onClose: () => void;
	} = $props();
</script>

<div class="mini-placeholder">
	{#if still}
		<img class="mini-placeholder-still" src={still} alt="" />
	{/if}
	<div class="mini-placeholder-column">
		<span class="mini-placeholder-icon"><MiniIcon size={32} /></span>
		<p class="mini-placeholder-headline" aria-live="polite">{m.mini_placeholder_headline()}</p>
		<p class="mini-placeholder-body">
			{covered ? m.mini_placeholder_body_wayland() : m.mini_placeholder_body()}
		</p>
		<div class="mini-placeholder-actions">
			<button
				type="button"
				class="mini-placeholder-primary"
				title={m.mini_play_here_title()}
				onclick={onPlayHere}
			>
				{m.mini_play_here()}
			</button>
			<button type="button" class="mini-placeholder-ghost" onclick={onClose}>
				{m.mini_placeholder_close()}
			</button>
		</div>
	</div>
</div>

<style>
	.mini-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: end start;
		background: #000;
		overflow: hidden;
	}
	.mini-placeholder-still {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: contain;
		opacity: 0.3;
	}
	.mini-placeholder-column {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-3);
		max-inline-size: 26rem;
		padding: var(--space-6);
		text-align: start;
	}
	.mini-placeholder-icon {
		color: var(--bone-300);
	}
	.mini-placeholder-headline {
		margin: 0;
		font-family: var(--font-display);
		font-weight: 600;
		font-size: var(--type-display-m);
		letter-spacing: var(--tracking-display);
		color: var(--bone-100);
		text-wrap: balance;
	}
	.mini-placeholder-body {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
		text-wrap: balance;
	}
	.mini-placeholder-actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-block-start: var(--space-2);
	}
	.mini-placeholder-primary,
	.mini-placeholder-ghost {
		block-size: 2.5rem;
		padding: 0 var(--space-4);
		border-radius: var(--radius-control);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-body);
		cursor: pointer;
	}
	.mini-placeholder-primary {
		border: 0;
		background: var(--bone-100);
		color: var(--ink-000);
	}
	.mini-placeholder-ghost {
		border: 1px solid var(--ink-300);
		background: transparent;
		color: var(--bone-100);
	}
	.mini-placeholder-ghost:hover {
		border-color: var(--bone-300);
	}
</style>
