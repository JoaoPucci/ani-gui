<!--
  Continue Watching's hand-off card (mockup): a finished entry's card
  moved on to the next entry. It keeps the Continue card's 16:9 shape
  and anatomy so the rail stays even; its art is the next entry's
  banner, its tag reads "NEXT" (or "EP 1"). The small line says why
  the card changed ("After …"); the large line carries the next
  entry's whole title, two lines deep, because the words that tell a
  franchise's entries apart come last and a one-line clamp cuts them. A click opens the next entry's show page.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { imageProxyUrl } from '$lib/api';
	import { accentFor } from '$lib/design/accent';
	import { m } from '$lib/paraglide/messages';
	import { shortTitle } from '../short-title';
	import type { Handoff } from './view';

	let { handoff, variant }: { handoff: Handoff; variant: 'next' | 'episode' } = $props();

	// The finished entry by the words that set it apart: the one-line
	// mono row would otherwise cut exactly those.
	const finished = $derived(
		shortTitle(
			handoff.finished.title,
			handoff.run.entries.map((e) => e.title)
		)
	);
	const image = $derived(imageProxyUrl(handoff.next.cover ?? handoff.next.poster));
</script>

<a
	class="handoff-card"
	style:--accent={accentFor(handoff.next.id)}
	href={resolve('/anime/[id]', { id: handoff.next.id })}
	aria-label={m.watchorder_handoff_aria({
		title: handoff.next.title,
		finished: handoff.finished.title
	})}
>
	<span class="frame">
		{#if image}<img src={image} alt="" loading="lazy" decoding="async" />{/if}
		<span class="tag" aria-hidden="true">
			{#if variant === 'episode'}
				<span class="tag-key">{m.home_resume_ep_key()}</span>
				<span class="tag-num">1</span>
			{:else}
				<span class="tag-word">{m.watchorder_handoff_next()}</span>
			{/if}
		</span>
	</span>
	<span class="body">
		<span class="show" title={handoff.finished.title}
			>{m.watchorder_handoff_after({ title: finished })}</span
		>
		<span class="line">{handoff.next.title}</span>
	</span>
</a>

<style>
	/* Home's .resume-card recipe, restated: the same frame, scrim, tag
	   and two text lines, so a hand-off sits in the rail as one of its
	   cards. */
	.handoff-card {
		display: grid;
		grid-template-rows: auto auto;
		inline-size: 100%;
		color: inherit;
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	.handoff-card:hover {
		transform: translateY(-3px);
	}
	.frame {
		position: relative;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		border-radius: var(--radius-card);
		background: var(--ink-100);
		transition: box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	.handoff-card:hover .frame {
		box-shadow:
			0 12px 28px -8px color-mix(in oklab, var(--accent) 38%, transparent),
			0 0 0 1px color-mix(in oklab, var(--accent) 60%, transparent);
	}
	.frame::after {
		content: '';
		position: absolute;
		z-index: 1;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: 55%;
		background: linear-gradient(
			180deg,
			transparent,
			color-mix(in oklab, var(--ink-000) 75%, transparent)
		);
		opacity: 0.85;
		pointer-events: none;
	}
	.frame img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
		filter: brightness(0.85);
		transition: filter var(--dur-med) var(--ease-out-soft);
	}
	.handoff-card:hover .frame img {
		filter: brightness(1);
	}
	.tag {
		position: absolute;
		z-index: 2;
		inset-block-end: var(--space-2);
		inset-inline-start: var(--space-3);
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		line-height: 1;
		filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.65));
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	.handoff-card:hover .tag {
		transform: translateY(-3px);
	}
	.tag-key {
		margin-block-end: 1px;
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		color: color-mix(in oklab, var(--bone-100) 75%, transparent);
	}
	.tag-num,
	.tag-word {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: 1.75rem;
		font-weight: 700;
		color: var(--bone-100);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.tag-word {
		text-transform: uppercase;
	}
	.handoff-card:hover .tag-num,
	.handoff-card:hover .tag-word {
		color: var(--accent);
	}
	.body {
		display: grid;
		gap: var(--space-2);
		padding-block-start: var(--space-3);
	}
	.show {
		display: -webkit-box;
		-webkit-line-clamp: 1;
		line-clamp: 1;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.line {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		min-block-size: 2lh;
		font-weight: 500;
		font-size: 1rem;
		line-height: 1.3;
		color: var(--bone-100);
	}
</style>
