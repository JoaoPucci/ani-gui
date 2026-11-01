<!--
  LinkShowDialog — says which show a guessed history row really is.

  Opened from the quiet note a show page carries when a guessed
  Continue card opened it, never on arrival. Two steps in one card:
  pick a show from the candidates (or search for one), then confirm
  with what the link changes and anything worth a second look. The
  candidates, the search and the link itself are the caller's
  ($lib/history/link-offer.ts); this only renders them and reports the
  choice.

  Esc and the backdrop close it while nothing is in flight.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { imageProxyUrl } from '$lib/api';
	import { subtypeLabel } from '$lib/search/subtype-label';
	import type { LinkCandidate, LinkWarning } from '$lib/history/link-candidates';

	interface Props {
		open: boolean;
		/** The title the history row saved. */
		savedTitle: string;
		/** The episode the row is at, as the card shows it. */
		episode: string;
		/** The shows offered; null while they load. */
		candidates: LinkCandidate[] | null;
		/** Whether the last load or search failed. */
		failed: boolean;
		/** The show picked, once one is. */
		chosen: LinkCandidate | null;
		warnings: LinkWarning[];
		/** How many history rows the link moves — the card's and those it
		 *  hides — or null while that is being worked out. */
		rowCount: number | null;
		/** A link in flight. */
		busy: boolean;
		/** Why the last link failed, when it did. */
		error: string | null;
		onSearch: (query: string) => void;
		onChoose: (candidate: LinkCandidate | null) => void;
		onConfirm: () => void;
		onCancel: () => void;
	}
	let {
		open,
		savedTitle,
		episode,
		candidates,
		failed,
		chosen,
		warnings,
		rowCount,
		busy,
		error,
		onSearch,
		onChoose,
		onConfirm,
		onCancel
	}: Props = $props();

	let query = $state('');
	let searchInput: HTMLInputElement | undefined = $state();
	let confirmBtn: HTMLButtonElement | undefined = $state();
	let list: HTMLUListElement | undefined = $state();
	/** The show last picked, so Back lands on its row. */
	let lastPicked: string | null = null;
	/** What had focus when the dialog opened, given it back on close. */
	let opener: HTMLElement | null = null;

	$effect.pre(() => {
		if (open && !opener && document.activeElement instanceof HTMLElement) {
			opener = document.activeElement;
		}
	});
	$effect(() => {
		if (open) return;
		const back = opener;
		opener = null;
		lastPicked = null;
		if (back?.isConnected) back.focus();
	});
	$effect(() => {
		if (!open) return;
		if (chosen) {
			lastPicked = chosen.ref.id;
			confirmBtn?.focus();
			return;
		}
		const row = lastPicked
			? list?.querySelector<HTMLButtonElement>(`button[data-id="${CSS.escape(lastPicked)}"]`)
			: null;
		(row ?? searchInput)?.focus();
	});

	/** What the list says to assistive tech, from one live region. */
	const status = $derived(
		candidates === null
			? m.detail_link_candidates_loading()
			: failed
				? m.detail_link_candidates_failed()
				: candidates.length === 0
					? m.detail_link_candidates_empty()
					: ''
	);

	/** The row's facts, in order: year, format, episode count. */
	function meta(c: LinkCandidate): { text: string; num: boolean }[] {
		const fields: { text: string; num: boolean }[] = [];
		const year = c.ref.start_date?.slice(0, 4);
		if (year) fields.push({ text: year, num: true });
		fields.push({ text: subtypeLabel(c.ref.subtype), num: false });
		const count = c.ref.episode_count;
		if (count !== null) {
			fields.push({ text: `${count} ${m.search_card_eps_suffix({ count })}`, num: true });
		}
		return fields;
	}

	function close() {
		if (!busy) onCancel();
	}

	function onKey(e: KeyboardEvent) {
		if (open && e.key === 'Escape') {
			e.preventDefault();
			close();
		}
	}
</script>

<svelte:window onkeydown={onKey} />

