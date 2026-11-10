<!--
  Backend Status — proves IPC works end-to-end. Loads app_info +
  history_list on mount, exposes history_clear. Plain HTML on purpose;
  the design pass (M3) will replace this entirely.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { appInfo, historyClear, historyList, metaCacheClear } from '$lib/api';
	import {
		createDiagnosticsActions,
		INITIAL_DIAGNOSTICS,
		type DiagnosticsState
	} from '$lib/diagnostics/history-actions';
	import { legacySweepView } from '$lib/diagnostics/legacy-sweep';
	import { retiredSweepView } from '$lib/diagnostics/retired-sweep';
	import { cacheReclaimView } from '$lib/diagnostics/cache-reclaim';
	import { browserStorage, loadHeroPrefs, saveHeroPrefs } from '$lib/hero/prefs';
	import { loadRowPrefs, saveRowPrefs } from '$lib/rows/prefs';
	import { loadMockPrefs, saveMockPrefs } from '$lib/watch-order/mock/prefs';
	import { m } from '$lib/paraglide/messages';
	import { describeError } from '$lib/play/error-copy';

	// The page's actions and what they leave on show live in
	// $lib/diagnostics/history-actions; this renders its state.
	let view = $state<DiagnosticsState>({ ...INITIAL_DIAGNOSTICS });
	const actions = createDiagnosticsActions({
		appInfo,
		historyList,
		historyClear,
		metaCacheClear,
		describe: describeError,
		changed: (next) => (view = next)
	});
	const info = $derived(view.info);
	const history = $derived(view.history);

	// What the backend's boot sweep removed, if anything. Present only
	// on the launch that upgraded from a version which kept its own
	// copy of the shell script — see $lib/diagnostics/legacy-sweep.
	const legacySweep = $derived(legacySweepView(info?.removed_legacy_paths));
	// What the sweep over a retired streaming provider's rows changed,
	// if anything — see $lib/diagnostics/retired-sweep.
	const retiredSweep = $derived(retiredSweepView(info?.retired_provider_sweep));
	// What the launch's cache reclaim deleted, if anything — see
	// $lib/diagnostics/cache-reclaim.
	const cacheReclaim = $derived(cacheReclaimView(info?.cache_reclaim));

	// Whether the home and detail pages show their banner framing
	// switch. Kept with the framing preferences themselves, so turning
	// the controls on or off leaves a chosen framing as it is — see
	// $lib/hero/prefs.
	let heroPrefs = $state(loadHeroPrefs(browserStorage()));
	function setFramingControls(shown: boolean) {
		heroPrefs = { ...heroPrefs, controls: shown };
		saveHeroPrefs(browserStorage(), heroPrefs);
	}

	// Watch-order mockups (0.19.0 design review): whether the player,
	// home and franchise pages draw them, and whether
	// each shows its option switch — see $lib/watch-order/mock.
	let mockPrefs = $state(loadMockPrefs(browserStorage()));
	function setMock(field: 'enabled' | 'switcher', on: boolean) {
		mockPrefs = { ...mockPrefs, [field]: on };
		saveMockPrefs(browserStorage(), mockPrefs);
	}

	// Whether the home page shows its row style switch, kept with the
	// row preferences the same way — see $lib/rows/prefs.
	let rowPrefs = $state(loadRowPrefs(browserStorage()));
	function setRowStyleControls(shown: boolean) {
		rowPrefs = { ...rowPrefs, controls: shown };
		saveRowPrefs(browserStorage(), rowPrefs);
	}

	onMount(() => void actions.refresh());
</script>

