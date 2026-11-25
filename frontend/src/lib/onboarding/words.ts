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
		case 'franchise-entry#1':
			return {
				title: m.onboarding_franchise_entry_1_title,
				body: m.onboarding_franchise_entry_1_body
			};
		case 'franchise-tour#1':
			return {
				title: m.onboarding_franchise_tour_1_title,
				body: m.onboarding_franchise_tour_1_body
			};
		case 'franchise-tour#2':
			return {
				title: m.onboarding_franchise_tour_2_title,
				body: m.onboarding_franchise_tour_2_body
			};
		case 'franchise-tour#3':
			return {
				title: m.onboarding_franchise_tour_3_title,
				body: m.onboarding_franchise_tour_3_body
			};
		case 'player-up-next#1':
			return {
				title: m.onboarding_player_up_next_1_title,
				body: m.onboarding_player_up_next_1_body
			};
		default:
			return null;
	}
}
