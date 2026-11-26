/**
 * Each tip step's words. The backend sends a tip's id and places only
 * (AGENTS.md §4); the title and body of step `n` (from 1) of tip `id`
 * are the messages `onboarding_<id>_<n>_title` and `…_body`, the id
 * with `-` as `_`. `tests/arch/onboarding_places.sh` holds every locale
 * to the registry, and `words.test.ts` holds this table to it.
 *
 * A static switch, like the About page's notes: the build compiles each
 * `m.*` into a callable only where it is called by name, so the words
 * cannot be looked up by a key built at run time.
 */
import { m } from '$lib/paraglide/messages';

export interface StepWords {
	title: () => string;
	body: () => string;
}

/** Step `step` (from 1) of tip `id`, or null for one with no words. */
export function stepWords(id: string, step: number): StepWords | null {
	switch (`${id}#${step}`) {
		case 'watch-order#1':
			return { title: m.onboarding_watch_order_1_title, body: m.onboarding_watch_order_1_body };
		default:
			return null;
	}
}
