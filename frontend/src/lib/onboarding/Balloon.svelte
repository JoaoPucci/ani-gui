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
    the accent marks only the eyebrow's rule.
-->
<script lang="ts" module>
	export interface BalloonAction {
		id: string;
		label: string;
		kind: 'primary' | 'secondary';
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
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
		onAction,
		measure
	}: {
		/** The balloon's element, for the host to hold as a modal. */
		root?: HTMLDivElement;
		title: string;
		body: string;
		/** The release that brought the feature, e.g. "0.19.0". */
		version: string;
		/** The eyebrow's key, e.g. "New". */
		newLabel: string;
		placement: Placement | null;
		accent: string;
		/** A short window: a tighter balloon. */
		compact: boolean;
		actions: BalloonAction[];
		onAction: (id: string) => void;
		measure: (size: { w: number; h: number }) => void;
	} = $props();

	const uid = Math.random().toString(36).slice(2, 8);

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
	<p class="tag">
		<span>{newLabel}</span>
		<span class="tag-rule" aria-hidden="true"></span>
		<span class="tag-version">{version}</span>
	</p>
	<h2 class="title" id="ob-title-{uid}">{title}</h2>
	<p class="body" id="ob-body-{uid}">{body}</p>
	<div class="foot">
		{#each actions as a (a.id)}
			<button
				type="button"
				class="act {a.kind}"
				data-kind={a.kind}
				data-autofocus={a.kind === 'primary' ? '' : undefined}
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

	/* The app's eyebrow voice: key, rule, value. */
	.tag {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-block-end: var(--space-2);
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

	.foot {
		display: flex;
		align-items: center;
		justify-content: flex-end;
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
	}
</style>
