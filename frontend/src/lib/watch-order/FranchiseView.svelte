<!--
  The franchise page, in the show page's frame: a hero band and
  masthead named after the franchise's first title, then the main
  story as a numbered list, other versions, films, side stories and
  recaps. Numbered stretches carry the oversized numeral; ways to
  continue stand side by side and titles with no clear order form one
  group, both unnumbered. The title the page was opened from is "You
  are here". Every reading comes from the page model
  (`./franchise-page`); this component only draws it.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import MetaLine from './MetaLine.svelte';
	import type { FranchisePageModel, FranchiseRow } from './franchise-page';
	import { loopGroupText, metaLine, numeral } from './meta';
	import { asRef } from './poster-ref';

	let { model, today }: { model: FranchisePageModel; today: string } = $props();

	const accent = $derived(accentFor(model.art.id));
	let recapsOpen = $state(false);
</script>

<div class="franchise" style:--accent={accent}>
	<section class="hero" aria-hidden="true">
		{#if imageProxyUrl(model.art.cover)}
			<img class="hero-img" src={imageProxyUrl(model.art.cover)} alt="" />
		{/if}
		<div class="hero-scrim"></div>
	</section>

	<div class="content">
		<section class="masthead">
			<div class="poster-frame">
				{#if imageProxyUrl(model.art.poster)}<img
						src={imageProxyUrl(model.art.poster)}
						alt=""
					/>{/if}
			</div>
			<div class="masthead-text">
				<p class="eyebrow">
					<span class="eyebrow-key">{m.watchorder_franchise_eyebrow()}</span>
					<span class="eyebrow-rule" aria-hidden="true"></span>
					<span class="eyebrow-value">{m.watchorder_franchise_count({ count: model.total })}</span>
				</p>
				<h1 class="title">{model.name}</h1>
				<div class="actions">
					<a
						class="btn btn-glass"
						style:--btn-glow="var(--accent)"
						href={resolve('/anime/[id]', { id: (model.resume?.entry ?? model.start).id })}
					>
						<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"
							><path d="M8 5v14l11-7z" fill="currentColor" /></svg
						>
						{model.resume ? m.watchorder_franchise_resume() : m.watchorder_franchise_start()}
					</a>
					{#if model.resume}
						<span class="resume-caption">
							{#if model.resume.position !== null}<span class="caption-num"
									>{numeral(model.resume.position)}</span
								>{/if}
							<span class="caption-title" title={model.resume.entry.title}
								>{model.resume.label}</span
							>
							<span class="caption-ep"
								>{m.watchorder_meta_progress_open({ episode: model.resume.episode })}</span
							>
						</span>
					{/if}
				</div>
				<p class="order-note">
					{model.order === 'linked'
						? m.watchorder_franchise_order_note_linked()
						: m.watchorder_franchise_order_note_released()}
					<a class="text-link" href={resolve('/settings')}
						>{m.watchorder_franchise_order_change()}</a
					>
				</p>
			</div>
		</section>

		{#snippet row(r: FranchiseRow, numbered: boolean)}
			<li class:here={r.here} style:--row-accent={accentFor(r.entry.id)}>
				<a
					class="story-row"
					class:compact={!numbered}
					href={resolve('/anime/[id]', { id: r.entry.id })}
					title={r.entry.title}
					aria-current={r.here ? 'page' : undefined}
				>
					{#if numbered}
						<span class="row-num" aria-hidden="true"
							>{r.position !== null ? numeral(r.position) : ''}</span
						>
					{/if}
					<span class="row-poster">
						{#if imageProxyUrl(r.entry.poster)}<img
								src={imageProxyUrl(r.entry.poster)}
								alt=""
								loading="lazy"
								decoding="async"
							/>{/if}
					</span>
					<span class="row-text">
						{#if r.here}<span class="here-key">{m.watchorder_franchise_here()}</span>{/if}
						{#if r.label !== r.entry.title}
							<span class="sr-only">{r.entry.title}</span>
							<span class="row-title" aria-hidden="true">{r.label}</span>
						{:else}
							<span class="row-title">{r.entry.title}</span>
						{/if}
						<MetaLine segments={metaLine(r.entry, r.progress, today)} />
						{#if r.leadsInto !== null}
							<span class="row-note">{m.watchorder_strip_leads_into({ title: r.leadsInto })}</span>
						{/if}
					</span>
					{#if r.share > 0}
						<span class="row-progress" aria-hidden="true">
							<span class="row-progress-fill" style:inline-size={`${r.share * 100}%`}></span>
						</span>
					{/if}
				</a>
			</li>
		{/snippet}

		<section class="main-story">
			<h2 class="eyebrow section-eyebrow">
				<span class="eyebrow-key">{m.watchorder_franchise_main()}</span>
			</h2>
			{#each model.sections as section, i (i)}
				{#if section.kind === 'line'}
					{#if section.unplaced}
						<p class="group-key after-loop">{m.watchorder_franchise_after_loop()}</p>
					{/if}
					<ol class="story-list">
						{#each section.rows as r (r.entry.id)}{@render row(r, true)}{/each}
					</ol>
				{:else if section.kind === 'ways'}
					<div class="fork">
						<p class="group-key">{m.watchorder_strip_ways({ count: section.ways.length })}</p>
						<div class="branches">
							{#each section.ways as way, w (w)}
								<div class="branch">
									<p class="branch-key">
										{m.watchorder_strip_way({ n: w + 1, count: section.ways.length })}
									</p>
									<ol class="story-list">
										{#each way as r (r.entry.id)}{@render row(r, false)}{/each}
									</ol>
								</div>
							{/each}
						</div>
					</div>
				{:else}
					<div class="loop">
						<p class="group-key">{loopGroupText(section.rows.length)}</p>
						<p class="group-hint">{m.watchorder_franchise_cycle_hint()}</p>
						<ol class="story-list">
							{#each section.rows as r (r.entry.id)}{@render row(r, false)}{/each}
						</ol>
					</div>
				{/if}
			{/each}
			{#if model.partial}
				<p class="partial">{m.watchorder_franchise_partial()}</p>
			{/if}
		</section>

		{#if model.versions.length > 0}
			<Strip eyebrow={m.watchorder_franchise_versions()}>
				{#each model.versions as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if model.films.length > 0}
			<Strip eyebrow={m.watchorder_franchise_films()}>
				{#each model.films as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if model.sides.length > 0}
			<Strip eyebrow={m.watchorder_franchise_sides()}>
				{#each model.sides as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
			</Strip>
		{/if}
		{#if model.recaps.length > 0}
			<section class="recaps">
				<div class="recaps-head">
					<h2 class="eyebrow">
						<span class="eyebrow-key">{m.watchorder_franchise_recaps()}</span>
						<span class="eyebrow-rule" aria-hidden="true"></span>
						<span class="eyebrow-value">{model.recaps.length}</span>
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
						{#each model.recaps as e (e.id)}<PosterCard anime={asRef(e)} />{/each}
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
		text-wrap: balance;
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
	.partial {
		margin: var(--space-4) 0 0;
		max-inline-size: 60rem;
		font-size: var(--type-meta);
		color: var(--bone-300);
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
