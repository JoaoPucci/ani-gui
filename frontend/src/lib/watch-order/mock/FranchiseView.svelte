<!--
  The franchise page (mockup), in the show page's frame: a hero band
  and masthead, then the main story, other versions, films, side
  stories and recaps. The main story is a numbered list (`list`) or a
  shelf of RunCards (`shelf`); the rest are Strip rails of PosterCards.
  The entry the page was opened from is "You are here".
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import { mastheadArt } from './masthead';
	import MetaLine from './MetaLine.svelte';
	import MockSwitch from './MockSwitch.svelte';
	import RunCard from './RunCard.svelte';
	import { metaLine, numeral, railKeyText } from './meta';
	import { pathNote, placement, railItems, segments } from './order';
	import type { WatchOrderMockPrefs } from './prefs';
	import { shortTitle } from './short-title';
	import { franchiseSummary } from './strip-view';
	import type { MockEntry, MockRun } from './types';
	import { asRef, currentIn, franchiseName, progressOf, progressShare } from './view';

	let {
		run,
		id,
		prefs,
		today,
		onchoose
	}: {
		run: MockRun;
		id: string;
		prefs: WatchOrderMockPrefs;
		today: string;
		onchoose: (next: WatchOrderMockPrefs) => void;
	} = $props();

	const segs = $derived(segments(run, prefs.order));
	const items = $derived(railItems(run, prefs.order));
	const first = $derived(run.entries.find((e) => e.id === run.nameId)!);
	const onNow = $derived(currentIn(run, prefs.order));
	const heroEntry = $derived(mastheadArt(run, prefs.order));
	const accent = $derived(accentFor(heroEntry.id));
	const titles = $derived(run.entries.map((e) => e.title));
	const franchiseTitles = $derived(
		[...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps]
			.filter((e) => e.id !== first.id)
			.map((e) => e.title)
	);
	const label = (e: MockEntry) =>
		prefs.titles === 'short' ? shortTitle(e.title, titles) : e.title;
	const posOf = (e: MockEntry) => placement(run, e.id, prefs.order).position;
	let recapsOpen = $state(false);
</script>

