<!--
  GuessSlip — the play page's note for a guessed session, attached to
  the now-playing row under the player. It never sits over the picture
  and never hides with the player's controls; fullscreen shows only the
  player, so it is not there either.

  While the session is a guess it names the title the history saved and
  asks "Is this the show you meant?", with Yes and No: No opens the
  picker. After the viewer picked another show and kept watching this
  one, it says the pick is saved and offers "Switch to it now". The
  session a switch opened says once which show plays now. It never
  speaks of a link or a guess: how the app keeps the answer is not the
  user's concern. Hide sets it aside for the card's row until the app
  restarts ($lib/history/in-place-link.svelte.ts).

  While the offer is decided ("holding") it keeps its room, empty: no
  mark, text or button that the answer may take away, so the text
  landing moves nothing. The text arrives with a short fade, none under
  reduced motion. When nothing is offered the room slides shut, at once
  under reduced motion.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import GuessMark from './GuessMark.svelte';
	import { slide } from '$lib/motion/transitions';
	import type { SlipState } from '$lib/history/in-place-link.svelte';

	let {
		view,
		title,
		linkedTitle,
		switchedTitle = null,
		busy = false,
		switching = false,
		onchange,
		onright,
		onswitch,
		onhide
	}: {
		view: Exclude<SlipState, 'none'>;
		/** The title the history saved for the card. */
		title: string | null;
		/** The show picked from here, in the "linked" state. */
		linkedTitle: string | null;
		/** The show a switch opened the session for, in the "switched"
		 *  state. */
		switchedTitle?: string | null;
		/** A pick being saved. */
		busy?: boolean;
		/** A switch to the pick on its way. */
		switching?: boolean;
		/** No: open the picker. */
		onchange: () => void;
		/** Yes. */
		onright: () => void;
		onswitch: () => void;
		onhide: () => void;
	} = $props();
</script>

<aside
	class="guess-slip"
	aria-labelledby={view === 'holding' ? undefined : 'guess-slip-text'}
	aria-label={view === 'holding' ? m.play_link_slip_aria() : undefined}
	data-testid="guess-slip"
	data-state={view}
	out:slide={{ duration: 180 }}
>
	{#if view === 'linked' || view === 'switched'}
		<svg class="mark" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
			<path
				d="M5 12.5l4.5 4.5L19 7.5"
				fill="none"
				stroke="currentColor"
				stroke-width="2.25"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
		{#if view === 'linked'}
			<p class="text" id="guess-slip-text">
				{m.play_link_linked_note({ title: linkedTitle ?? '' })}
			</p>
			<span class="actions">
				<button
					type="button"
					class="act act-strong"
					onclick={onswitch}
					disabled={switching}
					aria-busy={switching}>{m.play_link_switch_now()}</button
				>
			</span>
		{:else}
			<p class="text" id="guess-slip-text">
				{m.play_link_switched_note({ title: switchedTitle ?? '' })}
			</p>
		{/if}
	{:else if view === 'guess' && title !== null}
		<GuessMark class="mark" />
		<p class="text" id="guess-slip-text">{m.play_link_guess_note({ title })}</p>
		<span class="actions">
			<button type="button" class="act" onclick={onright} disabled={busy}
				>{m.play_link_guess_right()}</button
			>
			<button type="button" class="act act-strong" onclick={onchange} disabled={busy}
				>{m.play_link_change_button()}</button
			>
		</span>
	{/if}
	{#if view !== 'holding'}
		<button type="button" class="hide" aria-label={m.play_link_hide_aria()} onclick={onhide}>
			<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"
				><path
					d="M6 6 18 18 M18 6 6 18"
					fill="none"
					stroke="currentColor"
					stroke-width="2.25"
					stroke-linecap="round"
				/></svg
			>
		</button>
	{/if}
</aside>

<style>
	/* Attached to the now-playing row below: the watch column's own gap
	   is cut back to a small one, so the slip reads as that row's note
	   rather than a page alert. */
	.guess-slip {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		min-inline-size: 0;
		min-block-size: 2.5rem;
		margin-block-end: calc(var(--space-3) - var(--space-7));
		padding-block: var(--space-1);
		padding-inline: var(--space-4) 2.75rem;
		border: 1px solid var(--ink-200);
		border-inline-start: 2px solid color-mix(in oklab, var(--accent) 70%, transparent);
		border-radius: var(--radius-control);
		background: color-mix(in oklab, var(--accent) 5%, var(--ink-050));
		font-size: var(--type-meta);
		line-height: var(--leading-prose);
		color: var(--bone-300);
	}
	/* The room held while the offer is decided: nothing painted in it. */
	.guess-slip[data-state='holding'] {
		border-color: transparent;
		background: none;
	}
	.guess-slip :global(.mark) {
		color: color-mix(in oklab, var(--accent) 85%, var(--bone-100));
	}
	.text,
	.actions {
		animation: slip-text-in var(--dur-med) var(--ease-out-soft) both;
	}
	/* Wraps rather than cuts: a cut sentence loses what was guessed. */
	.text {
		flex: 1 1 18rem;
		min-inline-size: 0;
		margin: 0;
	}
	.actions {
		display: inline-flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}
	.act {
		padding: 4px var(--space-3);
		border: 1px solid transparent;
		border-radius: var(--radius-pill);
		background: none;
		font: inherit;
		color: var(--bone-200);
		white-space: nowrap;
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.act:disabled {
		opacity: 0.6;
		cursor: progress;
	}
	.act-strong {
		border-color: color-mix(in oklab, var(--accent) 55%, var(--ink-300));
		color: var(--bone-100);
	}
	.act:hover:not(:disabled),
	.act:focus-visible {
		color: var(--bone-100);
		border-color: var(--accent);
	}
	.act:focus-visible,
	.hide:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	.hide {
		position: absolute;
		inset-block-start: 50%;
		inset-inline-end: var(--space-1);
		translate: 0 -50%;
		inline-size: 2rem;
		block-size: 2rem;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: var(--radius-pill);
		background: none;
		color: var(--bone-400);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.hide:hover,
	.hide:focus-visible {
		color: var(--bone-100);
		background: var(--ink-200);
	}
	@keyframes slip-text-in {
		from {
			opacity: 0;
			transform: translateY(2px);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.text,
		.actions {
			animation: none;
		}
	}
</style>