{#if open}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="ls-backdrop"
		role="dialog"
		aria-modal="true"
		aria-labelledby="ls-title"
		tabindex="-1"
		onclick={(e) => {
			if (e.target === e.currentTarget) close();
		}}
	>
		<div class="ls-card" role="document">
			<header class="ls-head">
				<p class="ls-eyebrow" aria-hidden="true">
					<span class="ls-eyebrow-rule"></span>
					<span>{m.detail_link_dialog_eyebrow()}</span>
				</p>
				{#if chosen}
					<h2 id="ls-title" class="ls-title">
						{m.detail_link_confirm_heading({ title: chosen.ref.canonical_title })}
					</h2>
					<p class="ls-body">
						{m.detail_link_confirm_body({ title: chosen.ref.canonical_title })}
					</p>
				{:else}
					<h2 id="ls-title" class="ls-title">
						{m.detail_link_dialog_title({ title: savedTitle })}
					</h2>
					<p class="ls-body">
						{m.detail_link_dialog_body({ episode })}
					</p>
				{/if}
			</header>

			{#if chosen}
				{#if rowCount !== null && rowCount > 1}
					<p class="ls-body ls-rows">{m.detail_link_confirm_rows()}</p>
				{/if}
				{#if warnings.length > 0}
					<ul class="ls-warnings">
						{#if warnings.includes('season')}
							<li>{m.detail_link_warning_season()}</li>
						{/if}
						{#if warnings.includes('episode')}
							<li>
								{m.detail_link_warning_episode({
									episode,
									count: String(chosen.ref.episode_count ?? '')
								})}
							</li>
						{/if}
					</ul>
				{/if}
				{#if error}
					<p class="ls-error" role="alert">{error}</p>
				{/if}
				<footer class="ls-foot">
					<button
						type="button"
						class="ls-btn ls-btn-quiet"
						onclick={() => onChoose(null)}
						disabled={busy}
					>
						{m.detail_link_back_button()}
					</button>
					<button
						bind:this={confirmBtn}
						type="button"
						class="ls-btn ls-btn-accent"
						onclick={onConfirm}
						disabled={busy || rowCount === null}
						aria-busy={busy}
					>
						{busy ? m.detail_link_linking() : m.detail_link_confirm_button()}
					</button>
				</footer>
			{:else}
				<form
					class="ls-search"
					role="search"
					onsubmit={(e) => {
						e.preventDefault();
						if (query.trim()) onSearch(query);
					}}
				>
					<label class="ls-search-label" for="ls-query">{m.detail_link_search_label()}</label>
					<input
						bind:this={searchInput}
						id="ls-query"
						class="ls-input"
						type="search"
						autocomplete="off"
						placeholder={m.detail_link_search_placeholder()}
						bind:value={query}
					/>
					<button type="submit" class="ls-btn ls-btn-quiet">{m.detail_link_search_submit()}</button>
				</form>

				<p class="ls-live" role="status">{status}</p>
				{#if status}
					<p class="ls-status" aria-hidden="true">{status}</p>
				{:else if candidates}
					<ul bind:this={list} class="ls-list" aria-label={m.detail_link_candidates_aria()}>
						{#each candidates as c (c.ref.id)}
							{@const poster = imageProxyUrl(c.ref.poster_image?.small ?? c.ref.poster_image?.tiny)}
							<li>
								<button
									type="button"
									class="ls-row"
									data-id={c.ref.id}
									title={c.ref.canonical_title}
									onclick={() => onChoose(c)}
								>
									<span class="ls-thumb">
										{#if poster}
											<img src={poster} alt="" loading="lazy" decoding="async" />
										{/if}
									</span>
									<span class="ls-row-text">
										<span class="ls-row-title">{c.ref.canonical_title}</span>
										<span class="ls-row-meta">
											{#each meta(c) as field, i (i)}
												{#if i > 0}<span class="ls-dot" aria-hidden="true">·</span>{/if}
												<span class:ls-num={field.num}>{field.text}</span>
											{/each}
										</span>
									</span>
									{#if c.current}
										<span class="ls-current">{m.detail_link_shown_now()}</span>
									{/if}
								</button>
							</li>
						{/each}
					</ul>
				{/if}
				<footer class="ls-foot">
					<button type="button" class="ls-btn ls-btn-quiet" onclick={close}>
						{m.detail_link_cancel_button()}
					</button>
				</footer>
			{/if}
		</div>
	</div>
{/if}

<style>
	.ls-backdrop {
		position: fixed;
		inset: 0;
		background: color-mix(in oklab, var(--ink-000) 72%, transparent);
		display: grid;
		place-items: center;
		z-index: 100;
		animation: ls-fade var(--dur-fast) var(--ease-out-soft) both;
	}
	@keyframes ls-fade {
		from {
			opacity: 0;
		}
	}
	.ls-card {
		inline-size: min(34rem, calc(100vw - var(--space-6)));
		max-block-size: min(40rem, calc(100dvh - var(--space-6)));
		display: flex;
		flex-direction: column;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-card-hover);
		padding: var(--space-5);
		animation: ls-rise var(--dur-med) var(--ease-out-elastic) both;
	}
	@keyframes ls-rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ls-backdrop,
		.ls-card {
			animation: none;
		}
	}
	.ls-head {
		margin-block-end: var(--space-4);
	}
	.ls-eyebrow {
		margin: 0 0 var(--space-2);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--accent, var(--bone-300));
	}
	.ls-eyebrow-rule {
		display: inline-block;
		inline-size: 2rem;
		block-size: 1px;
		background: var(--accent, var(--bone-300));
	}
	.ls-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		font-weight: 600;
		font-style: italic;
		color: var(--bone-100);
		line-height: var(--leading-tight);
	}
	.ls-body {
		margin: var(--space-2) 0 0;
		font-size: var(--type-meta);
		line-height: var(--leading-prose);
		color: var(--bone-300);
	}
	.ls-search {
		display: flex;
		gap: var(--space-2);
		margin-block-end: var(--space-3);
	}
	.ls-search-label {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.ls-input {
		flex: 1;
		min-inline-size: 0;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-pill);
		border: 1px solid var(--ink-300);
		background: var(--ink-000);
		color: var(--bone-100);
		font: inherit;
		font-size: var(--type-meta);
	}
	.ls-list {
		list-style: none;
		margin: 0;
		padding: 0;
		overflow-y: auto;
		min-block-size: 0;
		flex: 1;
		border-block: 1px solid var(--ink-200);
	}
	.ls-row {
		display: grid;
		grid-template-columns: 2.5rem 1fr auto;
		align-items: center;
		gap: var(--space-3);
		inline-size: 100%;
		padding: var(--space-2) var(--space-1);
		background: transparent;
		border: 0;
		border-block-end: 1px solid color-mix(in oklab, var(--ink-200) 60%, transparent);
		color: inherit;
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.ls-list li:last-child .ls-row {
		border-block-end: 0;
	}
	.ls-row:hover,
	.ls-row:focus-visible {
		background: color-mix(in oklab, var(--accent, var(--ink-200)) 12%, transparent);
		outline: none;
	}
	.ls-thumb {
		inline-size: 2.5rem;
		aspect-ratio: 2 / 3;
		border-radius: 4px;
		overflow: hidden;
		background: var(--ink-200);
	}
	.ls-thumb img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		display: block;
	}
	.ls-row-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-inline-size: 0;
	}
	.ls-row-title {
		color: var(--bone-100);
		font-size: var(--type-meta);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.ls-dot {
		color: var(--bone-400);
	}
	.ls-row-meta {
		display: flex;
		gap: var(--space-2);
		font-size: var(--type-micro);
		color: var(--bone-400);
	}
	.ls-num {
		font-variant-numeric: tabular-nums;
	}
	.ls-current {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--accent, var(--bone-300));
	}
	.ls-live {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.ls-rows {
		margin: 0 0 var(--space-4);
	}
	.ls-status {
		margin: var(--space-3) 0;
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.ls-warnings {
		margin: 0 0 var(--space-4);
		padding-inline-start: var(--space-4);
		font-size: var(--type-meta);
		line-height: var(--leading-prose);
		color: var(--bone-200);
	}
	.ls-warnings li::marker {
		color: var(--accent-oxblood);
	}
	.ls-error {
		margin: 0 0 var(--space-4);
		font-size: var(--type-meta);
		color: var(--accent-oxblood);
	}
	.ls-foot {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-3);
		margin-block-start: var(--space-4);
	}
	.ls-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-pill);
		border: 1px solid transparent;
		font: inherit;
		font-size: var(--type-meta);
		font-weight: 500;
		line-height: 1;
		cursor: pointer;
	}
	.ls-btn:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
	.ls-btn-quiet {
		background: transparent;
		color: var(--bone-200);
		border-color: var(--ink-300);
	}
	.ls-btn-quiet:hover:not(:disabled) {
		background: var(--ink-100);
		color: var(--bone-100);
	}
	.ls-btn-accent {
		background: var(--brand);
		color: var(--brand-ink);
	}
	.ls-btn-accent:hover:not(:disabled) {
		background: color-mix(in oklab, var(--brand) 88%, white 12%);
	}
</style>
