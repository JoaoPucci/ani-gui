/**
 * What a play session names as its show — split from
 * $lib/play/play-origin.ts, whose verdicts it combines, for the
 * per-file complexity bar.
 */

import { guessedId, guessRow, recordableId } from './play-origin';

/** What a session's plays name as their show, and what its watches are
 *  recorded and mirrored as.
 *
 *  A session no guess opened plays its page's show. A guessed one names
 *  its guess apart, with the card's row. Once another show was picked
 *  for the card from the session (`linked`), its later plays name the
 *  pick in place of the guess: the backend records it only on the
 *  card's own listing, never the old guess, and the trackers hear
 *  nothing of a stream that is still the show before. A session a
 *  switch to a pick opened is no guess, and a show picked again from it
 *  goes out the same way, apart from its page. */
export function sessionShow(o: {
	id: string;
	guess: boolean;
	row: string | null;
	linked: string | null;
}): {
	kitsuId?: string;
	guessRow?: string;
	cardRow?: string;
	guessId?: string;
	recordAs: string;
	mirrors: boolean;
} {
	if (!o.guess && o.linked === null) {
		const kitsuId = recordableId(o.id, false);
		// The card's row, so the backend keeps the show its line records:
		// a player mounted again after "Keep watching this" sends the
		// page's show until it has read the pick back.
		return {
			...(kitsuId ? { kitsuId } : {}),
			...(o.row ? { cardRow: o.row } : {}),
			recordAs: o.id,
			mirrors: true
		};
	}
	const show = o.linked ?? o.id;
	const row = guessRow(true, o.row);
	const id = guessedId(true, show);
	return {
		...(row ? { guessRow: row } : {}),
		...(id ? { guessId: id } : {}),
		recordAs: show,
		mirrors: o.linked === null
	};
}
