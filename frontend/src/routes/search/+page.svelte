<!--
  Kitsu search — dense scannable result grid. M3 design pass.
  Aesthetic note: late-night repertory cinema, programmed by a librarian.
  The motifs are restraint: serif voice, mono numerals, hairline rules.
-->
<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import {
		imageProxyUrl,
		kitsuSearchPage,
		kitsuTopRated,
		kitsuTrending,
		settingsGet,
		type Config,
		type KitsuAnimeRef,
		type SearchOrder
	} from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import SearchProgress from '$lib/components/SearchProgress.svelte';
	import Strip from '$lib/components/Strip.svelte';
	import { filterAvailable } from '$lib/availability/filter';
	import { filterAvailableProgressive } from '$lib/availability/progressive';
	import { pickAvailabilityMode } from '$lib/availability/mode';
	import { createSearchRunner } from '$lib/search/run-search';
	import {
		measured,
		shouldAutoLoad,
		spent,
		UNMEASURED,
		type PagingPhase,
		type Reach
	} from '$lib/search/paging';
	import { watchSentinel, type SentinelWatch } from '$lib/search/sentinel';
	import { staggerSlots } from '$lib/search/stagger';
	import { keepFocus } from '$lib/search/keep-focus';
	import { subtypeLabel } from '$lib/search/subtype-label';
	import { resultCount } from '$lib/search/result-count';
	import { m } from '$lib/paraglide/messages';
	import { describeError } from '$lib/play/error-copy';

	let submitted = $state(''); // the query whose results are on screen.
	let results = $state<KitsuAnimeRef[] | null>(null);
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let busy = $state(false);
	// Infinite scroll: the runner's paging phase, and what the latest
	// measurement of the marker under the grid said — whether it is in
	// reach, and whether that is still news ($lib/search/paging.ts).
	let phase = $state<PagingPhase>({ kind: 'loading' });
	let reach = $state<Reach>(UNMEASURED);
	let sentinelWatch: SentinelWatch | null = null;

	// The grid just changed shape — a page settled, a type filter came
	// on or off — so the last measurement describes a layout that is
	// gone: spend it, and — once the new layout is in the DOM — ask the
	// observer for a measurement of it.
	function gridChanged() {
		reach = spent(reach);
		void tick().then(() => sentinelWatch?.remeasure());
	}

	// Discovery rows for the empty state — same data as the home page so
	// users have something to browse when they arrive without a query.
	let trending = $state<KitsuAnimeRef[] | null>(null);
	let topRated = $state<KitsuAnimeRef[] | null>(null);

	// User settings — only `mode` matters here, used to scope the
	// availability filter to the user's sub/dub preference. Settings
	// are loaded once on mount; navigating back to /search re-mounts
	// the page so any changes the user made on settings or detail
	// pages get picked up.
	let config = $state<Config | null>(null);

	// The order comes from Kitsu: choosing one starts the search over
	// in it, and each page that loads continues it below what is shown.
	// Sorting only the loaded pages here would re-sort the whole grid
	// as every page landed. Kitsu cannot sort a text search by title,
	// so there is no title order. The type filter stays here: it hides
	// cards, it does not move them.
	type SortKey = SearchOrder;
	// Message functions, called at render so a locale switch relabels.
	const SORT_LABELS: Record<SortKey, () => string> = {
		relevance: m.search_sort_relevance,
		year: m.search_sort_year,
		rating: m.search_sort_rating
	};
	const SUBTYPES = ['TV', 'movie', 'special', 'OVA', 'ONA', 'music'];
	let sortKey = $state<SortKey>('relevance');
	const activeSubtypes = new SvelteSet<string>();

	function toggleSubtype(s: string) {
		if (activeSubtypes.has(s)) activeSubtypes.delete(s);
		else activeSubtypes.add(s);
		gridChanged();
	}

	const displayed = $derived.by(() => {
		if (!results) return null;
		let list = results.slice();
		if (activeSubtypes.size > 0) {
			list = list.filter((h) => activeSubtypes.has(h.subtype ?? 'TV'));
		}
		return list;
	});

	const count = $derived(displayed?.length ?? 0);

	// The grid is empty but the runner is already looking a page
	// deeper — reads as searching, not as "nothing matched".
	const searching = $derived(busy || (results?.length === 0 && phase.kind === 'loading-more'));
	const shown = $derived((results?.length ?? 0) > 0);
	const moreToCome = $derived(
		phase.kind === 'idle' ||
			phase.kind === 'loading-more' ||
			phase.kind === 'failed' ||
			phase.kind === 'held'
	);
	const counter = $derived(resultCount(count, moreToCome));

	// Each card rises from the slot it arrived in, so an appended
	// page staggers from its own first card ($lib/search/stagger.ts).
	let slots = new Map<string, number>();
	const slotOf = $derived.by(() => (slots = staggerSlots(slots, displayed ?? [])));

	$effect(() => {
		// One measurement, one page: the load spends it.
		if (shouldAutoLoad(phase, reach, activeSubtypes.size > 0)) {
			reach = spent(reach);
			searchRunner.loadMore();
		}
	});

	// Focus handed on by a leaving paging button lands here: a silent
	// spot at the head of the folio, outside its live region, so the
	// status line is announced once — by the live region — not again
	// on focus.
	let focusAnchor = $state<HTMLElement | null>(null);
	// A paging button leaves when the phase it belongs to does — Try
	// again as its page loads, Load more at the end. If it held focus,
	// focus moves to the folio's anchor rather than falling to the page
	// body ($lib/search/keep-focus.ts).
	const keepFocusInFolio = (node: HTMLElement) => keepFocus(node, () => focusAnchor);

	function sentinel(node: HTMLElement) {
		const watch = watchSentinel(node, (inReach) => (reach = measured(inReach)));
		sentinelWatch = watch;
		return {
			destroy() {
				watch.stop();
				if (sentinelWatch === watch) sentinelWatch = null;
				reach = UNMEASURED;
			}
		};
	}

	onMount(async () => {
		// Resolve settings before kicking off the discovery requests
		// so the filter calls below see the user's actual mode. Naive
		// parallel start would race — trending could resolve first
		// and call filterAvailable(t, pickAvailabilityMode(null))
		// which falls back to 'sub' and never reflects the real mode.
		// settingsGet is a single fast IPC; awaiting once on mount is
		// cheap.
		try {
			config = await settingsGet();
		} catch {
			// Settings fetch failed — fall through with config null;
			// pickAvailabilityMode handles that with 'sub' default.
		}
		const mode = pickAvailabilityMode(config);
		kitsuTrending()
			.then((t) => filterAvailable(t, mode))
			.then((t) => (trending = t))
			.catch(() => (trending = []));
		kitsuTopRated()
			.then((t) => filterAvailable(t, mode))
			.then((t) => (topRated = t))
			.catch(() => (topRated = []));
	});

	// Drive the search off the URL ?q=… param. The global topbar from
	// +layout.svelte writes that param on submit; this page reacts to
	// it. As a bonus, search history (browser back/forward) and deep
	// links work for free.
	$effect(() => {
		const q = page.url.searchParams.get('q')?.trim() ?? '';
		if (q && q !== submitted) {
			void runSearch(q);
		}
	});

	// All async search state transitions live in the runner
	// ($lib/search/run-search.ts): render-then-prune emissions from
	// the progressive filter, plus a generation token that silences
	// superseded runs — including a re-run of the SAME query text
	// (A → B → A), which a text comparison could not distinguish.
	// The closures below are the page-side adapter, one Svelte
	// reassignment each.
	const searchRunner = createSearchRunner({
		searchPage: kitsuSearchPage,
		getConfig: async () => {
			// Same race as onMount: a fast ?q= deeplink can fire before
			// settingsGet has resolved on first load. The runner caches
			// the result so DUB users never search under the 'sub'
			// fallback twice.
			if (!config) config = await settingsGet();
			return config;
		},
		pickMode: pickAvailabilityMode,
		filter: (items, mode, emit, signal) =>
			filterAvailableProgressive(items, mode, emit, undefined, undefined, signal),
		onResults: (visible) => {
			results = visible;
		},
		onError: (e) => {
			error = { headline: m.search_error_headline(), detail: describeError(e) };
			results = null;
		},
		onBusy: (b) => {
			busy = b;
		},
		onPhase: (p) => {
			phase = p;
			// A page settled: the grid grew (or a retry, Keep looking or
			// Load more brought it back to idle). Only a measurement of
			// the grid as it now stands may load the next page.
			if (p.kind === 'idle') gridChanged();
		}
	});

	async function runSearch(q: string) {
		error = null;
		submitted = q;
		// The marker's last report belongs to the previous results; it
		// unmounts while this search's first page loads, and the
		// observer made when it mounts again reports its initial state.
		reach = UNMEASURED;
		await searchRunner.run(q, sortKey);
	}

	/** Choose the order the results come in: the search starts over,
	 *  with Kitsu sorting it. */
	function chooseOrder(key: SortKey) {
		if (key === sortKey) return;
		sortKey = key;
		if (submitted) void runSearch(submitted);
	}

	function posterFor(hit: KitsuAnimeRef): string | null {
		// `original` last — see PosterCard for the chain rationale.
		const url =
			hit.poster_image?.medium ??
			hit.poster_image?.large ??
			hit.poster_image?.small ??
			hit.poster_image?.original ??
			null;
		return imageProxyUrl(url);
	}
	function posterHoverFor(hit: KitsuAnimeRef): string | null {
		const url =
			hit.poster_image?.large ?? hit.poster_image?.medium ?? hit.poster_image?.original ?? null;
		return imageProxyUrl(url);
	}
	function yearOf(hit: KitsuAnimeRef): string | null {
		return hit.start_date ? hit.start_date.slice(0, 4) : null;
	}
	function ratingOf(hit: KitsuAnimeRef): string | null {
		if (hit.average_rating === null) return null;
		return (hit.average_rating / 10).toFixed(1);
	}