<main class="page">
	<h1>{m.diagnostics_title()}</h1>

	<section>
		<h2>{m.diagnostics_section_app_info()}</h2>
		{#if view.infoError}
			<p>{m.diagnostics_app_info_error()} {view.infoError}</p>
		{:else if info}
			<dl>
				<dt>{m.diagnostics_app_info_version()}</dt>
				<dd>{info.version}</dd>
				<dt>{m.diagnostics_app_info_history_file()}</dt>
				<dd>{info.history_path}</dd>
				<dt>{m.diagnostics_app_info_proxy_base()}</dt>
				<dd>{info.proxy_base_url}</dd>
			</dl>
		{:else}
			<p>{m.diagnostics_app_info_loading()}</p>
		{/if}
	</section>

	{#if cacheReclaim.visible}
		<section>
			<h2>{m.diagnostics_section_cache_reclaim()}</h2>
			<p>{m.diagnostics_cache_reclaim_description()}</p>
			<ul>
				{#if cacheReclaim.oldRows > 0}
					<li>{m.diagnostics_cache_reclaim_old({ count: cacheReclaim.oldRows })}</li>
				{/if}
				{#if cacheReclaim.expiredRows > 0}
					<li>{m.diagnostics_cache_reclaim_expired({ count: cacheReclaim.expiredRows })}</li>
				{/if}
			</ul>
		</section>
	{/if}

	{#if retiredSweep.visible}
		<section>
			<h2>{m.diagnostics_section_retired_sweep()}</h2>
			{#if retiredSweep.superseded.length > 0}
				<p>{m.diagnostics_retired_sweep_superseded()}</p>
				<ul>
					{#each retiredSweep.superseded as title, i (i)}
						<li>{title}</li>
					{/each}
				</ul>
			{/if}
			{#if retiredSweep.renumbered.length > 0}
				<p>{m.diagnostics_retired_sweep_renumbered()}</p>
				<ul>
					{#each retiredSweep.renumbered as title, i (i)}
						<li>{title}</li>
					{/each}
				</ul>
			{/if}
			{#if retiredSweep.storedEntries > 0}
				<p>{m.diagnostics_retired_sweep_stored({ count: retiredSweep.storedEntries })}</p>
			{/if}
		</section>
	{/if}

	{#if legacySweep.visible}
		<section>
			<h2>{m.diagnostics_section_legacy_sweep()}</h2>
			<p>{m.diagnostics_legacy_sweep_description()}</p>
			<ul>
				{#each legacySweep.paths as path (path)}
					<li><code>{path}</code></li>
				{/each}
			</ul>
		</section>
	{/if}

	<section>
		<h2>
			{#if history === null}
				{m.diagnostics_section_history_title_loading()}
			{:else}
				{m.diagnostics_section_history_title({ count: history.length })}
			{/if}
		</h2>
		{#if view.historyError}
			<p>{m.diagnostics_history_error()} {view.historyError}</p>
		{:else if history === null}
			<p>{m.diagnostics_history_loading()}</p>
		{:else if history.length === 0}
			<p>{m.diagnostics_history_empty()}</p>
		{:else}
			<ul>
				{#each history as entry (entry.id)}
					<li>
						{m.diagnostics_history_entry_format({
							title: entry.title,
							epNo: entry.ep_no,
							id: entry.id
						})}
					</li>
				{/each}
			</ul>
		{/if}
		{#if view.historyClearError}<p>
				{m.diagnostics_history_clear_error()}
				{view.historyClearError}
			</p>{/if}
		<p>
			<button type="button" onclick={() => actions.refresh()} disabled={view.busy}
				>{m.diagnostics_refresh_button()}</button
			>
			<button
				type="button"
				onclick={() => actions.clearHistory()}
				disabled={view.busy || history?.length === 0}
			>
				{m.diagnostics_clear_history_button()}
			</button>
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_metadata_cache()}</h2>
		<p>
			{m.diagnostics_metadata_cache_description()}
		</p>
		<p>
			<button type="button" onclick={() => actions.clearCache()} disabled={view.busy}
				>{m.diagnostics_clear_metadata_cache_button()}</button
			>
		</p>
		{#if view.cacheCleared}<p>{m.diagnostics_cache_clear_success()}</p>{/if}
		{#if view.cacheError}<p>{m.diagnostics_cache_clear_error()} {view.cacheError}</p>{/if}
	</section>

	<section>
		<h2>{m.diagnostics_section_banner_framing()}</h2>
		<p>{m.diagnostics_banner_framing_description()}</p>
		<p>
			<label>
				<input
					type="checkbox"
					checked={heroPrefs.controls}
					onchange={(e) => setFramingControls((e.currentTarget as HTMLInputElement).checked)}
				/>
				{m.diagnostics_banner_framing_toggle_label()}
			</label>
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_row_style()}</h2>
		<p>{m.diagnostics_row_style_description()}</p>
		<p>
			<label>
				<input
					type="checkbox"
					checked={rowPrefs.controls}
					onchange={(e) => setRowStyleControls((e.currentTarget as HTMLInputElement).checked)}
				/>
				{m.diagnostics_row_style_toggle_label()}
			</label>
		</p>
	</section>

	<section>
		<h2>{m.watchorder_diag_section()}</h2>
		<p>{m.watchorder_diag_description()}</p>
		<p>
			<label>
				<input
					type="checkbox"
					checked={mockPrefs.enabled}
					onchange={(e) => setMock('enabled', (e.currentTarget as HTMLInputElement).checked)}
				/>
				{m.watchorder_diag_toggle()}
			</label>
		</p>
		<p>
			<label>
				<input
					type="checkbox"
					checked={mockPrefs.switcher}
					onchange={(e) => setMock('switcher', (e.currentTarget as HTMLInputElement).checked)}
				/>
				{m.watchorder_diag_switcher()}
			</label>
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_search_kitsu()}</h2>
		<p>
			<a href={resolve('/search')}>{m.diagnostics_open_search_link_text()}</a
			>{m.diagnostics_open_search_link_hint()}
		</p>
	</section>

	<section>
		<h2>{m.diagnostics_section_test_stream()}</h2>
		<p>
			<a href={resolve('/play')}>{m.diagnostics_open_test_stream_link_text()}</a
			>{m.diagnostics_open_test_stream_link_hint()}
		</p>
	</section>
</main>

<style>
	/* Diagnostics is a utilitarian, design-pass-exempt page. Just
	   enough wrapping to keep the back button and prose off the rail
	   edge — same gutter the rest of the app uses. */
	.page {
		max-inline-size: var(--content-max);
		padding: var(--space-5) var(--space-8) var(--space-8);
	}
	.page section {
		margin-block-start: var(--space-5);
	}
</style>
