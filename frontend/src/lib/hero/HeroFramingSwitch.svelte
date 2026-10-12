<!--
  Hero framing switch: which image the hero shows and how it is framed.
  Rendered only under `import.meta.env.DEV` for now. A choice is stored
  and the page reloads, so both hero pages read it once at mount.
-->
<script lang="ts">
	import {
		HERO_FRAMINGS,
		HERO_SOURCES,
		browserStorage,
		saveHeroPrefs,
		type HeroOrigin,
		type HeroPrefs
	} from './framing';

	let { prefs, origin }: { prefs: HeroPrefs; origin: HeroOrigin | null } = $props();

	// Dev-only tool: not translated.
	const groupLabel = 'hero framing';

	function choose(next: HeroPrefs) {
		saveHeroPrefs(browserStorage(), next);
		location.reload();
	}
</script>

{#if import.meta.env.DEV}
	<div class="hero-framing-switch" role="group" aria-label={groupLabel}>
		{#each HERO_FRAMINGS as f (f)}
			<button
				type="button"
				aria-pressed={prefs.framing === f}
				onclick={() => choose({ ...prefs, framing: f })}>{f}</button
			>
		{/each}
		<span class="sep" aria-hidden="true"></span>
		{#each HERO_SOURCES as s (s)}
			<button
				type="button"
				aria-pressed={prefs.source === s}
				onclick={() => choose({ ...prefs, source: s })}>{s}</button
			>
		{/each}
		<span class="sep" aria-hidden="true"></span>
		<span class="origin">{origin ?? 'none'}</span>
	</div>
{/if}

<style>
	.hero-framing-switch {
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
