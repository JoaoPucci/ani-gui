<!--
  The player's load tip ($./load-tip): a tip in the player while the
  stream loads. It
  sits over the start veil in the frame's top inline-end corner, clear
  of the title card (lower inline-start) and the controls (bottom), and
  is drawn as a margin note: a crop-mark pair, the eyebrow, a serif
  title, the body and a "Got it", set straight on the veil like the
  card's own type.

  When the picture starts (`phase: 'playing'`) it stays in the same
  corner and folds to a small plate: the title and "Got it", the body
  gone, so it never competes with the picture or reaches the subtitle
  area. In a narrow frame the plate's "Got it" becomes a close mark
  named by `closeAria`.

  It is a note, not a dialog: it never takes focus, so Space and K keep
  driving the player, and Tab reaches its control in document order.
  Nothing here closes it on a timer.

  It sits in the player frame after the controls, so Tab reaches it
  there; it fills the frame and takes pointer events only on the note.
  Playing, the plate fades with the player's own chrome: out when the
  pointer leaves the frame, back on activity or when focus is in it.
  Focus rings are the app's own (`--ring`), never the show's colour.
-->
<script lang="ts">
	let {
		title,
		body,
		version,
		newLabel,
		accent,
		phase,
		gotItLabel,
		closeAria,
		onDismiss
	}: {
		title: string;
		body: string;
		/** The release that brought the feature, e.g. "0.19.0". */
		version: string;
		/** The eyebrow's key, e.g. "New". */
		newLabel: string;
		accent: string;
		/** The veil is up (`loading`) or the picture plays behind (`playing`). */
		phase: 'loading' | 'playing';
		gotItLabel: string;
		/** The close mark's name in a narrow frame, e.g. "Close tip". */
		closeAria: string;
		onDismiss: () => void;
	} = $props();

	const uid = Math.random().toString(36).slice(2, 8);
</script>

<div class="layer" style:--ob-accent={accent} data-testid="onboarding-load-tip">
	<div class="note" data-phase={phase} role="note" aria-labelledby="lt-title-{uid}">
		<span class="crop start" aria-hidden="true"></span>
		<span class="crop end" aria-hidden="true"></span>
		<div class="fold">
			<div class="fold-inner">
				<p class="tag">
					<span>{newLabel}</span>
					<span class="tag-rule" aria-hidden="true"></span>
					<span class="tag-version">{version}</span>
				</p>
			</div>
		</div>
		<p class="title" id="lt-title-{uid}">{title}</p>
		<div class="fold">
			<div class="fold-inner">
				<p class="body">{body}</p>
			</div>
		</div>
		<div class="foot">
			<button type="button" class="got-it" onclick={onDismiss}>{gotItLabel}</button>
			<button type="button" class="close" aria-label={closeAria} onclick={onDismiss}>
				<svg viewBox="0 0 12 12" aria-hidden="true">
					<path d="M2 2l8 8M10 2l-8 8" />
				</svg>
			</button>
		</div>
	</div>
</div>

