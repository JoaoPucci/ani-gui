<!-- The mockup's option switch: a dashed panel at the window's
     bottom-start corner, with each design choice and the states a
     stream can reach, to try them on the live mini window. -->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { MINI_MOCK_OPTIONS, type MiniMockOptionKey } from './prefs';
	import { miniMock } from './MiniHost.svelte';
	import type { MiniSim } from './MiniWindow.svelte';

	let { onForgetPlace }: { onForgetPlace: () => void } = $props();

	const GROUPS: {
		key: MiniMockOptionKey;
		label: () => string;
		names: Record<string, () => string>;
	}[] = [
		{
			key: 'layout',
			label: m.mini_sw_layout,
			names: { bar: m.mini_sw_layout_bar, centre: m.mini_sw_layout_centre }
		},
		{
			key: 'main',
			label: m.mini_sw_main,
			names: { back: m.mini_sw_main_back, placeholder: m.mini_sw_main_placeholder }
		},
		{
			key: 'onTop',
			label: m.mini_sw_ontop,
			names: { card: m.mini_sw_ontop_card, pin: m.mini_sw_ontop_pin, both: m.mini_sw_ontop_both }
		},
		{
			key: 'platform',
			label: m.mini_sw_platform,
			names: {
				wayland: m.mini_sw_platform_wayland,
				windows: m.mini_sw_platform_windows,
				x11: m.mini_sw_platform_x11
			}
		},
		{
			key: 'gap',
			label: m.mini_sw_gap,
			names: { '0': m.mini_sw_gap_0, '1500': m.mini_sw_gap_1500, '4000': m.mini_sw_gap_4000 }
		},
		{
			key: 'cues',
			label: m.mini_sw_cues,
			names: { native: m.mini_sw_cues_native, floor: m.mini_sw_cues_floor }
		},
		{
			key: 'lift',
			label: m.mini_sw_lift,
			names: { lift: m.mini_sw_lift_lift, stay: m.mini_sw_lift_stay }
		},
		{
			key: 'grip',
			label: m.mini_sw_grip,
			names: { chrome: m.mini_sw_grip_chrome, always: m.mini_sw_grip_always }
		},
		{
			key: 'failure',
			label: m.mini_sw_failure,
			names: { inline: m.mini_sw_failure_inline, 'hand-back': m.mini_sw_failure_handback }
		},
		{
			key: 'shortcut',
			label: m.mini_sw_shortcut,
			names: { i: m.mini_sw_shortcut_i, none: m.mini_sw_shortcut_none }
		},
		{
			key: 'copy',
			label: m.mini_sw_copy,
			names: {
				general: m.mini_sw_copy_general,
				gnome: m.mini_sw_copy_gnome,
				kde: m.mini_sw_copy_kde
			}
		}
	];

	const SIMS: { sim: MiniSim; label: () => string }[] = [
		{ sim: 'stall', label: m.mini_sw_sim_stall },
		{ sim: 'error', label: m.mini_sw_sim_error },
		{ sim: 'end', label: m.mini_sw_sim_end }
	];
</script>

<details class="mini-switcher" open>
	<summary>{m.mini_sw_heading()}</summary>
	{#each GROUPS as g (g.key)}
		<fieldset>
			<legend>{g.label()}</legend>
			{#each MINI_MOCK_OPTIONS[g.key] as value (value)}
				<label>
					<input
						type="radio"
						name="mini-{g.key}"
						checked={miniMock.prefs[g.key] === value}
						onchange={() => miniMock.setPrefs({ [g.key]: value })}
					/>
					{g.names[value]()}
				</label>
			{/each}
		</fieldset>
	{/each}
	<fieldset>
		<legend>{m.mini_sw_simulate()}</legend>
		{#each SIMS as s (s.sim)}
			<button
				type="button"
				aria-pressed={miniMock.sim === s.sim}
				disabled={!miniMock.session}
				onclick={() => miniMock.setSim(s.sim)}>{s.label()}</button
			>
		{/each}
		<button
			type="button"
			aria-pressed={miniMock.skip}
			disabled={!miniMock.session}
			onclick={() => miniMock.setSkip(!miniMock.skip)}>{m.mini_sw_sim_skip()}</button
		>
		<button
			type="button"
			disabled={!miniMock.session}
			onclick={() => {
				miniMock.setSim('none');
				miniMock.setSkip(false);
			}}>{m.mini_sw_sim_clear()}</button
		>
	</fieldset>
	<fieldset>
		<button type="button" onclick={onForgetPlace}>{m.mini_sw_reset_place()}</button>
		<button type="button" onclick={() => miniMock.setPrefs({ hintSeen: false })}
			>{m.mini_sw_reset_hint()}</button
		>
	</fieldset>
</details>

<style>
	.mini-switcher {
		position: fixed;
		z-index: 901;
		inset-inline-start: var(--space-3);
		inset-block-end: var(--space-3);
		max-inline-size: 22rem;
		max-block-size: calc(100vh - 6rem);
		overflow-y: auto;
		padding: var(--space-2) var(--space-3);
		border: 1px dashed var(--bone-400);
		border-radius: var(--radius-card);
		background: var(--ink-050);
		color: var(--bone-200);
		font: 400 0.75rem/1.4 var(--font-body);
		-webkit-app-region: no-drag;
		app-region: no-drag;
	}
	summary {
		cursor: pointer;
		font-family: var(--font-mono);
		color: var(--bone-300);
	}
	fieldset {
		display: flex;
		flex-wrap: wrap;
		gap: 2px var(--space-2);
		margin: var(--space-1) 0 0;
		padding: 0;
		border: 0;
	}
	legend {
		padding: 0;
		margin-block-end: 2px;
		color: var(--bone-300);
		font-weight: 600;
	}
	label {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}
	button {
		padding: 2px 8px;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		font: inherit;
		cursor: pointer;
	}
	button[aria-pressed='true'] {
		border-color: var(--accent);
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>
