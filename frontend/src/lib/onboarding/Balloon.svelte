<!--
  The balloon that explains one new feature beside the place it points
  at: the eyebrow (New, rule, the release that brought it), a serif
  title, the body and its actions.

  - A dialog named by its title and described by its body. The host
    makes it the topmost modal once it is placed ($lib/components/
    modal-layer), which moves focus to the control marked
    `data-autofocus` (the primary action) and keeps Tab inside it.
    Escape is the modal layer's, which hands it to the host.
  - It stays hidden until it is measured and placed, so it never takes
    focus while it cannot be seen.
  - Focus rings are the app's own (`--ring`), never the show's colour;
    the accent marks only the eyebrow's rule and the current step.
  - A sequence shows where it is twice: the localized readout ("Step 2
    of 3") and a still film strip, the steps passed filled, the current
    one drawn twice as wide. Width carries the state, so a contrast
    theme keeps it and it needs no motion. On each step the primary
    action takes focus again and carries the step's words, so Enter
    walks the sequence. The button that keeps focus says nothing new by
    itself, so each step after the first is announced through a polite
    live region in the balloon.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import type { BalloonAction } from './balloon-actions';
	import type { Placement } from './place';

	let {
		root = $bindable(),
		title,
		body,
		version,
		newLabel,
		placement,
		accent,
		compact,
		actions,
		step,
		stepLabel,
		onAction,
		measure
	}: {
		/** The balloon's element, for the host to hold as a modal. */
		root?: HTMLDivElement;
		title: string;
		body: string;
		/** The release that brought the feature, e.g. "0.19.0"; empty for
		 *  a walkthrough asked for, which names none. */
		version: string;
		/** The eyebrow's key, e.g. "New". */
		newLabel: string;
		placement: Placement | null;
		accent: string;
		/** A short window: a tighter balloon. */
		compact: boolean;
		actions: BalloonAction[];
		/** Where a sequence is (from 1); null for one step. */
		step: { index: number; total: number } | null;
		/** The localized readout of `step`. */
		stepLabel: string;
		onAction: (id: BalloonAction['id']) => void;
		measure: (size: { w: number; h: number }) => void;
	} = $props();

	const uid = Math.random().toString(36).slice(2, 8);
	const quiet = $derived(actions.filter((a) => a.kind === 'quiet'));
	const rest = $derived(actions.filter((a) => a.kind !== 'quiet'));

	// Each step after the first: announced, since focus may not move.
	let announced = $state('');
	let moved = false;
	$effect(() => {
		const at = step?.index ?? 1;
		const said = `${stepLabel}. ${title}. ${body}`;
		if (at === 1 && !moved) return;
		moved = true;
		announced = said;
	});

	// Each new step: its primary action takes focus again (the modal
	// layer focused it on the first).
	let lastStep = 1;
	$effect(() => {
		const at = step?.index ?? 1;
		if (at === lastStep || !root) return;
		lastStep = at;
		root.querySelector<HTMLElement>('button[data-kind="primary"]')?.focus({ preventScroll: true });
	});

	onMount(() => {
		if (!root) return;
		const el = root;
		const ro = new ResizeObserver(() => measure({ w: el.offsetWidth, h: el.offsetHeight }));
		ro.observe(el);
		return () => ro.disconnect();
	});
</script>

<div
	bind:this={root}
	class="balloon"
	class:sheet={placement?.mode === 'sheet'}
	class:compact
	data-side={placement?.side ?? 'below'}
	data-testid="onboarding-balloon"
	role="dialog"
	aria-modal="true"
	aria-labelledby="ob-title-{uid}"
	aria-describedby="ob-body-{uid}"
	tabindex="-1"
	style:--ob-accent={accent}
	style:inset-inline-start="{placement?.x ?? -9999}px"
	style:inset-block-start="{placement?.y ?? -9999}px"
	style:inline-size={placement ? `${placement.width}px` : undefined}
	style:--arrow="{placement?.arrow ?? 0}px"
	style:visibility={placement ? 'visible' : 'hidden'}
>
	<span class="pointer" aria-hidden="true"></span>
	<div class="head">
		<p class="tag">
			<span>{newLabel}</span>
			{#if version}
				<span class="tag-rule" aria-hidden="true"></span>
				<span class="tag-version">{version}</span>
			{/if}
		</p>
		{#if step}
			<p class="step">
				<span class="frames" aria-hidden="true">
					{#each Array.from({ length: step.total }, (_, i) => i) as i (i)}
						<span class="frame" class:seen={i < step.index - 1} class:current={i === step.index - 1}
						></span>
					{/each}
				</span>
				<span class="step-label">{stepLabel}</span>
			</p>
		{/if}
	</div>
	<h2 class="title" id="ob-title-{uid}">{title}</h2>
	<p class="sr-only" aria-live="polite">{announced}</p>
	<p class="body" id="ob-body-{uid}">{body}</p>
	<div class="foot">
		{#each quiet as a (a.id)}
			<button type="button" class="act quiet" data-kind="quiet" onclick={() => onAction(a.id)}>
				{a.label}
			</button>
		{/each}
		<span class="spacer"></span>
		{#each rest as a (a.id)}
			<button
				type="button"
				class="act {a.kind}"
				data-kind={a.kind}
				data-autofocus={a.kind === 'primary' ? '' : undefined}
				aria-describedby={a.kind === 'primary' && step
					? `ob-title-${uid} ob-body-${uid}`
					: undefined}
				onclick={() => onAction(a.id)}
			>
				{a.label}
			</button>
		{/each}
	</div>
</div>

<style>
	.balloon {
		position: fixed;
		z-index: 1200;
		box-sizing: border-box;
		max-inline-size: 22rem;
		padding: var(--space-4) var(--space-5);
		background: var(--ink-100);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		box-shadow: 0 24px 48px -16px rgb(0 0 0 / 0.65);
		color: var(--bone-100);
		font-family: var(--font-body);
		animation: rise var(--dur-med) var(--ease-out-soft) both;
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.balloon.sheet {
		max-inline-size: none;
	}
	/* The dialog itself is only a fallback focus target (tabindex -1,
	   never reached by Tab); the ring belongs to its controls. */
	.balloon:focus {
		outline: none;
	}
	@keyframes rise {
		from {
			opacity: 0;
			translate: 0 6px;
		}
	}
	.pointer {
		position: absolute;
		inline-size: 12px;
		block-size: 12px;
		background: var(--ink-100);
		border: 1px solid var(--ink-300);
		rotate: 45deg;
	}
	.sheet .pointer {
		display: none;
	}
	/* No locale reads right to left yet, so the inline axis is the
	   viewport's x, which placement reasons in. */
	[data-side='below'] .pointer {
		inset-block-start: -7px;
		inset-inline-start: calc(var(--arrow) - 6px);
		border-inline-end: none;
		border-block-end: none;
	}
	[data-side='above'] .pointer {
		inset-block-end: -7px;
		inset-inline-start: calc(var(--arrow) - 6px);
		border-inline-start: none;
		border-block-start: none;
	}
	[data-side='end'] .pointer {
		inset-inline-start: -7px;
		inset-block-start: calc(var(--arrow) - 6px);
		border-inline-end: none;
		border-block-start: none;
	}
	[data-side='start'] .pointer {
		inset-inline-end: -7px;
		inset-block-start: calc(var(--arrow) - 6px);
		border-inline-start: none;
		border-block-end: none;
	}

	p {
		margin: 0;
	}

	/* The eyebrow at the start, the step readout at the end. */
	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-3);
		margin-block-end: var(--space-2);
	}
	/* The app's eyebrow voice: key, rule, value. */
	.tag {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font: 600 var(--type-micro) / 1 var(--font-body);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		white-space: nowrap;
		color: var(--bone-100);
	}
	.tag-rule {
		inline-size: 1.5rem;
		block-size: 1px;
		background: var(--ob-accent);
	}
	.tag-version {
		font-family: var(--font-mono);
		font-weight: 400;
		letter-spacing: var(--tracking-meta);
		color: var(--bone-300);
	}

	/* The start veil's film strip, standing still: one 16:9 frame per
	   step, the passed ones filled, the current one twice as wide. */
	.step {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex: none;
	}
	.frames {
		display: flex;
		align-items: center;
		gap: 3px;
		--h: 8px;
	}
	.frame {
		block-size: var(--h);
		inline-size: calc(var(--h) * 16 / 9);
		border-radius: 1.5px;
		border: 1px solid color-mix(in oklab, var(--bone-100) 45%, transparent);
	}
	.frame.seen {
		background: var(--bone-300);
		border-color: var(--bone-300);
	}
	.frame.current {
		inline-size: calc(var(--h) * 32 / 9);
		background: var(--ob-accent);
		border-color: color-mix(in oklab, var(--ob-accent) 60%, var(--bone-100));
	}
	.step-label {
		font: 400 var(--type-micro) / 1 var(--font-mono);
		letter-spacing: var(--tracking-meta);
		font-variant-numeric: tabular-nums;
		color: var(--bone-300);
		white-space: nowrap;
	}

	.title {
		margin: 0 0 var(--space-2);
		font-family: var(--font-display);
		font-size: 1.375rem;
		font-weight: 500;
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
	}
	.body {
		max-inline-size: 34ch;
		color: var(--bone-200);
		font-size: 0.9375rem;
		line-height: var(--leading-prose);
	}
	.sheet .body {
		max-inline-size: 60ch;
	}
	.compact {
		max-inline-size: 26rem;
		padding: var(--space-3) var(--space-4);
	}
	.compact .title {
		font-size: 1.125rem;
		margin-block-end: var(--space-1);
	}
	.compact .body {
		max-inline-size: 44ch;
	}
	.compact .foot {
		margin-block-start: var(--space-2);
	}

	.spacer {
		flex: 1 1 0;
	}
	.foot {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-block-start: var(--space-4);
	}
	.act {
		padding: var(--space-2) var(--space-4);
		border: 1px solid transparent;
		border-radius: var(--radius-control);
		font: 500 var(--type-meta) / 1 var(--font-body);
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.primary {
		border-color: var(--bone-100);
		background: var(--bone-100);
		color: var(--ink-000);
	}
	.primary:hover {
		background: var(--bone-200);
		border-color: var(--bone-200);
	}
	.secondary {
		border-color: var(--ink-300);
		background: transparent;
		color: var(--bone-100);
	}
	.secondary:hover {
		border-color: var(--bone-300);
	}
	/* A text action: no box at rest, so it never competes with the
	   primary, trimmed at the start to line up with the body text. */
	.quiet {
		padding-inline: var(--space-1);
		margin-inline-start: calc(var(--space-1) * -1);
		background: transparent;
		color: var(--bone-300);
	}
	.quiet:hover {
		color: var(--bone-100);
	}

	/* Contrast themes: system colours only. The edge and pointer are the
	   text colour, the rule too; nothing rests on a shadow or a fill. */
	@media (forced-colors: active) {
		.balloon,
		.pointer {
			border-color: CanvasText;
			box-shadow: none;
		}
		.pointer {
			background: Canvas;
		}
		.tag-rule {
			forced-color-adjust: none;
			background: CanvasText;
		}
		.frames {
			forced-color-adjust: none;
		}
		/* Ahead is a dashed gate, so seen and ahead differ by more than fill. */
		.frame {
			border-color: CanvasText;
			border-style: dashed;
			background: Canvas;
		}
		.frame.seen {
			border-style: solid;
			background: CanvasText;
			border-color: CanvasText;
		}
		.frame.current {
			border-style: solid;
			background: Highlight;
			border-color: Highlight;
		}
		/* A text action is underlined, so it still reads as a control. */
		.quiet {
			text-decoration: underline;
		}
	}
</style>