<style>
	.layer {
		position: absolute;
		inset: 0;
		pointer-events: none;
		container-type: size;
		/* The veil's gutter, so the note and the title card share a frame
		   margin. */
		--edge: clamp(1.25rem, 3cqi, 3.5rem);
	}
	p {
		margin: 0;
	}

	/* — The note: loading ———————————————————————————————————————————— */
	.note {
		position: absolute;
		inset-block-start: var(--edge);
		inset-inline-end: var(--edge);
		inline-size: min(22rem, 34cqi);
		/* No clipping box here: a focus ring is never cut. A frame too
		   short for the full note folds it instead (below), so it stays
		   in the upper part, clear of the picture's centre and the
		   subtitle band. */
		pointer-events: auto;
		display: grid;
		padding: var(--space-4) var(--space-5);
		color: var(--bone-100);
		text-shadow: 0 1px 12px rgb(0 0 0 / 0.55);
		/* Not a card: the veil's own shade, deepened toward the corner so
		   the type reads on a bright still. */
		background: radial-gradient(
			120% 120% at 100% 0%,
			rgb(0 0 0 / 0.55),
			rgb(0 0 0 / 0.2) 45%,
			transparent 70%
		);
		border-radius: var(--radius-card);
		transition:
			inline-size var(--dur-slow) var(--ease-in-out),
			padding var(--dur-slow) var(--ease-in-out),
			background-color var(--dur-slow) var(--ease-out-soft);
	}
	.note:dir(rtl) {
		background: radial-gradient(
			120% 120% at 0% 0%,
			rgb(0 0 0 / 0.55),
			rgb(0 0 0 / 0.2) 45%,
			transparent 70%
		);
	}

	/* Crop marks at two opposite corners: the spotlight's registration
	   ticks, halved, so the tip reads as the same "this is new" mark. */
	.crop {
		position: absolute;
		inline-size: 12px;
		block-size: 12px;
		border: 0 solid var(--ob-accent);
	}
	.crop.start {
		inset-block-start: 0;
		inset-inline-start: 0;
		border-block-start-width: 2px;
		border-inline-start-width: 2px;
	}
	.crop.end {
		inset-block-end: 0;
		inset-inline-end: 0;
		border-block-end-width: 2px;
		border-inline-end-width: 2px;
	}

	/* A fold: a row that collapses to nothing without measuring. Hidden
	   when folded, so a screen reader does not read the gone text. */
	.fold {
		display: grid;
		grid-template-rows: 1fr;
		transition:
			grid-template-rows var(--dur-slow) var(--ease-in-out),
			opacity var(--dur-med) var(--ease-out-soft),
			visibility 0s linear 0s;
	}
	.fold-inner {
		min-block-size: 0;
		overflow: hidden;
	}

	.tag {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-block-end: var(--space-2);
		font: 600 var(--type-micro) / 1 var(--font-body);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
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
		font-family: var(--font-display);
		font-weight: 400;
		font-size: clamp(1.125rem, 1.6cqi, 1.5rem);
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		overflow-wrap: anywhere;
		transition: font-size var(--dur-slow) var(--ease-in-out);
	}
	.body {
		padding-block-start: var(--space-2);
		font-size: clamp(0.875rem, 1.1cqi, 1rem);
		line-height: var(--leading-prose);
		color: var(--bone-200);
		max-inline-size: 36ch;
	}
	.foot {
		display: flex;
		justify-content: flex-end;
		margin-block-start: var(--space-3);
	}
	/* Outlined, not the balloon's bone fill: on the veil a filled button
	   would be the brightest thing in the frame, brighter than the
	   loader it sits beside. */
	.got-it {
		padding: var(--space-2) var(--space-4);
		border: 1px solid color-mix(in oklab, var(--bone-100) 55%, transparent);
		border-radius: var(--radius-control);
		background: rgb(0 0 0 / 0.25);
		color: var(--bone-100);
		font: 500 var(--type-meta) / 1 var(--font-body);
		text-shadow: none;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.got-it:hover {
		border-color: var(--bone-100);
		background: rgb(0 0 0 / 0.4);
	}
	.close {
		display: none;
		place-items: center;
		inline-size: 1.75rem;
		block-size: 1.75rem;
		border: 1px solid transparent;
		border-radius: var(--radius-control);
		color: var(--bone-200);
	}
	.close:hover {
		color: var(--bone-100);
	}
	.close svg {
		inline-size: 12px;
		block-size: 12px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.5;
		stroke-linecap: round;
	}

	/* — Playing: the same corner, folded to a plate ——————————————————— */
	/* The picture is now the subject. The plate is solid ink (a bright
	   frame cannot wash it out) and small: one serif line and a quiet
	   control, side by side. */
	.note[data-phase='playing'] {
		inline-size: min(17rem, 30cqi);
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		column-gap: var(--space-3);
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
		background: color-mix(in oklab, var(--ink-100) 88%, transparent);
		text-shadow: none;
	}
	.note[data-phase='playing'] .fold {
		grid-template-rows: 0fr;
		grid-column: 1 / -1;
		opacity: 0;
		visibility: hidden;
		transition:
			grid-template-rows var(--dur-slow) var(--ease-in-out),
			opacity var(--dur-fast) var(--ease-out-soft),
			visibility 0s linear var(--dur-slow);
	}
	/* Playing, the "New" key stays (without the version), so the plate
	   never reads as one of the player's own controls. */
	.note[data-phase='playing'] .fold:has(.tag) {
		grid-template-rows: 1fr;
		opacity: 1;
		visibility: visible;
		margin-block-end: var(--space-1);
	}
	.note[data-phase='playing'] .tag-rule,
	.note[data-phase='playing'] .tag-version {
		display: none;
	}
	.note[data-phase='playing'] .title {
		grid-row: 2;
		grid-column: 1;
		font-size: var(--type-meta);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.note[data-phase='playing'] .foot {
		grid-row: 2;
		grid-column: 2;
		margin-block-start: 0;
	}
	.note[data-phase='playing'] .got-it {
		padding: var(--space-1) var(--space-3);
		background: transparent;
		border-color: var(--ink-300);
		color: var(--bone-200);
	}
	.note[data-phase='playing'] .got-it:hover {
		border-color: var(--bone-300);
		color: var(--bone-100);
	}
	/* Playing, the plate is chrome like the controls: hidden while the
	   pointer is away from the frame, shown on hover or with focus in it. */
	.note[data-phase='playing'] {
		opacity: 0;
		transition:
			opacity var(--dur-fast) var(--ease-out-soft),
			inline-size var(--dur-slow) var(--ease-in-out),
			padding var(--dur-slow) var(--ease-in-out);
	}
	:global(.player-frame:hover) .note[data-phase='playing'],
	.note[data-phase='playing']:focus-within {
		opacity: 1;
	}
	/* Playing, the crop marks hug the plate's own corners. */
	.note[data-phase='playing'] .crop {
		inline-size: 8px;
		block-size: 8px;
	}

	/* — Narrow frames (a small window, the mini player) ——————————————— */
	@container (inline-size <= 560px) {
		.note {
			inline-size: min(18rem, calc(100cqi - var(--edge) * 2));
		}
		.note[data-phase='playing'] {
			inline-size: min(14rem, 60cqi);
		}
		.note[data-phase='playing'] .got-it {
			display: none;
		}
		.note[data-phase='playing'] .close {
			display: grid;
		}
	}
	/* A frame too short for the full note shows it folded from the start. */
	@container (block-size <= 360px) {
		.note[data-phase='loading'] .fold:has(.body) {
			display: none;
		}
	}

	/* — Contrast themes: the note is a Canvas plate with a CanvasText
	   border in both phases (the veil's shade, a background, would carry
	   nothing there), and no crop marks. */
	@media (forced-colors: active) {
		.note,
		.note:dir(rtl) {
			forced-color-adjust: none;
			background: Canvas;
			color: CanvasText;
			border: 1px solid CanvasText;
			text-shadow: none;
		}
		.note[data-phase='playing'] {
			background: Canvas;
		}
		/* The marks would smudge the border at the rounded corners. */
		.crop {
			display: none;
		}
		.tag-rule {
			background: CanvasText;
		}
		.tag-version,
		.body {
			color: CanvasText;
		}
		.got-it,
		.note[data-phase='playing'] .got-it,
		.close {
			background: ButtonFace;
			color: ButtonText;
			border-color: ButtonText;
		}
		.got-it:hover,
		.note[data-phase='playing'] .got-it:hover,
		.close:hover {
			background: Highlight;
			color: HighlightText;
			border-color: Highlight;
		}
	}
</style>
