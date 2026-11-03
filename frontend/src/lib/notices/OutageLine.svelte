<!--
  One line above Play on a show's page while the streaming source is
  not answering, linking to the notice. It does not stop anyone from
  trying; it says, before the play fails, what the app already knows.
  Drawn only while the notification center runs.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { centerEnabled } from './enabled';
	import { noticeCenter } from './instance';
	import { detailPath, when } from './copy';
	import { toneOf } from './model';

	const enabled = centerEnabled();
	const live = $derived(
		enabled ? noticeCenter.rows.find((n) => n.kind === 'outage' && toneOf(n) === 'live') : undefined
	);
	const since = $derived(
		live ? when(typeof live.data.since === 'number' ? live.data.since : live.firstAt) : ''
	);
</script>

{#if live}
	<p class="outage-line" role="status">
		<span class="ol-dot" aria-hidden="true"></span>
		<span class="ol-text">{m.notices_outage_line({ time: since })}</span>
		<!-- eslint-disable svelte/no-navigation-without-resolve -- detailPath builds the route's own path -->
		<a class="ol-link" href={detailPath(live.key)}>{m.notices_outage_line_details()}</a>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	</p>
{/if}

<style>
	.outage-line {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2) var(--space-3);
		margin: 0 0 var(--space-3);
		max-inline-size: 60ch;
		font-size: var(--type-meta);
		line-height: 1.45;
		color: var(--bone-200);
	}
	.ol-dot {
		flex: none;
		inline-size: 0.5rem;
		block-size: 0.5rem;
		border-radius: 50%;
		background: var(--accent-ochre);
		transform: translateY(-0.05em);
	}
	.ol-text {
		flex: 1 1 18rem;
		min-inline-size: 0;
	}
	.ol-link {
		color: var(--bone-100);
		font-weight: 500;
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.ol-link:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 2px;
	}
</style>
