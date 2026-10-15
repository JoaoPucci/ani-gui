<!--
  Hero framing switch: which image the hero shows and how it is framed.
  The pages render it inside the hero band while the diagnostics page's
  toggle is on. A choice goes to the page through `onchoose`; the page
  applies it to the hero at once and keeps it for the next load.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { HeroOrigin } from './framing';
	import {
		HERO_FRAMINGS,
		HERO_SOURCES,
		type HeroFraming,
		type HeroPrefs,
		type HeroSource
	} from './prefs';

	let {
		prefs,
		origin,
		onchoose
	}: {
		prefs: HeroPrefs;
		origin: HeroOrigin | null;
		/** A choice made in the switch; the page applies and keeps it. */
		onchoose: (next: HeroPrefs) => void;
	} = $props();

	const framingLabel: Record<HeroFraming, () => string> = {
		top: m.hero_framing_top,
		contain: m.hero_framing_contain,
		pillar: m.hero_framing_pillar,
		focus: m.hero_framing_focus
	};
	const sourceLabel: Record<HeroSource, () => string> = {
		kitsu: m.hero_framing_source_kitsu,
		anilist: m.hero_framing_source_anilist
	};
	// The readout shares a source's words on purpose: when the pressed
	// source and the readout agree, the preferred image was there; when
	// they differ, the fallback is on screen.
	const originLabel: Record<HeroOrigin | 'none', () => string> = {
		anilist: m.hero_framing_origin_anilist,
		kitsu: m.hero_framing_origin_kitsu,
		poster: m.hero_framing_origin_poster,
		none: m.hero_framing_origin_none
	};
</script>

<div class="hero-framing-switch" role="group" aria-label={m.hero_framing_group_label()}>
	{#each HERO_FRAMINGS as f (f)}
		<button
			type="button"
			aria-pressed={prefs.framing === f}
			onclick={() => onchoose({ ...prefs, framing: f })}>{framingLabel[f]()}</button
		>
	{/each}
	<span class="sep" aria-hidden="true"></span>
	{#each HERO_SOURCES as s (s)}
		<button
			type="button"
			aria-pressed={prefs.source === s}
			onclick={() => onchoose({ ...prefs, source: s })}>{sourceLabel[s]()}</button
		>
	{/each}
	<span class="sep" aria-hidden="true"></span>
	<span class="origin">
		<span class="sr-only">{m.hero_framing_origin_prefix()}</span>
		{originLabel[origin ?? 'none']()}
	</span>
</div>

<style>
	/* Lives inside the hero band it frames: absolute, not fixed, so it
	   shares no edge with the toast stack or the download bar, leaves
	   with the banner on scroll, and is immune to the page roots' enter
	   animations — a transform left on the detail page's root makes a
	   `fixed` child anchor to the page, not the window. Top-right is
	   the corner both heroes leave free: home puts its copy and pager
	   at the bottom, detail's masthead overlaps the bottom fade. */
	.hero-framing-switch {
		position: absolute;
		/* Above the scrim and grain; level with the home pager. */
		z-index: 3;
		/* Clear the glassy topbar the band bleeds under. */
		inset-block-start: calc(var(--topbar-h) + var(--space-5));
		/* The band overshoots the main area's gutter on the right; pull
		   in by it plus the strip pad to line up with the home pager
		   and the strips' right edge. */
		inset-inline-end: calc(var(--space-7) + var(--space-8));
		display: flex;
		/* One row, always: the band is full-bleed and never narrower than
		   the window, and a second row sat over the home title and the
		   detail masthead at the window's minimum width. The readout is
		   the one item that gives way. */
		flex-wrap: nowrap;
		align-items: center;
		gap: 2px;
		padding: 3px;
		border-radius: var(--radius-control);
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		box-shadow: var(--shadow-card-rest);
		font-family: var(--font-body);
		font-size: var(--type-micro);
		line-height: 1;
		color: var(--bone-300);
	}
	button {
		padding: 6px 8px;
		border: 0;
		border-radius: 4px;
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		white-space: nowrap;
		/* --dur-fast is 0ms under prefers-reduced-motion (tokens.css). */
		transition:
			background-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
	}
	button:hover {
		color: var(--bone-100);
	}
	button[aria-pressed='true'] {
		background: var(--bone-100);
		color: var(--ink-000);
	}
	/* Keyboard focus takes the app's global ring (tokens.css), which
	   on the detail page is the show's own accent. */
	.sep {
		inline-size: 1px;
		block-size: 1rem;
		margin-inline: 4px;
		background: var(--ink-200);
	}
	/* A fact beside the controls, in the app's voice for metadata; the
	   one item that shrinks when the row is short of room. */
	.origin {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		padding-inline: 6px;
		font-family: var(--font-mono);
		letter-spacing: var(--tracking-meta);
		white-space: nowrap;
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		margin: -1px;
		padding: 0;
		overflow: hidden;
		clip: rect(0 0 0 0);
		white-space: nowrap;
		border: 0;
	}
</style>
