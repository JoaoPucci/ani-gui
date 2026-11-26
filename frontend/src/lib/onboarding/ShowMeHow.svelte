<!--
  A feature's own "Show me how": a text link on the feature's page that
  runs its walkthrough there at any time ($./tips.svelte showGuide). Only
  a feature that has a walkthrough places it; one asked for this way
  says "How it works" and never counts as a tip seen.
-->
<script lang="ts">
	import type { OnboardingHowTo } from '$lib/api';
	import { m } from '$lib/paraglide/messages';
	import { page } from '$app/state';
	import { onboarding } from './tips.svelte';

	let { howTo }: { howTo: OnboardingHowTo } = $props();
</script>

<button
	type="button"
	class="show-how"
	onclick={() => onboarding.showGuide(howTo, page.url.pathname)}>{m.onboarding_show_how()}</button
>

<style>
	.show-how {
		padding: 0;
		border: 0;
		background: none;
		font: 500 var(--type-meta) / 1.4 var(--font-body);
		color: var(--bone-200);
		text-decoration: underline;
		text-underline-offset: 0.2em;
		cursor: pointer;
	}
	.show-how:hover {
		color: var(--bone-100);
	}
</style>
