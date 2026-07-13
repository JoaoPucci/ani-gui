/**
 * Render state for the detail page's primary CTA area, derived from
 * the availability probe's lifecycle.
 *
 * The probe deliberately surfaces throttled / failed allmanga
 * searches as an error rather than a verdict (a transient failure
 * must not poison the cache with available:false). The template used
 * to map that null verdict to the same pulsing skeleton as an
 * in-flight probe — with no retry anywhere, the play-button area
 * loaded forever whenever the first probe hit a rate limit. A
 * settled-but-unknown verdict renders the real actions instead: the
 * lazy click path already surfaces the true error if the show can't
 * stream.
 */
export type CtaState = 'loading' | 'unavailable' | 'ready';

/** Map probe lifecycle → CTA area render state. */
export function ctaState(resolved: boolean, availability: boolean | null): CtaState {
	if (!resolved) return 'loading';
	if (availability === false) return 'unavailable';
	return 'ready';
}
