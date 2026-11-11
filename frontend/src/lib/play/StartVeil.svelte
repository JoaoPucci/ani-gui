<!--
  The start veil: what the player frame shows over the picture from a
  stream attaching to its first frame (`start`), and while a playing
  stream waits for data (`stall`). Its mode is decided in
  `$lib/play/start-veil`; its look is the Diagnostics switch
  (`$lib/play/start-look`):

  - `still`: the episode's still, blurred and dimmed past any focus a
    paused frame could have (or the blurred poster, or ink, or an
    accent field when there is no usable image), under a centred film
    strip and a caption.
  - `card`: the same still with an editorial title card lower left,
    the episode's oversized numeral, its title and the show's, and the
    film strip in the card's status line.

  A stall has no backdrop (the frozen frame is the picture): a compact
  strip on a small dark pill, kept inside the video picture.

  Layering: the veil lives inside the player frame, above the picture
  and below the player's controls, by order alone: it takes no
  z-index, and the controls come after it in the frame. Anything the app lays over or
  beside the player for a shared session, such as Watch Together's
  room panel and room messages, sits outside the frame or above it:
  the veil must never cover them.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { fade } from '$lib/motion/transitions';
	import { clock, episodeNumeral, type VeilMode } from '$lib/play/start-veil';
	import type { StartLook } from '$lib/play/start-look';

	interface Props {
		look: StartLook;
		mode: VeilMode;
		/** The episode's still, through the image proxy. */
		still: string | null;
		/** The show's poster, the backdrop when there is no still. */
		poster: string | null;
		episode: number;
		episodeTitle: string | null;
		showTitle: string | null;
		/** Where a resume lands, in seconds; null for a start at the top. */
		resumeAt: number | null;
		fullscreen: boolean;
	}

	let { look, mode, still, poster, episode, episodeTitle, showTitle, resumeAt, fullscreen }: Props =
		$props();

	// Sources that failed to load; the backdrop falls through to the next.
	let failed = $state<string[]>([]);
	let loaded = $state<string | null>(null);

	const backdrop = $derived([still, poster].find((s) => s && !failed.includes(s)) ?? null);
	// A cached image can finish before the load listener is in place.
	const settled = (img: HTMLImageElement) => {
		if (img.complete && img.naturalWidth > 0) loaded = img.getAttribute('src');
	};
	const backdropIsPoster = $derived(backdrop !== null && backdrop !== still);

	const n = $derived(String(episode));
	const resumeLine = $derived(
		resumeAt === null ? null : m.playstart_resuming({ time: clock(resumeAt) })
	);
	const caption = $derived(resumeLine ?? m.playstart_starting({ n }));
</script>

