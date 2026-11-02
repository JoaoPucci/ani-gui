<!--
  The mockups' option switch: one pressed-button group per option the
  surface offers. Shown only while the diagnostics page's second toggle
  is on, so screenshots can leave it out.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { MOCK_OPTIONS, type MockOptionKey, type WatchOrderMockPrefs } from './prefs';

	let {
		prefs,
		keys,
		onchoose
	}: {
		prefs: WatchOrderMockPrefs;
		keys: readonly MockOptionKey[];
		onchoose: (next: WatchOrderMockPrefs) => void;
	} = $props();

	const groupLabel: Record<MockOptionKey, () => string> = {
		order: m.watchorder_mock_order,
		strip: m.watchorder_mock_strip,
		placement: m.watchorder_mock_placement,
		titles: m.watchorder_mock_titles,
		upNext: m.watchorder_mock_upnext,
		franchise: m.watchorder_mock_franchise,
		setting: m.watchorder_mock_setting,
		handoff: m.watchorder_mock_handoff,
		wording: m.watchorder_mock_wording,
		cta: m.watchorder_mock_cta,
		resume: m.watchorder_mock_resume
	};
	const optionLabel: Record<string, () => string> = {
		linked: m.watchorder_setting_linked,
		released: m.watchorder_setting_released,
		rail: m.watchorder_mock_strip_rail,
		trio: m.watchorder_mock_strip_trio,
		top: m.watchorder_mock_placement_top,
		bottom: m.watchorder_mock_placement_bottom,
		full: m.watchorder_mock_titles_full,
		short: m.watchorder_mock_titles_short,
		poster: m.watchorder_mock_upnext_poster,
		still: m.watchorder_mock_upnext_still,
		list: m.watchorder_mock_franchise_list,
		shelf: m.watchorder_mock_franchise_shelf,
		segmented: m.watchorder_mock_setting_segmented,
		radio: m.watchorder_mock_setting_radio,
		next: m.watchorder_mock_handoff_next,
		episode: m.watchorder_mock_handoff_episode,
		role: m.watchorder_mock_wording_role,
		previous: m.watchorder_mock_wording_previous,
		watch: m.watchorder_mock_wording_watch,
		plain: m.watchorder_mock_wording_plain,
		button: m.watchorder_mock_cta_button,
		stack: m.watchorder_mock_cta_stack,
		chip: m.watchorder_mock_resume_chip,
		sentence: m.watchorder_mock_resume_sentence
	};
</script>

{#if prefs.switcher}
	<div class="mock-switch" role="group" aria-label={m.watchorder_mock_group_aria()}>
		<span class="badge">{m.watchorder_mock_badge()}</span>
		{#each keys as key (key)}
			<span class="group">
				<span class="group-label">{groupLabel[key]()}</span>
				{#each MOCK_OPTIONS[key] as option (option)}
					<button
						type="button"
						aria-pressed={prefs[key] === option}
						onclick={() => onchoose({ ...prefs, [key]: option })}>{optionLabel[option]()}</button
					>
				{/each}
			</span>
		{/each}
	</div>
{/if}

<style>
	/* The hero framing switch's surface, inline rather than over the
	   hero: these surfaces have no free corner to float in. */
	.mock-switch {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
		inline-size: fit-content;
		max-inline-size: 100%;
		padding: 3px var(--space-2);
		border: 1px dashed var(--ink-300);
		border-radius: var(--radius-control);
		background: var(--ink-050);
		font-size: var(--type-micro);
		line-height: 1;
		color: var(--bone-300);
	}
	.badge {
		font-family: var(--font-mono);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-400);
	}
	.group {
		display: inline-flex;
		align-items: center;
		gap: 2px;
	}
	.group-label {
		margin-inline-end: var(--space-1);
	}
	button {
		padding: 6px var(--space-2);
		border-radius: calc(var(--radius-control) - 2px);
		color: var(--bone-300);
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
