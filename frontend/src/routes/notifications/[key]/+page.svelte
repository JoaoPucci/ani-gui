<!--
  A notice's own page: its title, when, and everything it has to say,
  with its one action. Reached from a row or a toast's View, except on
  the player, where notices open in place instead. With the
  notification center off, or a notice no longer in the list, it says
  so.
-->
<script lang="ts">
	import { page } from '$app/state';
	import { m } from '$lib/paraglide/messages';
	import { updateStore } from '$lib/update/store.svelte';
	import NoticeBody from '$lib/notices/NoticeBody.svelte';
	import NoticeGlyph from '$lib/notices/NoticeGlyph.svelte';
	import { centerEnabled } from '$lib/notices/enabled';
	import { noticeCenter } from '$lib/notices/instance';
	import { copyOf, when } from '$lib/notices/copy';
	import { toneOf } from '$lib/notices/model';
	import { act } from '$lib/notices/panel';

	const enabled = centerEnabled();
	const key = $derived(page.params.key ?? '');
	const notice = $derived(enabled ? noticeCenter.rows.find((n) => n.key === key) : undefined);
	const copy = $derived(notice ? copyOf(notice) : null);

	$effect(() => {
		if (notice && !notice.read) noticeCenter.markRead(notice.key);
	});

	function onAct() {
		if (!notice || !copy?.action) return;
		act(copy.action.kind, notice, {
			openUpdate: () => updateStore.openDialog(),
			reveal: (dir) => void window.aniGui?.revealInFolder?.(dir),
			openLink: (url) => void window.aniGui?.openExternal?.(url)
		});
	}
</script>

<article class="nd">
	{#if notice && copy}
		<header class="nd-head tone-{toneOf(notice)}">
			<span class="nd-glyph"
				><NoticeGlyph kind={notice.kind} settled={toneOf(notice) === 'good'} size={22} /></span
			>
			<div class="nd-titles">
				<h1 class="nd-title">{copy.title}</h1>
				<p class="nd-meta">
					<span>{copy.meta}</span>
					<time datetime={new Date(notice.at).toISOString()}>{when(notice.at)}</time>
				</p>
			</div>
		</header>
		<NoticeBody {copy} {onAct} />
	{:else if enabled}
		<p class="nd-missing">{m.notices_detail_missing()}</p>
	{:else}
		<p class="nd-missing">{m.notices_detail_off()}</p>
	{/if}
</article>

<style>
	.nd {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		max-inline-size: 48rem;
		padding-block: var(--space-7);
	}
	.nd-head {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		column-gap: var(--space-4);
		align-items: start;
		padding-block-end: var(--space-5);
		border-block-end: 1px solid var(--ink-200);
	}
	.nd-glyph {
		display: inline-flex;
		padding-block-start: 0.375rem;
		color: var(--bone-300);
	}
	.tone-failure .nd-glyph {
		color: var(--accent-oxblood);
	}
	.tone-live .nd-glyph {
		color: var(--accent-ochre);
	}
	.tone-good .nd-glyph {
		color: var(--accent-jade);
	}
	.nd-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-display-m);
		font-weight: 500;
		letter-spacing: var(--tracking-display);
		line-height: var(--leading-tight);
		color: var(--bone-100);
		overflow-wrap: anywhere;
	}
	.nd-meta {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		margin: var(--space-2) 0 0;
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.nd-meta time {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
		color: var(--bone-400);
	}
	.nd-missing {
		margin: 0;
		font-size: var(--type-body);
		color: var(--bone-300);
	}
</style>
