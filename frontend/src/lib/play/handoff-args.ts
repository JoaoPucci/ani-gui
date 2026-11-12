import type { KitsuAnimeRef, PlayArgs, VersionKind } from '$lib/api';
import { buildPlayArgs } from './play-args';

/**
 * The request a handoff — the external player or Syncplay — sends
 * for an episode: the same request the embedded play sends, so the
 * backend resolves through the same walk and, once the player has
 * started, records the watch under the same Kitsu id. The id is
 * what lets the handoff's history row be found from the detail page
 * and weighed against another provider's row; without it the
 * backend has nothing to persist the show's reverse mapping under.
 *
 * A guessed Continue card's session names, as its own plays do, the
 * card's row and its guess apart from the page's id
 * (`sessionShow` in `./session-show`): the backend notes the card
 * beside a row the handoff writes on another listing, so the card
 * keeps standing for the line the user clicked, and records the guess
 * only on the card's own listing.
 */
export function handoffArgs(input: {
	title: string;
	episode: number;
	kitsuId: string;
	/** The guessed card's row the session's plays name (`guessRow`). */
	guessRow?: string;
	/** The card's row a session no guess opened names (`cardRow`). */
	cardRow?: string;
	/** The guess the session's plays name for the card (`guessId`). */
	guessId?: string;
	config: { mode: string };
	detail: KitsuAnimeRef | null;
	/** The version the embedded play names for the show. */
	version?: VersionKind;
	/** The captions choice remembered for the show, which the external
	 *  player starts on as the embedded one does. */
	caption?: string;
}): PlayArgs {
	return buildPlayArgs({
		title: input.title,
		episode: input.episode,
		mode: input.config.mode === 'dub' ? 'dub' : 'sub',
		show: input.detail,
		kitsuId: input.kitsuId,
		guessRow: input.guessRow,
		cardRow: input.cardRow,
		guessId: input.guessId,
		version: input.version,
		caption: input.caption
	});
}
