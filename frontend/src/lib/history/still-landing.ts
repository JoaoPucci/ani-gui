// A still that lands on a Continue card after its look has settled.
//
// AniList's pacing can hold an episode's still back; it lands later on
// the episode details the card already settled on. That is the one
// change a settled card takes (./look-book). Details that arrive with a
// still after the card settled at the episode wait are not a landing:
// the card keeps the poster it settled on.

import type { SettledLook } from './card-look';

/**
 * Whether `after` is `before` with an episode still that has since
 * landed: the same show and episode, a look without a still before
 * (the poster, or the placeholder when there is none), the episode's
 * still after.
 */
export function stillLanded(
	before: SettledLook,
	after: SettledLook,
	poster: string | null
): boolean {
	if (before.matchId !== after.matchId || before.episode !== after.episode) return false;
	const hadStill = before.image !== null && before.image.src !== poster;
	const hasStill = after.image !== null && after.image.src !== poster;
	return !hadStill && hasStill;
}

/**
 * Follows what each card's details last settled on, so a still that
 * lands later can be told from details that came with one.
 */
export function createStillLandings() {
	const truths = new Map<string, SettledLook>();
	return {
		/**
		 * Records `truth`, the look the card's details settle on now (null:
		 * not in), and returns it when it is a still landing on `shown`,
		 * the look the card shows: the details' last look had none, and
		 * the card still shows that look. Otherwise null.
		 */
		observe(
			entryId: string,
			truth: SettledLook | null,
			shown: SettledLook | null,
			poster: string | null
		): SettledLook | null {
			const before = truths.get(entryId);
			if (!truth) {
				truths.delete(entryId);
				return null;
			}
			truths.set(entryId, truth);
			if (!before || !shown || shown.image?.src !== before.image?.src) return null;
			return stillLanded(before, truth, poster) ? truth : null;
		}
	};
}
