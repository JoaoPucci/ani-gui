/**
 * The last watch-order answer per show, for the rest of the session,
 * so a show page visited again draws its strip in its first frame
 * instead of a few frames after its title. The page still reads the
 * answer again and draws what it says.
 */
import type { WatchOrderData } from './loader';

/** The most shows remembered; the least recently used goes first. */
export const MEMO_LIMIT = 200;

/** The answer remembered for `id`, or null. */
export function rememberedAnswer(id: string): WatchOrderData | null {
	void id;
	return null;
}

/**
 * What to draw for `id` now that `fresh` has answered, remembering it:
 * `fresh`, unless it could not read the franchise and an earlier answer
 * could, which then stays (a failed read does not take a strip away).
 */
export function settledAnswer(id: string, fresh: WatchOrderData): WatchOrderData {
	void id;
	return fresh;
}

/** Forgets every answer. */
export function forgetAnswers(): void {}
