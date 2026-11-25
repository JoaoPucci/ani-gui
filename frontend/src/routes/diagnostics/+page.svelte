<!--
  Backend Status — proves IPC works end-to-end. Loads app_info +
  history_list on mount, exposes history_clear. Plain HTML on purpose;
  the design pass (M3) will replace this entirely.
-->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { beginVisit } from '$lib/navigation/visit';
	import { resolve } from '$app/paths';
	import {
		appInfo,
		heroWalk,
		historyClear,
		historyList,
		metaCacheClear,
		onboardingDebug,
		onboardingDebugSeen,
		type OnboardingDebugTip
	} from '$lib/api';
	import {
		asRunnable,
		installLabel,
		readTipKeys,
		windowLabel,
		type TipKeysState
	} from '$lib/diagnostics/tip-keys';
	import { onboarding } from '$lib/onboarding/tips.svelte';
	import { previewable } from '$lib/onboarding/on-demand';
	import { stepWords } from '$lib/onboarding/words';
	import {
		createDiagnosticsActions,
		INITIAL_DIAGNOSTICS,
		type DiagnosticsState
	} from '$lib/diagnostics/history-actions';
	import { legacySweepView } from '$lib/diagnostics/legacy-sweep';
	import { retiredSweepView } from '$lib/diagnostics/retired-sweep';
	import { cacheReclaimView } from '$lib/diagnostics/cache-reclaim';
	import {
		artLabel,
		originLabel,
		outcomeLabel,
		readHeroWalk,
		statusLabel,
		type HeroWalkState
	} from '$lib/diagnostics/hero-walk';
	import { browserStorage, loadHeroPrefs, saveHeroPrefs } from '$lib/hero/prefs';
	import { loadRowPrefs, saveRowPrefs } from '$lib/rows/prefs';
	import { loadStartLook, saveStartLook, type StartLook } from '$lib/play/start-look';
	import { m } from '$lib/paraglide/messages';
	import { describeError } from '$lib/play/error-copy';

	// The page's actions and what they leave on show live in
	// $lib/diagnostics/history-actions; this renders its state.
	let view = $state<DiagnosticsState>({ ...INITIAL_DIAGNOSTICS });
	// Its reads are cancelled when the page goes ($lib/navigation/visit).
	const reads = beginVisit();
	onDestroy(reads.end);
	const actions = createDiagnosticsActions({
		appInfo: () => appInfo({ signal: reads.signal }),
		historyList: () => historyList({ signal: reads.signal }),
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

	// Whether the home page shows its row style switch, kept with the
	// row preferences the same way — see $lib/rows/prefs.
	let rowPrefs = $state(loadRowPrefs(browserStorage()));
	function setRowStyleControls(shown: boolean) {
		rowPrefs = { ...rowPrefs, controls: shown };
		saveRowPrefs(browserStorage(), rowPrefs);
	}

	// The look the player's start veil takes, kept the same way — see
	// $lib/play/start-look. The next episode opened picks it up.
	let startLook = $state(loadStartLook(browserStorage()));
	function setStartLook(look: StartLook) {
		startLook = look;
		saveStartLook(browserStorage(), look);
	}

	// The home hero's last pick, as the backend made it — see
	// $lib/diagnostics/hero-walk. Read once, when the page opens.
	let heroWalkState = $state<HeroWalkState>({ kind: 'loading' });

	// The feature tips, by key, with this install's state — see
	// $lib/diagnostics/tip-keys. Read when the page opens, and again
	// after a seen toggle.
	let tipKeys = $state<TipKeysState>({ kind: 'loading' });
	const readTips = () =>
		readTipKeys(() => onboardingDebug({ signal: reads.signal })).then((s) => {
			if (!reads.signal.aborted) tipKeys = s;
		});
	// Every toggle is held while one change is out. One the backend refuses
	// goes back and says so, and a list that cannot be read again keeps
	// the last one read.
	let tipBusy = $state(false);
	let tipSeenFailed = $state(false);
	async function setTipSeen(tip: OnboardingDebugTip, box: HTMLInputElement) {
		const want = box.checked;
		tipBusy = true;
		tipSeenFailed = false;
		try {
			await onboardingDebugSeen(tip.id, want);
		} catch {
			box.checked = tip.seen;
			tipSeenFailed = true;
		}
		const again = await readTipKeys(() => onboardingDebug({ signal: reads.signal }));
		if (!reads.signal.aborted && again.kind === 'read') tipKeys = again;
		else tipSeenFailed = true;
		tipBusy = false;
	}
	const kindLabel = (k: OnboardingDebugTip['kind']) =>
		k === 'single'
			? m.diagnostics_tips_kind_single()
			: k === 'sequence'
				? m.diagnostics_tips_kind_sequence()
				: m.diagnostics_tips_kind_entry();

	onMount(() => {
		void actions.refresh();
		void readHeroWalk(() => heroWalk({ signal: reads.signal })).then((s) => {
			if (!reads.signal.aborted) heroWalkState = s;
		});
		void readTips();
	});
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

	<section data-testid="hero-walk">
		<h2>{m.diagnostics_section_hero_walk()}</h2>
		<p>{m.diagnostics_hero_walk_description()}</p>
		{#if heroWalkState.kind === 'loading'}
			<p>{m.diagnostics_hero_walk_loading()}</p>
		{:else if heroWalkState.kind === 'failed'}
			<p>{m.diagnostics_hero_walk_failed()}</p>
		{:else}
			{@const flag = heroWalkState.view.flag}
			{@const walk = heroWalkState.view.walk}
			<p data-testid="hero-walk-flag" data-on={String(flag.on)} data-origin={flag.origin}>
				{m.diagnostics_hero_walk_flag({
					name: flag.name,
					state: flag.on ? m.diagnostics_hero_walk_flag_on() : m.diagnostics_hero_walk_flag_off()
				})}
				<br />
				{m.diagnostics_hero_walk_origin({ origin: originLabel(flag.origin) })}
			</p>
			{#if flag.on}<p>{m.diagnostics_hero_walk_signed_note()}</p>{/if}
			{#if walk === null}
				<p>{m.diagnostics_hero_walk_none()}</p>
			{:else}
				<p>
					{m.diagnostics_hero_walk_summary({
						read: walk.steps.length,
						entries: walk.entries,
						preference: walk.airing_first
							? m.diagnostics_hero_walk_flag_on()
							: m.diagnostics_hero_walk_flag_off()
					})}
				</p>
				<table class="hero-walk">
					<thead>
						<tr>
							<th scope="col">{m.diagnostics_hero_walk_col_rank()}</th>
							<th scope="col">{m.diagnostics_hero_walk_col_title()}</th>
							<th scope="col">{m.diagnostics_hero_walk_col_art()}</th>
							<th scope="col">{m.diagnostics_hero_walk_col_status()}</th>
							<th scope="col">{m.diagnostics_hero_walk_col_preference()}</th>
							<th scope="col">{m.diagnostics_hero_walk_col_outcome()}</th>
						</tr>
					</thead>
					<tbody>
						{#each walk.steps as step (step.rank)}
							<tr
								data-testid="hero-walk-step"
								data-id={step.id}
								data-verdict={step.verdict.kind}
								data-slot={step.verdict.kind === 'taken' ? String(step.verdict.slot) : undefined}
							>
								<td>{step.rank}</td>
								<td>{step.title}</td>
								<td>{artLabel(step.art)}</td>
								<td>{statusLabel(step)}</td>
								<td>
									{step.preference_applied
										? m.diagnostics_hero_walk_yes()
										: m.diagnostics_hero_walk_no()}
								</td>
								<td>{outcomeLabel(step.verdict)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		{/if}
	</section>

	<section data-testid="tip-keys">
		<h2>{m.diagnostics_section_tips()}</h2>
		<p>{m.diagnostics_tips_description()}</p>
		{#if tipKeys.kind === 'loading'}
			<p>{m.diagnostics_tips_loading()}</p>
		{:else if tipKeys.kind === 'failed'}
			<p>{m.diagnostics_tips_failed()}</p>
		{:else}
			{@const view = tipKeys.view}
			<p>
				{m.diagnostics_tips_version({ version: view.version })}
				{installLabel(view)}
			</p>
			{#if tipSeenFailed}
				<p role="alert">{m.diagnostics_tips_seen_failed()}</p>
			{/if}
			<table class="hero-walk">
				<thead>
					<tr>
						<th scope="col">{m.diagnostics_tips_col_key()}</th>
						<th scope="col">{m.diagnostics_tips_col_tool()}</th>
						<th scope="col">{m.diagnostics_tips_col_window()}</th>
						<th scope="col">{m.diagnostics_tips_col_seen()}</th>
						<th scope="col">{m.diagnostics_tips_col_show()}</th>
					</tr>
				</thead>
				<tbody>
					{#each view.tips as tip (tip.id)}
						<tr data-testid="tip-key" data-id={tip.id} data-pending={String(tip.pending)}>
							<td><code>{tip.id}</code></td>
							<td>{kindLabel(tip.kind)}</td>
							<td>{windowLabel(tip)}</td>
							<td>
								<label>
									<input
										type="checkbox"
										checked={tip.seen}
										disabled={tip.on_demand || tipBusy}
										onchange={(e) => setTipSeen(tip, e.currentTarget)}
									/>
									{m.diagnostics_tips_seen()}
								</label>
							</td>
							<td>
								{#if onboarding.armed?.id === tip.id}
									<span>{m.diagnostics_tips_waiting()}</span>
									<button type="button" onclick={() => onboarding.disarm()}
										>{m.diagnostics_tips_cancel()}</button
									>
								{:else if previewable(tip) && stepWords(tip.id, 1)}
									<button type="button" onclick={() => onboarding.showNow(asRunnable(tip))}
										>{m.diagnostics_tips_show()}</button
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</section>

	<section>
		<h2>{m.diagnostics_section_start_look()}</h2>
		<fieldset class="start-look">
			<legend>{m.diagnostics_start_look_description()}</legend>
			<label>
				<input
					type="radio"
					name="start-look"
					checked={startLook === 'still'}
					onchange={() => setStartLook('still')}
				/>
				{m.diagnostics_start_look_still()}
			</label>
			<label>
				<input
					type="radio"
					name="start-look"
					checked={startLook === 'card'}
					onchange={() => setStartLook('card')}
				/>
				{m.diagnostics_start_look_card()}
			</label>
		</fieldset>
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
		<h2>{m.diagnostics_section_search_kitsu()}</h2>
		<p>
			<a href={resolve('/search')}>{m.diagnostics_open_search_link_text()}</a
			>{m.diagnostics_open_search_link_hint()}
		</p>
	</section>
</main>

<style>
	/* The start-look radios: one named group, laid out like the page's
	   other switches. */
	.start-look {
		display: grid;
		gap: 0.4rem;
		margin: 0;
		padding: 0;
		border: 0;
	}
	.start-look legend {
		margin-block-end: 0.4rem;
		padding: 0;
	}
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
	/* The hero walk: a plain table, wide enough to read a row across. */
	.hero-walk {
		border-collapse: collapse;
	}
	.hero-walk th,
	.hero-walk td {
		padding-block: 0.2rem;
		padding-inline: 0 var(--space-4);
		text-align: start;
		vertical-align: top;
	}
</style>
