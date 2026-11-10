<!--
  The room the show page holds for the watch-order strip, or for its
  franchise link alone, while the franchise walk runs
  (`./arrival`). A quiet outline of the strip's own layout, at its
  measured size: the real "Watch order" label, the two posters and
  their lines, the place's numeral, the footer rule and the link's
  stack; for the link alone, the stack and its line. Nothing in it
  moves: no shimmer, no pulse. It is decoration, hidden from screen
  readers by the block that holds it.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { Shape } from './arrival';

	let { shape }: { shape: Shape } = $props();
</script>

{#if shape === 'strip'}
	<div class="reserve">
		<div class="band">
			<p class="eyebrow">{m.watchorder_strip_eyebrow()}</p>
			<span class="sk rule l"></span><span class="sk rule r"></span>
			<span class="sk key kl"></span><span class="sk key kc"></span><span class="sk key kr"></span>
			<span class="sk poster pl"></span><span class="sk line tl1"></span><span class="sk line tl2"
			></span>
			<span class="sk num"></span>
			<span class="sk poster pr"></span><span class="sk line tr1"></span><span class="sk line tr2"
			></span>
			<span class="sk foot"></span>
			<span class="sk stack"></span><span class="sk line sl"></span>
		</div>
	</div>
{:else}
	<div class="reserve link">
		<div class="lone">
			<span class="sk stack"></span><span class="sk line sl"></span>
		</div>
	</div>
{/if}

<style>
	/* Positions are the strip's own, measured from the drawn strip of a
	   run that is one line, so the strip lands on the outline. */
	.reserve {
		padding-inline: var(--space-8);
	}
	.band {
		position: relative;
		margin-inline: calc(-1 * var(--space-5));
		block-size: 302px;
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-050) 72%, transparent);
	}
	.eyebrow {
		position: absolute;
		inset-inline-start: var(--space-5);
		inset-block-start: 27px;
		margin: 0;
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.sk {
		position: absolute;
		border-radius: 3px;
		border: 1px solid color-mix(in oklab, var(--bone-100) 9%, transparent);
	}
	.sk.line,
	.sk.key {
		border: 0;
		background: color-mix(in oklab, var(--bone-100) 7%, transparent);
	}
	.sk.rule {
		inset-block: 63px 104px;
		inline-size: 0;
		border-width: 0;
		border-inline-start-width: 1px;
		border-radius: 0;
		border-color: var(--ink-200);
	}
	.sk.rule.l {
		inset-inline-start: calc(50% - 76px);
	}
	.sk.rule.r {
		inset-inline-start: calc(50% + 77px);
	}
	.sk.key {
		inset-block-start: 66px;
		block-size: 9px;
		inline-size: 4.5rem;
	}
	.sk.key.kl {
		inset-inline-end: calc(50% + 108px);
	}
	.sk.key.kc {
		inset-inline-start: calc(50% - 40px);
		inline-size: 80px;
	}
	.sk.key.kr {
		inset-inline-start: calc(50% + 108px);
	}
	.sk.poster {
		inset-block-start: 96px;
		inline-size: 56px;
		block-size: 78px;
	}
	.sk.poster.pl {
		inset-inline-end: calc(50% + 108px);
	}
	.sk.poster.pr {
		inset-inline-start: calc(50% + 108px);
	}
	.sk.line {
		block-size: 9px;
	}
	.sk.tl1,
	.sk.tl2 {
		inset-inline-end: calc(50% + 176px);
	}
	.sk.tr1,
	.sk.tr2 {
		inset-inline-start: calc(50% + 176px);
	}
	.sk.tl1,
	.sk.tr1 {
		inset-block-start: 112px;
		inline-size: 8rem;
	}
	.sk.tl2,
	.sk.tr2 {
		inset-block-start: 132px;
		inline-size: 5rem;
	}
	.sk.num {
		inset-block-start: 96px;
		inset-inline-start: calc(50% - 26px);
		inline-size: 53px;
		block-size: 54px;
	}
	.sk.foot {
		inset-block-start: 198px;
		inset-inline: var(--space-5);
		block-size: 0;
		border-width: 0;
		border-block-start-width: 1px;
		border-radius: 0;
		border-color: var(--ink-200);
	}
	.band .sk.stack {
		inset-block-start: 230px;
		inset-inline-start: calc(50% - 273px);
		inline-size: 56px;
		block-size: 40px;
	}
	.band .sk.sl {
		inset-block-start: 246px;
		inset-inline-start: calc(50% - 197px);
		inline-size: 11rem;
	}
	.lone {
		position: relative;
		block-size: 70px;
		margin-inline-start: calc(-1 * var(--space-3));
	}
	.lone .sk.stack {
		inset-block-start: 15px;
		inset-inline-start: 28px;
		inline-size: 56px;
		block-size: 40px;
	}
	.lone .sk.sl {
		inset-block-start: 31px;
		inset-inline-start: 116px;
		inline-size: 12rem;
	}
</style>
