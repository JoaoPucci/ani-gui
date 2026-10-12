<!--
  DEV-ONLY hero framing switch for the banner experiment. Rendered only
  under `import.meta.env.DEV`; delete with `$lib/hero/framing` before
  anything ships. A choice is stored and the page reloads, so both hero
  pages read it once at mount.
-->
<script lang="ts">
	import {
		HERO_EXPERIMENT_STORAGE_KEY,
		type HeroExperiment,
		type HeroFraming,
		type HeroOrigin,
		type HeroSource
	} from './framing';

	let { exp, origin }: { exp: HeroExperiment; origin: HeroOrigin | null } = $props();

	const framings: HeroFraming[] = ['current', 'contain', 'pillar', 'focus'];
	const sources: HeroSource[] = ['kitsu', 'anilist'];
	// Dev-only tool: not translated.
	const groupLabel = 'hero framing experiment';

	function choose(next: HeroExperiment) {
		localStorage.setItem(HERO_EXPERIMENT_STORAGE_KEY, JSON.stringify(next));
		location.reload();
	}
</script>

{#if import.meta.env.DEV}
	<div class="switch" role="group" aria-label={groupLabel}>
		{#each framings as f (f)}
			<button
				type="button"
				aria-pressed={exp.framing === f}
				onclick={() => choose({ ...exp, framing: f })}>{f}</button
			>
		{/each}
		<span class="sep" aria-hidden="true"></span>
		{#each sources as s (s)}
			<button
				type="button"
				aria-pressed={exp.source === s}
				onclick={() => choose({ ...exp, source: s })}>{s}</button
			>
		{/each}
		<span class="sep" aria-hidden="true"></span>
		<span class="origin">{origin ?? 'none'}</span>
	</div>
{/if}

<style>
	.switch {
		position: fixed;
		inset-block-end: var(--space-4);
		inset-inline-end: var(--space-4);
		z-index: 1000;
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 3px;
		border-radius: 6px;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		font: 12px/1 var(--font-body);
	}
	button {
		padding: 6px 8px;
		border: 0;
		border-radius: 4px;
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
	}
	button[aria-pressed='true'] {
		background: var(--bone-100);
		color: var(--ink-000);
	}
	button:focus-visible {
		outline: 2px solid var(--bone-100);
		outline-offset: 1px;
	}
	.sep {
		inline-size: 1px;
		block-size: 16px;
		margin-inline: 4px;
		background: var(--ink-200);
	}
	.origin {
		padding-inline: 6px;
		color: var(--bone-300);
	}
</style>