</script>

<svelte:head>
	<title>{m.app_page_title_search()}</title>
</svelte:head>

<div class="page">
	<SearchProgress {busy} />
	<header class="page-header">
		<h1 class="page-title">{m.search_title()}</h1>
		{#if !submitted}
			<p class="page-sub">{m.search_subtitle_empty()}</p>
		{/if}
	</header>

	<section class="results-meta" aria-live="polite">
		{#if submitted}
			<p class="eyebrow">
				<span class="eyebrow-key">{m.search_eyebrow_key()}</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">{submitted}</span>
				<span class="eyebrow-sep" aria-hidden="true">·</span>
				{#if searching}
					<span class="eyebrow-count">{m.search_status_searching()}</span>
				{:else}
					<span class="eyebrow-count">
						<span class="num">{counter.num}</span>
						<span class="word">{counter.word}</span>
					</span>
				{/if}
			</p>
		{:else}
			<p class="eyebrow">
				<span class="eyebrow-key">{m.search_eyebrow_key_empty()}</span>
				<span class="eyebrow-rule" aria-hidden="true"></span>
				<span class="eyebrow-value">{m.search_eyebrow_value_empty()}</span>
			</p>
			<p class="eyebrow-caption">{m.search_empty_caption()}</p>
		{/if}
	</section>

	<!-- Mounted for as long as the search has answered, with shows,
	     with nothing, or with a failure: a reorder can open on a page
	     the availability filter empties, a search can end with nothing,
	     and a reorder's first page can fail, and in each case the chip
	     the reader pressed stays in place with its focus. -->
	{#if submitted && (results !== null || error !== null)}
		<section class="controls" aria-label={m.search_controls_aria_label()}>
			<div class="control-group">
				<span class="control-label">{m.search_control_sort_label()}</span>
				<!-- Single-select radio behaviour via aria-pressed; same
				     chip style as Type for visual consistency. -->
				<div class="chips" role="radiogroup" aria-label={m.search_chips_sort_aria_label()}>
					{#each Object.entries(SORT_LABELS) as [key, label] (key)}
						<button
							type="button"
							class="chip"
							class:active={sortKey === key}
							role="radio"
							aria-checked={sortKey === key}
							onclick={() => chooseOrder(key as SortKey)}
						>
							{label()}
						</button>
					{/each}
				</div>
			</div>
			<div class="control-group">
				<span class="control-label">{m.search_control_type_label()}</span>
				<div class="chips" role="group" aria-label={m.search_chips_type_aria_label()}>
					{#each SUBTYPES as s (s)}
						<button
							type="button"
							class="chip"
							class:active={activeSubtypes.has(s)}
							aria-pressed={activeSubtypes.has(s)}
							onclick={() => toggleSubtype(s)}
						>
							{subtypeLabel(s)}
						</button>
					{/each}
					{#if activeSubtypes.size > 0}
						<button
							type="button"
							class="chip chip-clear"
							onclick={() => {
								activeSubtypes.clear();
								gridChanged();
							}}
							aria-label={m.search_chips_clear_button_aria_label()}
						>
							{m.search_chips_clear_button_aria_label()}
						</button>
					{/if}
				</div>
			</div>
		</section>
	{/if}

	{#if error}
		<div class="state state-error" role="alert">
			<p class="state-headline">{error.headline}</p>
			{#if error.detail}<p class="state-detail">{error.detail}</p>{/if}
		</div>
	{:else if (busy && !results) || (results?.length === 0 && phase.kind === 'loading-more')}
		<ul class="grid" aria-busy="true" aria-label={m.search_skeleton_aria_label()}>
			{#each Array.from({ length: 12 }, (_, k) => k) as i (i)}
				<li class="card card-skeleton" style="--i: {i}">
					<div class="poster poster-skeleton"></div>
					<div class="line line-skeleton" style="inline-size: 70%"></div>
					<div class="line line-skeleton line-thin" style="inline-size: 40%"></div>
				</li>
			{/each}
		</ul>
	{:else if results !== null && results.length === 0 && !moreToCome}
		<div class="state state-empty">
			<p class="state-headline">
				{m.search_empty_headline_prefix()}<em>{submitted ?? ''}</em
				>{m.search_empty_headline_suffix()}
			</p>
			<p class="state-detail">
				{m.search_empty_detail()}
			</p>
		</div>
	{:else if !submitted}
		<!-- No query yet: surface the same discovery rows the home page uses. -->
		{#if trending && trending.length > 0}
			<Strip
				eyebrow={m.search_discovery_trending_eyebrow()}
				caption={m.search_discovery_trending_caption()}
			>
				{#each trending as hit (hit.id)}
					<PosterCard anime={hit} />
				{/each}
			</Strip>
		{/if}
		{#if topRated && topRated.length > 0}
			<Strip
				eyebrow={m.search_discovery_top_rated_eyebrow()}
				caption={m.search_discovery_top_rated_caption()}
			>
				{#each topRated as hit (hit.id)}
					<PosterCard anime={hit} />
				{/each}
			</Strip>
		{/if}
	{:else if displayed !== null && displayed.length === 0 && results !== null && results.length > 0}
		<div class="state state-empty">
			<p class="state-headline">{m.search_filtered_empty_headline()}</p>
			<p class="state-detail">{m.search_filtered_empty_detail()}</p>
		</div>
	{:else if displayed !== null && displayed.length > 0}
		<ul class="grid">
			{#each displayed as hit, i (hit.id)}
				{@const poster = posterFor(hit)}
				{@const posterHover = posterHoverFor(hit)}
				{@const accent = accentFor(hit.id)}
				{@const year = yearOf(hit)}
				{@const rating = ratingOf(hit)}
				<li class="card" style="--i: {slotOf.get(hit.id) ?? i}; --accent: {accent};">
					<a href={resolve('/anime/[id]', { id: hit.id })} class="card-link">
						<span class="poster">
							{#if poster}
								<img src={poster} alt="" loading="lazy" decoding="async" />
								{#if posterHover && posterHover !== poster}
									<img
										class="poster-hover"
										src={posterHover}
										alt=""
										loading="lazy"
										decoding="async"
									/>
								{/if}
							{:else}
								<span class="poster-placeholder" aria-hidden="true">
									<span class="poster-placeholder-title">{hit.canonical_title}</span>
								</span>
							{/if}
						</span>
						<span class="card-body">
							<span class="card-title">{hit.canonical_title}</span>
							<span class="card-meta">
								<span>{subtypeLabel(hit.subtype)}</span>
								{#if year}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span>{year}</span>
								{/if}
								{#if hit.episode_count}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span
										>{hit.episode_count}
										{m.search_card_eps_suffix({ count: hit.episode_count })}</span
									>
								{/if}
								{#if rating}
									<span class="card-meta-sep" aria-hidden="true">·</span>
									<span class="card-meta-rating">{rating}</span>
								{/if}
							</span>
						</span>
					</a>
				</li>
			{/each}
		</ul>
		{#if phase.kind === 'loading-more'}
			<!-- One row of the next page's slots, held open below the
			     grid where its cards will land. The row is as wide as the
			     grid's columns: extra slots fall into rows clipped to zero. -->
			<ul class="grid grid-ghost" aria-hidden="true">
				{#each Array.from({ length: 12 }, (_, k) => k) as k (k)}
					<li class="card card-skeleton" style="--i: {k}">
						<div class="poster poster-skeleton"></div>
						<div class="line line-skeleton" style="inline-size: 70%"></div>
						<div class="line line-skeleton line-thin" style="inline-size: 40%"></div>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}

	<!-- Mounted once each search's first page has settled, and
	     unmounted while the next search's first page loads: every search
	     gets a fresh observer, which reports its initial state (an
	     observer reports only changes, and a grid short for two searches
	     in a row gives it none), measured against this search's grid,
	     not the previous one still on screen. -->
	{#if submitted && results !== null && !error && phase.kind !== 'loading'}
		<div class="sentinel" use:sentinel aria-hidden="true"></div>
	{/if}
	{#if submitted}
		<div class="folio" data-phase={phase.kind} data-settled={results !== null}>
			<span class="folio-anchor" tabindex="-1" bind:this={focusAnchor}></span>
			<!-- Once a first page is shown: one rule, always in place, so a
			     state change recolours it instead of moving what is below.
			     Before that the folio is the bare live region. -->
			{#if results !== null}
				<span
					data-testid="search-paging-rule"
					class="folio-rule"
					class:folio-rule-drawing={phase.kind === 'loading-more' && shown}
					class:folio-rule-closed={phase.kind === 'end' && shown}
					class:folio-rule-failed={phase.kind === 'failed'}
					class:folio-rule-held={phase.kind === 'held'}
					aria-hidden="true"
				></span>
			{/if}
			<!-- The one live region of the paging, mounted with the query
			     so a screen reader hears each change; buttons stay outside. -->
			<p class="folio-line" data-testid="search-paging" aria-live="polite" aria-atomic="true">
				{#if phase.kind === 'loading-more' && shown}
					{m.search_paging_loading()}
				{:else if phase.kind === 'end' && shown}
					{m.search_paging_end({ query: submitted })}
				{:else if phase.kind === 'failed'}
					{m.search_paging_failed()}
					{#if describeError(phase.error)}
						<span class="folio-detail">{describeError(phase.error)}</span>
					{/if}
				{:else if phase.kind === 'held'}
					{m.search_paging_held()}
				{/if}
			</p>
			{#if phase.kind === 'failed'}
				<button
					type="button"
					class="folio-action"
					use:keepFocusInFolio
					onclick={() => searchRunner.loadMore()}
				>
					{m.search_paging_retry()}
				</button>
			{:else if (phase.kind === 'idle' || phase.kind === 'loading-more') && activeSubtypes.size > 0}
				<!-- A type filter turns the auto-load off (see shouldAutoLoad);
				     the next page is the reader's to ask for. The button stays
				     mounted while that page loads, so focus stays on it. -->
				<button
					type="button"
					class="folio-action"
					aria-disabled={phase.kind === 'loading-more'}
					use:keepFocusInFolio
					onclick={() => searchRunner.loadMore()}
				>
					{m.search_paging_load_more()}
				</button>
			{:else if phase.kind === 'held'}
				<button
					type="button"
					class="folio-action"
					use:keepFocusInFolio
					onclick={() => searchRunner.loadMore()}
				>
					{m.search_paging_keep_looking()}
				</button>
			{/if}
		</div>
	{/if}
</div>

<style>
	.page {
		max-inline-size: var(--content-max-wide);
		margin-inline: auto;
		padding: var(--space-7) var(--space-8) var(--space-8);
	}

	.page-header {
		margin-block-end: var(--space-6);
	}
	.page-title {
		margin: 0;
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-l);
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		color: var(--bone-100);
	}
	.page-sub {
		margin: var(--space-3) 0 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		line-height: var(--leading-prose);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}

	.results-meta {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		margin-block-end: var(--space-6);
		padding-block-end: var(--space-4);
		/* manga-page divider: 1px solid + 1px hairline 4px below */
		border-block-end: 1px solid var(--ink-200);
		box-shadow: 0 5px 0 -4px var(--ink-200);
	}
	.eyebrow-sep {
		color: color-mix(in oklab, var(--bone-100) 30%, transparent);
	}
	.eyebrow-count {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-variant-numeric: tabular-nums lining-nums;
	}
	.eyebrow-count .num {
		color: var(--bone-100);
	}
	.eyebrow-caption {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}

	/* — Grid: dense, never centered, never gappy. */
	.grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
		gap: var(--space-6) var(--space-5);
	}

	.card {
		--i: 0;
		animation: card-in var(--dur-med) var(--ease-out-soft) both;
		animation-delay: calc(var(--i) * var(--dur-stagger));
	}
	@keyframes card-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.card-link {
		display: block;
		color: inherit;
		transition: transform var(--dur-med) var(--ease-out-elastic);
		will-change: transform;
	}
	.card-link:hover {
		transform: translateY(-4px);
	}
	.card-link:focus-visible {
		outline: none;
	}
	.card-link:focus-visible .poster {
		box-shadow:
			var(--shadow-card-rest),
			0 0 0 2px var(--bone-100);
	}

	.poster {
		position: relative;
		display: block;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow: var(--shadow-card-rest);
		transition: box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.card-link:hover .poster {
		box-shadow: var(--shadow-card-hover);
	}
	.poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		transition:
			opacity var(--dur-med) var(--ease-out-soft),
			transform var(--dur-slow) var(--ease-out-soft);
	}
	.poster img.poster-hover {
		opacity: 0;
	}
	.card-link:hover .poster img:not(.poster-hover) {
		transform: scale(1.04);
	}
	.card-link:hover .poster .poster-hover {
		opacity: 1;
		transform: scale(1.04);
	}

	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-4);
		background: linear-gradient(180deg, var(--ink-100) 0%, var(--ink-050) 100%);
		border-block-end: 1px solid var(--ink-200);
	}
	.poster-placeholder-title {
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-m);
		line-height: var(--leading-tight);
		color: var(--bone-200);
		text-align: center;
		/* Keep long titles from blowing the card. */
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 5;
		line-clamp: 5;
		-webkit-box-orient: vertical;
	}

	.card-body {
		display: block;
		padding-block-start: var(--space-3);
	}
	.card-title {
		display: block;
		font-family: var(--font-body);
		font-weight: 500;
		font-size: 1rem;
		line-height: 1.3;
		color: var(--bone-100);
		/* clamp to 2 lines, keep grid rhythm */
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}
	.card-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		/* row-gap then column-gap — values keep tight horizontal rhythm
		   while wrapped lines get a smaller vertical gap. */
		gap: var(--space-1) var(--space-2);
		margin-block-start: var(--space-2);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-micro);
		font-weight: 500;
		color: var(--bone-200);
		letter-spacing: var(--tracking-meta);
	}
	/* Each value (and separator) stays as one unit when the row wraps.
	   Without nowrap, "366 eps" splits between "366" and "eps", so the
	   rating slides up next to "366" while "eps" drops to a new line —
	   which is the misalignment the user flagged. */
	.card-meta > span {
		white-space: nowrap;
	}
	.card-meta-sep {
		color: var(--bone-400);
	}
	.card-meta-rating::before {
		content: '★ ';
		color: var(--accent);
	}

	/* — Skeletons: opacity pulse, no shimmer-stripe gradient. */
	.card-skeleton .poster-skeleton {
		inline-size: 100%;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line {
		block-size: 0.75rem;
		background: var(--ink-100);
		border-radius: 2px;
		margin-block-start: var(--space-3);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line-thin {
		block-size: 0.6rem;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.55;
		}
	}

	/* — Paging folio: the line under the grid. One rule carries the
	   state — drawn in persimmon while the next page loads, closed
	   with the manga-page double rule at the end, oxblood on a
	   failure — and one line of copy says it. */
	.sentinel {
		block-size: 1px;
	}
	.folio {
		position: relative;
		display: grid;
		justify-items: start;
		gap: var(--space-3);
		margin-block-start: var(--space-6);
		padding-block-end: var(--space-6);
	}
	/* Mounted with the query so its live region is in place before the
	   first page lands. Until a first page has been shown (and under a
	   failed first search) it is the empty live region alone and takes
	   no room; from then on its room is fixed, so a second search does
	   not move the bottom of the page. */
	.folio:not([data-settled='true']) {
		margin-block-start: 0;
		padding-block-end: 0;
	}
	/* Out of the grid's flow: a focus target, not a row. */
	.folio-anchor {
		position: absolute;
		inset-block-start: 0;
		inset-inline-start: 0;
	}
	.folio-anchor:focus {
		outline: none;
	}
	/* The rule is a fixed 5px band: a line at its head whose weight and
	   colour carry the state, and, at the end, a second hairline at its
	   foot. Nothing about it changes size between states. */
	.folio-rule {
		--rule-weight: 1px;
		--rule-color: transparent;
		position: relative;
		display: block;
		inline-size: 100%;
		block-size: 5px;
		overflow: hidden;
	}
	.folio-rule::before,
	.folio-rule::after {
		content: '';
		position: absolute;
		inset-inline: 0;
	}
	.folio-rule::before {
		inset-block-start: 0;
		block-size: var(--rule-weight);
		background: var(--rule-color);
	}
	/* The two states that ask something of the reader — wait, or act —
	   carry a heavier rule than the settled ones. */
	.folio-rule-drawing {
		--rule-weight: 2px;
		--rule-color: var(--ink-200);
	}
	/* Drawn from the start of the line: the origin follows the
	   writing direction. */
	.folio-rule-drawing::after {
		inset-block-start: 0;
		block-size: 2px;
		background: var(--accent-persimmon);
		transform-origin: left center;
		animation: folio-draw 1.6s var(--ease-in-out) infinite;
	}
	.folio-rule-drawing:dir(rtl)::after {
		transform-origin: right center;
	}
	@keyframes folio-draw {
		0% {
			transform: scaleX(0);
			opacity: 1;
		}
		70% {
			transform: scaleX(1);
			opacity: 1;
		}
		100% {
			transform: scaleX(1);
			opacity: 0;
		}
	}
	/* The manga-page double rule closes the list: two hairlines,
	   3px apart. */
	.folio-rule-closed {
		--rule-color: var(--ink-300);
	}
	.folio-rule-closed::after {
		inset-block-end: 0;
		block-size: 1px;
		background: var(--ink-300);
	}
	.folio-rule-failed {
		--rule-weight: 2px;
		--rule-color: var(--accent-oxblood);
	}
	.folio-rule-held {
		--rule-color: var(--bone-400);
	}
	.folio-line {
		margin: 0;
		max-inline-size: 60ch;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		line-height: var(--leading-prose);
		color: var(--bone-300);
	}
	/* One line's room even while empty, once a first page is shown, so
	   copy arriving or leaving does not move what is below. */
	.folio[data-settled='true'] .folio-line {
		min-block-size: calc(1em * var(--leading-prose));
	}
	.folio-detail {
		color: var(--bone-400);
	}
	.folio-action {
		padding: 6px var(--space-4);
		background: transparent;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		font-family: var(--font-body);
		font-size: var(--type-meta);
		font-weight: 500;
		color: var(--bone-100);
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.folio-action[aria-disabled='true'] {
		color: var(--bone-400);
		cursor: progress;
	}
	.folio-action:hover {
		border-color: var(--bone-300);
	}
	.folio-action:focus-visible {
		outline: none;
		box-shadow: 0 0 0 2px var(--bone-100);
	}
	.grid-ghost {
		margin-block-start: var(--space-6);
		grid-template-rows: auto;
		grid-auto-rows: 0;
		row-gap: 0;
		overflow: hidden;
	}

	@media (prefers-reduced-motion: reduce) {
		.card,
		.card-skeleton .poster-skeleton,
		.line {
			animation: none;
		}
		.folio-rule-drawing::after {
			animation: none;
			opacity: 0.6;
		}
	}

	/* — Calm states. */
	.state {
		margin-block-start: var(--space-7);
		padding: var(--space-6);
		border-inline-start: 2px solid var(--ink-300);
	}
	.state-error {
		border-inline-start-color: var(--accent-oxblood);
	}
	.state-empty {
		border-inline-start-color: var(--bone-400);
	}
	.state-headline {
		margin: 0 0 var(--space-2);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-m);
		color: var(--bone-100);
		letter-spacing: -0.01em;
	}
	.state-headline em {
		font-style: normal;
		font-weight: 600;
		color: var(--bone-200);
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}

	/* Sort + filter controls — match settings' .field grouping so
	   the label and its control read as a single unit. Each row is
	   a vertical stack of (label, control) with tight 6px gap;
	   rows separated by space-4 vertical rhythm. */
	.controls {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		margin-block-end: var(--space-5);
	}
	.control-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.control-label {
		font-family: var(--font-body);
		font-size: var(--type-body);
		font-weight: 500;
		color: var(--bone-100);
	}
	/* Both Sort + Type use the same outlined-chip vocabulary. Sort
	   behaves as a radiogroup (single-select); Type as a multi-
	   select group. Same visual result so the two rows feel like
	   one control system. */
	.chips {
		display: inline-flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.chip {
		padding: 6px var(--space-3);
		background: transparent;
		/* ink-300, the subdued-border token, so chips read clearly
		   against the page's ink-000 surface. */
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-pill);
		font-family: var(--font-body);
		font-size: var(--type-meta);
		font-weight: 500;
		color: var(--bone-200);
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.chip:hover:not(.active) {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.chip.active {
		background: var(--accent-persimmon);
		color: var(--ink-000);
		border-color: var(--accent-persimmon);
	}
	.chip-clear {
		border-style: dashed;
	}
</style>