{#if mode !== 'off'}
	<div
		class="veil start-veil"
		class:fullscreen
		data-look={look}
		data-mode={mode}
		aria-hidden="true"
		out:fade={{ duration: 240 }}
	>
		{#if mode === 'start'}
			<div class="backdrop" class:field={backdrop === null}>
				{#if backdrop}
					<img
						src={backdrop}
						alt=""
						class:poster={backdropIsPoster}
						class:loaded={loaded === backdrop}
						onload={() => (loaded = backdrop)}
						onerror={() => backdrop && (failed = [...failed, backdrop])}
						{@attach settled}
					/>
				{/if}
				<div class="shade"></div>
			</div>
		{/if}

		{#if mode === 'stall'}
			<div class="picture stall">
				<span class="pill">{@render strip()}</span>
			</div>
		{:else if look === 'still'}
			<div class="centre">
				<span class="strip-l">{@render strip()}</span>
				<p class="centre-caption">{caption}</p>
			</div>
		{:else}
			<div class="card">
				<span class="numeral">{episodeNumeral(episode)}</span>
				<div class="card-text">
					<p class="card-title">{episodeTitle ?? m.playstart_episode({ n })}</p>
					{#if showTitle}<p class="card-show">{showTitle}</p>{/if}
				</div>
				<span class="rule"></span>
				<p class="card-status">
					<span class="strip-s">{@render strip()}</span>
					<span class="card-caption">{caption}</span>
				</p>
			</div>
		{/if}
	</div>
{/if}

{#snippet strip()}
	<span class="strip">
		{#each [0, 1, 2, 3, 4] as i (i)}<span class="cell" style:--i={i}></span>{/each}
	</span>
{/snippet}

<style>
	/* — Frame ——————————————————————————————————————————————————— */
	.veil {
		position: absolute;
		inset: 0;
		pointer-events: none;
		container-type: size;
		overflow: hidden;
		color: var(--bone-100);
		/* The controls bar owns the bottom 84px; everything here sits
		   above a margin that clears it, and starts on its inline edge. */
		--safe-end: 6rem;
		--edge: clamp(1.25rem, 3cqi, 3.5rem);
	}
	.veil.fullscreen {
		--safe-end: 7rem;
	}
	p {
		margin: 0;
	}

	/* The loader arrives softly, and only once a start or a stall has
	   lasted a beat: one that lands sooner never flashes it. The beat
	   is a transition delay, which reduced motion keeps (tokens.css)
	   while it takes every animation off. The backdrop and the card's
	   text are not delayed: they stand in for a black frame, or, at a
	   fullscreen switch, for the episode being left. */
	.veil {
		--loader-beat: 400ms;
	}
	.strip,
	.centre-caption,
	.pill {
		transition: opacity var(--dur-slow) var(--ease-out-soft) var(--loader-beat);
	}
	@starting-style {
		.strip,
		.centre-caption,
		.pill {
			opacity: 0;
		}
	}

	/* The video picture inside the frame: a pillarboxed or letterboxed
	   source leaves black bars, and the stall belongs on the picture,
	   not on a bar. Its end edge also clears the controls. */
	.picture {
		position: absolute;
		inset-inline: var(--player-letterbox-x, 0px);
		inset-block-start: var(--player-letterbox-y, 0px);
		inset-block-end: max(var(--player-letterbox-y, 0px), var(--safe-end));
	}

	/* — Backdrop (both looks, start) ——————————————————————————————— */
	/* Ink until the still arrives, so nothing flashes the accent first;
	   the accent field is only for a show with no usable image. */
	.backdrop {
		position: absolute;
		inset: 0;
		background: var(--ink-000);
	}
	.backdrop.field {
		background:
			radial-gradient(
				120% 90% at 50% 40%,
				color-mix(in oklab, var(--accent) 24%, var(--ink-000)),
				var(--ink-000) 75%
			),
			var(--ink-000);
	}
	.backdrop img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		opacity: 0;
		/* Blurred well past any focus a paused frame could have: a hint
		   of what is coming, never mistaken for the picture itself. */
		filter: blur(14px) brightness(0.5) saturate(0.75);
		transform: scale(1.08);
		transition: opacity var(--dur-slow) var(--ease-out-soft);
	}
	.backdrop img.poster {
		filter: blur(32px) brightness(0.42) saturate(0.9);
		transform: scale(1.2);
	}
	.backdrop img.loaded {
		opacity: 1;
	}
	/* The show's accent laid into the shadows, then a floor of ink so
	   the controls and the card read on any still. */
	.shade {
		position: absolute;
		inset: 0;
		background:
			linear-gradient(to top, rgb(0 0 0 / 0.78), rgb(0 0 0 / 0.1) 55%, rgb(0 0 0 / 0.25)),
			color-mix(in oklab, var(--accent) 16%, transparent);
	}
	.veil[data-look='card'] .shade {
		background:
			radial-gradient(130% 110% at 0% 100%, rgb(0 0 0 / 0.82), rgb(0 0 0 / 0.15) 70%),
			linear-gradient(to top, rgb(0 0 0 / 0.6), transparent 45%),
			color-mix(in oklab, var(--accent) 14%, transparent);
	}
	.veil:dir(rtl) .shade {
		transform: scaleX(-1);
	}

	/* — Film strip loader ——————————————————————————————————————— */
	/* Five 16:9 frames; a lit frame travels along them in reading
	   order, like a strip advancing through the gate. The lit colour
	   is mixed toward bone so a dark accent still clears the ink. */
	.strip {
		display: flex;
		gap: calc(var(--cell) * 0.32);
		--cell: clamp(12px, 1.3cqi, 22px);
		--lit: color-mix(in oklab, var(--accent) 60%, var(--bone-100));
	}
	.strip-l .strip {
		--cell: clamp(16px, 1.9cqi, 34px);
		filter: drop-shadow(0 0 6px rgb(0 0 0 / 0.6));
	}
	.strip-s .strip {
		--cell: clamp(11px, 1cqi, 18px);
	}
	.veil.fullscreen .stall .strip {
		--cell: clamp(18px, 1.6cqi, 30px);
	}
	.cell {
		inline-size: var(--cell);
		aspect-ratio: 16 / 9;
		border-radius: 2px;
		box-shadow: inset 0 0 0 1.5px color-mix(in oklab, var(--bone-100) 50%, transparent);
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
		animation: frame-lit 1.25s var(--ease-in-out) infinite;
		animation-delay: calc(var(--i) * 0.14s);
	}
	@keyframes frame-lit {
		0%,
		55%,
		100% {
			background: color-mix(in oklab, var(--bone-100) 6%, transparent);
			box-shadow: inset 0 0 0 1.5px color-mix(in oklab, var(--bone-100) 50%, transparent);
		}
		18% {
			background: var(--lit);
			box-shadow: inset 0 0 0 1.5px var(--lit);
		}
	}

	/* — Still: centred loader ——————————————————————————————————————— */
	/* Set a little below the middle of the space above the controls,
	   where the eye lands on a frame about to play. */
	.centre {
		position: absolute;
		inset: 0 0 var(--safe-end);
		padding-block-start: 16cqb;
		display: grid;
		place-content: center;
		justify-items: center;
		gap: clamp(0.75rem, 2.4cqb, 1.5rem);
	}
	.centre-caption {
		font-family: var(--font-body);
		font-size: clamp(0.875rem, 1.2cqi, 1.25rem);
		color: var(--bone-200);
		font-variant-numeric: tabular-nums;
		text-shadow: 0 1px 8px rgb(0 0 0 / 0.7);
	}

	/* — Stall (both looks) ———————————————————————————————————————————— */
	/* A small dark pill behind the strip only, so it reads on a
	   near-white frame without dimming the whole picture. */
	.stall {
		display: grid;
		place-items: center;
	}
	.pill {
		display: block;
		padding-block: clamp(7px, 0.8cqi, 18px);
		padding-inline: clamp(12px, 1.4cqi, 30px);
		border-radius: var(--radius-pill);
		background: rgb(0 0 0 / 0.55);
	}

	/* — Card: title card ————————————————————————————————————————— */
	.card {
		position: absolute;
		inset-inline-start: var(--edge);
		inset-block-end: var(--safe-end);
		inline-size: min(64cqi, 52rem);
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: end;
		column-gap: clamp(0.875rem, 2cqi, 2rem);
		row-gap: clamp(0.625rem, 1.6cqb, 1.125rem);
		text-shadow: 0 1px 12px rgb(0 0 0 / 0.55);
	}
	.numeral {
		font-family: var(--font-display);
		font-weight: 300;
		font-variant-numeric: tabular-nums lining-nums;
		font-size: clamp(3.5rem, min(20cqb, 12cqi), 10rem);
		line-height: 0.78;
		letter-spacing: -0.03em;
		color: var(--bone-100);
		font-optical-sizing: auto;
	}
	.card-text {
		display: grid;
		gap: 0.35em;
		padding-block-end: 0.1em;
	}
	.card-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-size: clamp(1.125rem, min(4cqb, 2.4cqi), 2.25rem);
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		overflow-wrap: anywhere;
	}
	.card-show {
		font-family: var(--font-body);
		font-size: clamp(0.8125rem, min(2.2cqb, 1.15cqi), 1.125rem);
		color: var(--bone-300);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	/* A plain divider, not a bar: it carries no progress, so it never
	   reads as finished. */
	.rule {
		grid-column: 1 / -1;
		display: block;
		block-size: 1px;
		background: color-mix(in oklab, var(--bone-100) 26%, transparent);
	}
	.card-status {
		grid-column: 1 / -1;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-inline-size: 0;
		font-family: var(--font-body);
		font-size: clamp(0.8125rem, min(2.2cqb, 1.1cqi), 1.125rem);
		color: color-mix(in oklab, var(--accent) 40%, var(--bone-100));
		font-variant-numeric: tabular-nums;
	}
	.card-status > span:last-child {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* — Short frames: the card drops the show line before it crowds
	   the top; the numeral shrinks with the frame already. */
	@container (max-height: 380px) {
		.card-show {
			display: none;
		}
	}

	/* — Reduced motion: nothing travels. The film strip lights every
	   frame at half strength, so the still strip reads as "working",
	   not as progress stuck part-way or finished. */
	@media (prefers-reduced-motion: reduce) {
		.cell {
			background: color-mix(in oklab, var(--lit) 50%, transparent);
			box-shadow: inset 0 0 0 1.5px var(--lit);
		}
		.backdrop img {
			transform: none;
		}
	}
</style>
