<!--
  DownloadBar — full-width status strip across the bottom edge of
  the viewport while one or more downloads are in flight. Modelled
  on Android Studio's status-bar pattern: indeterminate progress +
  adaptive label + Cancel-all X, all clustered on the right side
  of the strip; the rest of the strip is empty (frosted black).

  The bar sits below the rail in z-index so the rail visually
  covers the leftmost portion of the strip background. Content is
  right-aligned, so the controls never land under the rail.

  Label is driven by formatDownloadBarLabel — 1 item gets the
  episode (with range progress when known); 2+ items get a count.
  Long anime titles are truncated with an ellipsis.

  Cancel-all uses the same `window.confirm` pattern as the per-item
  cancel in DownloadDock for consistency. Bypasses if the user
  hits Escape — no item is touched.
-->
<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { downloadStore } from '$lib/download/store.svelte';
	import { formatDownloadBarLabel } from '$lib/download/bar-label';
	import { m } from '$lib/paraglide/messages';

	const active = $derived(downloadStore.active);
	const visible = $derived(active.length > 0);
	const label = $derived(formatDownloadBarLabel(active));

	function cancelAll() {
		const count = active.length;
		if (count === 0) return;
		// Match DownloadDock's per-item confirm pattern — single prompt,
		// adaptive wording. Phrased in line with the cancel-of-N idiom
		// the dock uses (intentionally not paraglided to mirror the
		// existing literal in DownloadDock; revisit when that one
		// migrates).
		const msg =
			count === 1
				? `Cancel download of "${active[0].title}"?`
				: `Cancel ${count} active downloads?`;
		const ok = typeof window !== 'undefined' ? window.confirm(msg) : true;
		if (!ok) return;
		// Snapshot ids first so the array can mutate during the loop
		// without skipping the second half (downloadStore.cancel removes
		// items synchronously from `items`, which would shift indices).
		const ids = active.map((i) => i.id);
		for (const id of ids) downloadStore.cancel(id);
	}
</script>

{#if visible}
	<aside
		class="dl-bar"
		aria-label={m.download_bar_aria_label()}
		transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
	>
		<span class="dl-bar-progress" aria-hidden="true">
			<span></span>
		</span>
		<span class="dl-bar-label">
			{#if label?.kind === 'single'}
				<span class="dl-bar-title">{label.title}</span>
				<span class="dl-bar-sep" aria-hidden="true">·</span>
				<span class="dl-bar-suffix">
					{m.download_bar_ep_label_single({ episode: label.episode })}
				</span>
			{:else if label?.kind === 'single-range-progress'}
				<span class="dl-bar-title">{label.title}</span>
				<span class="dl-bar-sep" aria-hidden="true">·</span>
				<span class="dl-bar-suffix">
					{m.download_bar_ep_label_range_progress({
						current: label.currentEp,
						total: label.total
					})}
				</span>
			{:else if label?.kind === 'single-range-static'}
				<span class="dl-bar-title">{label.title}</span>
				<span class="dl-bar-sep" aria-hidden="true">·</span>
				<span class="dl-bar-suffix">
					{m.download_bar_ep_label_range_static({ episode: label.episode })}
				</span>
			{:else if label?.kind === 'multi'}
				<span class="dl-bar-suffix">
					{m.download_bar_label_multi({ count: label.episodeCount })}
				</span>
			{/if}
		</span>
		<button
			type="button"
			class="dl-bar-cancel"
			onclick={cancelAll}
			aria-label={m.download_bar_cancel_all_aria_label()}
			title={m.download_bar_cancel_all_title()}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true">
				<path
					d="M6 6l12 12M18 6 6 18"
					stroke="currentColor"
					stroke-width="2"
					fill="none"
					stroke-linecap="round"
				/>
			</svg>
		</button>
	</aside>
{/if}

<style>
	.dl-bar {
		/* Full-viewport-wide strip across the bottom edge. Rail sits
		   at z-index 20; we sit below at 10 so the rail visually
		   covers our leftmost ~rail-width. Content is right-aligned
		   via justify-content so the controls live in the
		   rail-clear zone regardless of viewport width. */
		position: fixed;
		inset-inline: 0;
		inset-block-end: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-3);
		padding-block: var(--space-2);
		padding-inline: var(--space-4);
		min-block-size: 2rem;
		/* Black, slightly transparent. Lets a hint of the page below
		   bleed through so the bar reads as overlay, not opaque slab. */
		background: color-mix(in oklab, #000 80%, transparent);
		backdrop-filter: blur(8px);
		-webkit-backdrop-filter: blur(8px);
		color: var(--bone-200);
		border-block-start: 1px solid color-mix(in oklab, var(--bone-100) 8%, transparent);
		font-family: var(--font-body);
		font-size: var(--type-meta);
	}
	.dl-bar-progress {
		position: relative;
		flex-shrink: 0;
		inline-size: 8rem;
		block-size: 3px;
		background: color-mix(in oklab, var(--bone-100) 14%, transparent);
		border-radius: 999px;
		overflow: hidden;
	}
	.dl-bar-progress span {
		position: absolute;
		inset-block: 0;
		inline-size: 30%;
		/* Use --brand (saffron-orange) rather than --accent — the
		   accent token cycles per-show, so the bar would flicker
		   between blue/jade/etc. as the user navigates. --brand is
		   the stable identity colour for downloads. */
		background: var(--brand, currentColor);
		animation: dl-indet 1.4s var(--ease-in-out) infinite;
	}
	@keyframes dl-indet {
		0% {
			inset-inline-start: -30%;
		}
		100% {
			inset-inline-start: 100%;
		}
	}
	.dl-bar-label {
		/* The label is composed: title + separator + episode marker.
		   Only the title truncates — the "Ep N" / "Ep n of M" suffix
		   should always be readable, so we put a max-width on the
		   title alone and keep the separator + suffix non-shrinking. */
		display: inline-flex;
		align-items: baseline;
		gap: 0.35rem;
		min-inline-size: 0;
		color: var(--bone-100);
	}
	.dl-bar-title {
		max-inline-size: 16rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dl-bar-sep {
		flex-shrink: 0;
		color: var(--bone-400);
	}
	.dl-bar-suffix {
		flex-shrink: 0;
		white-space: nowrap;
		color: var(--brand);
	}
	.dl-bar-cancel {
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 1.5rem;
		block-size: 1.5rem;
		padding: 0;
		border: 0;
		border-radius: var(--radius-2, 0.4rem);
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		transition:
			background-color 120ms ease,
			color 120ms ease;
	}
	.dl-bar-cancel:hover {
		background: color-mix(in oklab, var(--bone-100) 12%, transparent);
		color: var(--bone-100);
	}
	.dl-bar-cancel:active {
		transform: scale(0.94);
	}
	.dl-bar-cancel:focus-visible {
		outline: 2px solid var(--brand, currentColor);
		outline-offset: 2px;
	}
	.dl-bar-cancel svg {
		inline-size: 0.95rem;
		block-size: 0.95rem;
	}
</style>
