<!-- The top bar's sign that an episode plays in the mini player, once
     the main window is somewhere other than that show's play page. A
     click brings playback back. -->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import MiniIcon from './MiniIcon.svelte';

	let {
		title,
		episode,
		accent,
		onPlayHere
	}: { title: string; episode: number; accent: string; onPlayHere: () => void } = $props();
</script>

<button
	type="button"
	class="mini-chip"
	style:--accent={accent}
	title={m.mini_play_here_title()}
	aria-label={m.mini_chip_aria({ title, episode: String(episode) })}
	onclick={onPlayHere}
>
	<span class="mini-chip-dot" aria-hidden="true"></span>
	<MiniIcon size={16} />
	<span class="mini-chip-ep">{m.play_episode_nav_current_label({ episode: String(episode) })}</span>
</button>

<style>
	.mini-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		block-size: 2rem;
		padding: 0 var(--space-3) 0 var(--space-2);
		border-radius: var(--radius-pill);
		border: 1px solid color-mix(in oklab, var(--bone-100) 22%, transparent);
		background: color-mix(in oklab, var(--ink-050) 70%, transparent);
		color: var(--bone-100);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: 0.8125rem;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		cursor: pointer;
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}
	.mini-chip:hover {
		border-color: color-mix(in oklab, var(--bone-100) 45%, transparent);
	}
	.mini-chip-dot {
		inline-size: 6px;
		block-size: 6px;
		border-radius: 50%;
		background: var(--accent);
	}
</style>