<div class="franchise" style:--accent={accent}>
	<section class="hero" aria-hidden="true">
		{#if imageProxyUrl(heroEntry.cover)}
			<img class="hero-img" src={imageProxyUrl(heroEntry.cover)} alt="" />
		{/if}
		<div class="hero-scrim"></div>
	</section>

	<div class="content">
		<section class="masthead">
			<div class="poster-frame">
				{#if imageProxyUrl(heroEntry.poster)}<img
						src={imageProxyUrl(heroEntry.poster)}
						alt=""
					/>{/if}
			</div>
			<div class="masthead-text">
				<p class="eyebrow">
					<span class="eyebrow-key">{m.watchorder_franchise_eyebrow()}</span>
					<span class="eyebrow-rule" aria-hidden="true"></span>
					<span class="eyebrow-value"
						>{m.watchorder_franchise_count({ count: franchiseSummary(run).total })}</span
					>
				</p>
				<h1 class="title">{franchiseName(first.title, franchiseTitles)}</h1>
				<div class="actions">
					<a
						class="btn btn-glass"
						style:--btn-glow="var(--accent)"
						href={resolve('/anime/[id]', { id: heroEntry.id })}
					>
						<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"
							><path d="M8 5v14l11-7z" fill="currentColor" /></svg
						>
						{onNow ? m.watchorder_franchise_resume() : m.watchorder_franchise_start()}
					</a>
					{#if onNow}
						<span class="resume-caption">
							{#if posOf(onNow.entry) !== null}<span class="caption-num"
									>{numeral(posOf(onNow.entry)!)}</span
								>{/if}
							<span class="caption-title">{label(onNow.entry)}</span>
							<span class="caption-ep"
								>{m.watchorder_meta_progress_open({ episode: onNow.episode })}</span
							>
						</span>
					{/if}
				</div>
				<p class="order-note">
					{prefs.order === 'linked'
						? m.watchorder_franchise_order_note_linked()
						: m.watchorder_franchise_order_note_released()}
					<a class="text-link" href={resolve('/settings')}
						>{m.watchorder_franchise_order_change()}</a
					>
				</p>
			</div>
		</section>

		{#if prefs.switcher}
			<div class="switch-row">
				<MockSwitch {prefs} keys={['order', 'franchise', 'titles']} {onchoose} />
			</div>
		{/if}

		{#snippet row(e: MockEntry, numbered: boolean)}
			{@const pos = numbered ? posOf(e) : null}
			{@const share = progressShare(progressOf(run, e))}
			{@const note = pathNote(run, e.id, prefs.order)}
			<li class:here={e.id === id} style:--row-accent={accentFor(e.id)}>
				<a
					class="story-row"
					class:compact={!numbered}
					href={resolve('/anime/[id]', { id: e.id })}
					title={e.title}
					aria-current={e.id === id ? 'page' : undefined}
				>
					{#if numbered}
						<span class="row-num" aria-hidden="true">{pos !== null ? numeral(pos) : ''}</span>
					{/if}
					<span class="row-poster">
						{#if imageProxyUrl(e.poster)}<img
								src={imageProxyUrl(e.poster)}
								alt=""
								loading="lazy"
								decoding="async"
							/>{/if}
					</span>
					<span class="row-text">
						{#if e.id === id}<span class="here-key">{m.watchorder_franchise_here()}</span>{/if}
						<span class="row-title">{label(e)}</span>
						{#if label(e) !== e.title}<span class="sr-only">{e.title}</span>{/if}
						<MetaLine segments={metaLine(run, e, today)} />
						{#if note?.kind === 'leads-into'}
							<span class="row-note"
								>{m.watchorder_strip_leads_into({ title: label(note.entry) })}</span
							>
						{/if}
					</span>
					{#if share > 0}
						<span class="row-progress" aria-hidden="true">
							<span class="row-progress-fill" style:inline-size={`${share * 100}%`}></span>
						</span>
					{/if}
				</a>
			</li>
		{/snippet}

		<section class="main-story">
			<h2 class="eyebrow section-eyebrow">
				<span class="eyebrow-key">{m.watchorder_franchise_main()}</span>
			</h2>
			{#if prefs.franchise === 'list'}
				{#each segs as seg, i (i)}
					{#if seg.kind === 'line'}
						{#if i > 0 && segs[i - 1].kind === 'cycle'}
							<p class="group-key after-loop">{m.watchorder_franchise_after_loop()}</p>
						{/if}
						<ol class="story-list">
							{#each seg.entries as e (e.id)}{@render row(e, true)}{/each}
						</ol>
					{:else if seg.kind === 'fork'}
						<div class="fork">
							<p class="group-key">{m.watchorder_strip_ways({ count: seg.branches.length })}</p>
							<div class="branches">
								{#each seg.branches as branch, b (b)}
									<div class="branch">
										<p class="branch-key">
											{m.watchorder_strip_way({ n: b + 1, count: seg.branches.length })}
										</p>
										<ol class="story-list">
											{#each branch as e (e.id)}{@render row(e, false)}{/each}
										</ol>
									</div>
								{/each}
							</div>
						</div>
					{:else}
						<div class="loop">
							<p class="group-key">
								{m.watchorder_strip_unnumbered_cycle({ count: seg.entries.length })}
							</p>
							<p class="group-hint">{m.watchorder_franchise_cycle_hint()}</p>
							<ol class="story-list">
								{#each seg.entries as e (e.id)}{@render row(e, false)}{/each}
							</ol>
						</div>
					{/if}
				{/each}
			{:else}
				<ol class="shelf">
					{#each items as item (item.entry.id)}
						<li>
							<RunCard
								{run}
								entry={item.entry}
								position={item.position}
								label={label(item.entry)}
								current={item.entry.id === id}
								keyText={railKeyText(item.key, item.entry.id === id, label)}
								{today}
							/>
						</li>
					{/each}
				</ol>
			{/if}
		</section>

		{#if run.versions.length > 0}
			<Strip eyebrow={m.watchorder_franchise_versions()}>
				{#each run.versions as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if run.films.length > 0}
			<Strip eyebrow={m.watchorder_franchise_films()}>
				{#each run.films as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if run.sides.length > 0}
			<Strip eyebrow={m.watchorder_franchise_sides()}>
				{#each run.sides as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if run.recaps.length > 0}
			<section class="recaps">
				<div class="recaps-head">
					<h2 class="eyebrow">
						<span class="eyebrow-key">{m.watchorder_franchise_recaps()}</span>
						<span class="eyebrow-rule" aria-hidden="true"></span>
						<span class="eyebrow-value">{run.recaps.length}</span>
					</h2>
					<button
						type="button"
						class="recaps-toggle"
						aria-expanded={recapsOpen}
						onclick={() => (recapsOpen = !recapsOpen)}
						>{recapsOpen
							? m.watchorder_franchise_recaps_hide()
							: m.watchorder_franchise_recaps_show()}</button
					>
				</div>
				{#if recapsOpen}
					<Strip eyebrow={m.watchorder_franchise_recaps()} headless>
						{#each run.recaps as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
					</Strip>
				{/if}
			</section>
		{/if}
	</div>
</div>

<style>
	.franchise {
		padding-block-end: var(--space-8);
		animation: franchise-enter var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes franchise-enter {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}
	.hero {
		position: relative;
		aspect-ratio: var(--hero-aspect);
		overflow: hidden;
		background: color-mix(in oklab, var(--accent) 18%, var(--ink-050));
	}
	.hero-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
	}
	.hero-scrim {
		position: absolute;
		inset: 0;
		background: linear-gradient(
			180deg,
			color-mix(in oklab, var(--ink-000) 20%, transparent) 0%,
			transparent 30%,
			color-mix(in oklab, var(--ink-000) 70%, transparent) 75%,
			var(--ink-000) 100%
		);
	}
	.content {
		max-inline-size: 90rem;
		margin-inline: auto;
	}
	.masthead {
		position: relative;
		display: grid;
		grid-template-columns: minmax(10rem, 13rem) 1fr;
		gap: var(--space-7);
		align-items: end;
		margin-block-start: -8rem;
		padding-inline: var(--space-8);
	}
	.poster-frame {
		position: relative;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: var(--radius-card);
		background: var(--ink-100);
		box-shadow:
			0 1px 0 1px color-mix(in oklab, var(--accent) 40%, transparent),
			0 24px 48px -16px rgb(0 0 0 / 0.7);
	}
	.poster-frame img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.masthead-text {
		min-inline-size: 0;
	}
	.title {
		margin: var(--space-3) 0 var(--space-5);
		font-family: var(--font-display);
		font-style: italic;
		font-size: clamp(2rem, 4vw, var(--type-display-xl));
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		overflow-wrap: anywhere;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-4);
	}
	.btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
	}
	.resume-caption {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-3);
		min-inline-size: 0;
		font-size: var(--type-meta);
		color: color-mix(in oklab, var(--bone-100) 70%, transparent);
	}
	.caption-num {
		font-family: var(--font-mono);
		font-weight: 700;
		color: var(--bone-100);
	}
	.caption-title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 24rem;
		color: var(--bone-100);
	}
	.caption-ep {
		font-variant-numeric: tabular-nums;
	}
	.text-link {
		margin-inline-start: var(--space-2);
		font-weight: 600;
		color: var(--bone-100);
		text-decoration: underline;
		text-decoration-color: color-mix(in oklab, var(--bone-100) 30%, transparent);
		text-underline-offset: 0.25em;
	}
	.order-note {
		margin: var(--space-4) 0 0;
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.switch-row {
		padding-inline: var(--space-8);
		margin-block-start: var(--space-6);
	}
	.main-story {
		padding-inline: var(--space-8);
		margin-block: var(--space-7);
	}
	.section-eyebrow {
		margin-block-end: var(--space-4);
	}
	.story-list {
		display: grid;
		margin: 0;
		padding: 0;
		list-style: none;
		max-inline-size: 60rem;
		border-block-start: 1px solid var(--ink-200);
	}
	.fork,
	.loop {
		max-inline-size: 60rem;
		margin-block: var(--space-5);
		padding-inline-start: var(--space-5);
		border-inline-start: 1px solid var(--ink-300);
	}
	.group-key,
	.branch-key {
		margin: 0 0 var(--space-3);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
	}
	.branch-key {
		color: var(--bone-300);
	}
	.after-loop {
		margin-block-start: var(--space-5);
	}
	.group-hint {
		margin: calc(-1 * var(--space-2)) 0 var(--space-3);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.branches {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
		gap: var(--space-5);
		align-items: start;
	}
	.story-list li {
		border-block-end: 1px solid var(--ink-200);
	}
	.story-row {
		position: relative;
		display: grid;
		grid-template-columns: 4.5rem 3.5rem minmax(0, 1fr) 7rem;
		gap: var(--space-4);
		align-items: center;
		padding-block: var(--space-3);
		padding-inline: var(--space-2) var(--space-4);
		border-inline-start: 2px solid transparent;
		color: inherit;
		transition: background-color var(--dur-fast) var(--ease-out-soft);
	}
	.story-row:hover {
		background: var(--ink-050);
	}
	.story-row.compact {
		grid-template-columns: 3rem minmax(0, 1fr) 5rem;
	}
	.here .story-row {
		background: color-mix(in oklab, var(--row-accent) 8%, var(--ink-050));
		border-inline-start-color: var(--row-accent);
	}
	.row-num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-numeral-xl);
		font-weight: 700;
		line-height: 1;
		text-align: end;
		color: color-mix(in oklab, var(--bone-100) 85%, transparent);
	}
	.here .row-num {
		color: var(--bone-100);
	}
	.row-poster {
		position: relative;
		aspect-ratio: var(--poster-aspect);
		overflow: hidden;
		border-radius: calc(var(--radius-card) - 2px);
		background: var(--ink-100);
	}
	.row-poster img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.row-text {
		display: grid;
		gap: var(--space-1);
		min-inline-size: 0;
	}
	.here-key {
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
	}
	.row-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-size: var(--type-body);
		font-weight: 500;
		line-height: 1.3;
	}
	.row-note {
		font-size: var(--type-micro);
		color: var(--bone-300);
	}
	.row-progress {
		grid-column: -2;
		block-size: 3px;
		border-radius: var(--radius-pill);
		background: var(--ink-200);
		overflow: hidden;
	}
	.row-progress-fill {
		display: block;
		block-size: 100%;
		background: var(--row-accent);
	}
	.shelf {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
		gap: var(--space-6) var(--space-5);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.recaps {
		margin-block-start: var(--space-7);
	}
	.recaps-head {
		display: flex;
		align-items: baseline;
		gap: var(--space-5);
		padding-inline: var(--space-8);
		margin-block-end: var(--space-4);
	}
	.recaps-toggle {
		font-size: var(--type-meta);
		font-weight: 600;
		text-decoration: underline;
		text-decoration-color: color-mix(in oklab, var(--bone-100) 30%, transparent);
		text-underline-offset: 0.25em;
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	@media (max-width: 1100px) {
		.masthead {
			grid-template-columns: 9rem 1fr;
			gap: var(--space-5);
		}
	}
</style>
