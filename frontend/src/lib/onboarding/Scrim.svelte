<!--
  The spotlight's dim, in three layers fixed to the window
  ($./spotlight draws their paths):

  - `.dim`, ink over the whole window at partial opacity, cut out at the
    lit area. It takes no pointer events. Opacity carries the dim, not a
    colour's alpha, so a contrast theme, which forces colours, keeps it.
  - `.catch`, a transparent click-catcher cut out at the place itself
    and at the window's own buttons. A press anywhere else stops here,
    reaches nothing under it, and is handed to `onOutside`. It is
    `no-drag`, so a press over the top bar's drag area closes the tip
    rather than moving the window.
  - `.frame`, four corner ticks in the show's colour just outside the
    lit area; in a contrast theme, a whole Highlight outline as well.

  Between a sequence's steps the cut-out and the ticks glide to the next
  place, one structural move (`travelling`); at rest they follow the
  place at once, so a scroll never leaves them trailing. With reduced
  motion they jump. The catcher never glides: its cut-out is where the
  place is now.

  The layer holds no text and is set `direction: ltr`, so its logical
  properties map to the viewport axes the rects use.
-->
<script lang="ts">
	import type { Rect } from './place';
	import { catchClip, dimClip } from './spotlight';

	let {
		root = $bindable(),
		lit,
		place,
		passHoles,
		view,
		accent,
		travelling = false,
		onArrived = () => {},
		onOutside
	}: {
		/** The layer's element, which the host keeps out of the inert page. */
		root?: HTMLDivElement;
		/** The lit area, viewport px. */
		lit: Rect;
		/** The place itself: presses land on it. */
		place: Rect;
		/** The window's buttons: dimmed, but a press still reaches them. */
		passHoles: Rect[];
		view: { w: number; h: number };
		accent: string;
		/** Moving between a sequence's steps. */
		travelling?: boolean;
		/** The glide is over. */
		onArrived?: () => void;
		onOutside: () => void;
	} = $props();

	/** As long as the glide (`--dur-slow`), and past it. */
	const GLIDE_MS = 700;
	$effect(() => {
		if (!travelling) return;
		const t = setTimeout(onArrived, GLIDE_MS);
		return () => clearTimeout(t);
	});

	/** How far outside the lit area the ticks stand. */
	const OUTSET = 8;

	function onPress(e: PointerEvent) {
		// No focus change, no text selection, no click on what is under.
		e.preventDefault();
		e.stopPropagation();
		onOutside();
	}
</script>

<div
	bind:this={root}
	class="scrim"
	class:travelling
	style:--ob-accent={accent}
	aria-hidden="true"
	data-testid="onboarding-scrim"
>
	<div class="dim" style:clip-path={dimClip(lit, view)}></div>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="catch"
		style:clip-path={catchClip(place, passHoles, view)}
		onpointerdown={onPress}
		oncontextmenu={(e) => e.preventDefault()}
	></div>
	<div
		class="frame"
		style:translate="{lit.x - OUTSET}px {lit.y - OUTSET}px"
		style:inline-size="{lit.w + OUTSET * 2}px"
		style:block-size="{lit.h + OUTSET * 2}px"
	>
		<span class="edge"></span>
		<span class="tick tl"></span><span class="tick tr"></span>
		<span class="tick bl"></span><span class="tick br"></span>
	</div>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 1190;
		pointer-events: none;
		direction: ltr;
	}
	.dim,
	.catch {
		position: absolute;
		inset: 0;
	}
	/* Theater-dim, the player's own move. */
	.dim {
		background: var(--ink-000);
		opacity: 0.74;
		animation: dim-in var(--dur-slow) var(--ease-out-soft) both;
	}
	.travelling .dim {
		transition: clip-path var(--dur-slow) var(--ease-in-out);
	}
	.travelling .frame {
		transition:
			translate var(--dur-slow) var(--ease-in-out),
			inline-size var(--dur-slow) var(--ease-in-out),
			block-size var(--dur-slow) var(--ease-in-out);
	}
	@keyframes dim-in {
		from {
			opacity: 0;
		}
	}
	.catch {
		pointer-events: auto;
		background: transparent;
		-webkit-app-region: no-drag;
	}
	.frame {
		position: absolute;
		inset-block-start: 0;
		inset-inline-start: 0;
	}
	.edge {
		position: absolute;
		inset: 8px;
		border-radius: var(--radius-card);
	}
	.tick {
		position: absolute;
		inline-size: 14px;
		block-size: 14px;
		border-color: var(--ob-accent);
		border-style: solid;
		border-width: 0;
	}
	.tl {
		inset-block-start: 0;
		inset-inline-start: 0;
		border-block-start-width: 2px;
		border-inline-start-width: 2px;
	}
	.tr {
		inset-block-start: 0;
		inset-inline-end: 0;
		border-block-start-width: 2px;
		border-inline-end-width: 2px;
	}
	.bl {
		inset-block-end: 0;
		inset-inline-start: 0;
		border-block-end-width: 2px;
		border-inline-start-width: 2px;
	}
	.br {
		inset-block-end: 0;
		inset-inline-end: 0;
		border-block-end-width: 2px;
		border-inline-end-width: 2px;
	}

	/* Contrast themes: the dim is the theme's Canvas at the same opacity,
	   the ticks are Highlight, and the lit area gets a whole Highlight
	   outline, so it is marked by a line and not by the dim's edge alone. */
	@media (forced-colors: active) {
		.scrim {
			forced-color-adjust: none;
		}
		.dim {
			background: Canvas;
		}
		.tick {
			border-color: Highlight;
		}
		.edge {
			outline: 2px solid Highlight;
		}
	}
</style>
