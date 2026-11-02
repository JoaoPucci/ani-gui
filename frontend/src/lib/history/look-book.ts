// The page's record of which look each Continue card settled on, and
// the session's memory of the looks the cards settled on without the
// episode wait.
// The decision itself is ./card-look; this keeps its first answer, and
// lets a held-back episode still land on it.

import {
	decideLook,
	offeredEpisode,
	posterSource,
	type CardText,
	type LookInputs,
	type ProxyUrl,
	type SettledLook
} from './card-look';

/** What the session remembers about a card's final look. */
export interface RememberedCard {
	look: SettledLook;
	text: CardText | null;
}

/** What a card renders. */
export interface CardLookState {
	/** null while the card is a skeleton. */
	look: SettledLook | null;
	/** Live text, or the remembered text until the live details catch up. */
	text: CardText | null;
	/** The look and text were known when the card first drew, so they
	 *  draw at once; an image the browser has yet to load still fades in. */
	instant: boolean;
	/** The look a still that landed later took the place of; null otherwise. */
	landedOver: SettledLook | null;
}

/** The cards' remembered final looks, kept across visits to Home. */
export const sessionLooks = new Map<string, RememberedCard>();

/** The session key for a history row: its id and the episode it records. */
export function lookKey(entry: { id: string; ep_no: string }): string {
	return `${entry.id}|${entry.ep_no}`;
}

/** A look decided for another show or another episode is no longer this card's. */
function contradicted(pin: { look: SettledLook }, inputs: LookInputs): boolean {
	const { match } = inputs;
	if (match === undefined) return false;
	if ((match?.id ?? null) !== pin.look.matchId) return true;
	return match !== null && inputs.offered !== pin.look.episode;
}

/** Whether the live details for the offered episode are in. */
function detailsIn(inputs: LookInputs): boolean {
	return (
		inputs.match === null ||
		offeredEpisode(inputs.episode, inputs.offered, inputs.episodeFor) !== undefined
	);
}

/**
 * Whether `after` is `before` with an episode still that has since
 * landed: the same show and episode, a look without a still before
 * (the poster, or the placeholder when there is none), the episode's
 * still after. AniList's pacing can hold a still back, and it lands
 * later on the episode details the card already settled on; it is the
 * one change a settled card takes.
 */
function stillLanded(before: SettledLook, after: SettledLook, poster: string | null): boolean {
	if (before.matchId !== after.matchId || before.episode !== after.episode) return false;
	const hadStill = before.image !== null && before.image.src !== poster;
	const hasStill = after.image !== null && after.image.src !== poster;
	return !hadStill && hasStill;
}

/**
 * The page's record of which look each card settled on. `look` is
 * called as the card renders; the first settled answer for a card is
 * kept, so the card does not change artwork while it offers the same
 * show and episode, but to take an episode still that lands after the
 * card settled without one (`stillLanded`). A look decided for another
 * show or another episode — remembered from an earlier visit, or pinned
 * before the card's offered episode moved — no longer describes the
 * card, and the card resolves afresh.
 */
export function createLookBook(proxy: ProxyUrl, remembered: Map<string, RememberedCard>) {
	const pins = new Map<
		string,
		{ look: SettledLook; text: CardText | null; instant: boolean; landedOver: SettledLook | null }
	>();
	// What each card's details last settled on, so a still that lands
	// later can be told from details that came with one.
	const truths = new Map<string, SettledLook>();
	// Cards this page has drawn. Only a card's first draw may take its
	// look from an earlier visit: a look recorded while the card sat on
	// this page as a skeleton is this visit's answer, and it fades in.
	const seen = new Set<string>();

	function pinFor(entryId: string, key: string, inputs: LookInputs) {
		let pin = pins.get(entryId);
		const known = seen.has(entryId) ? undefined : remembered.get(key);
		seen.add(entryId);
		if (!pin && known) {
			pin = { ...known, instant: true, landedOver: null };
			pins.set(entryId, pin);
		}
		if (pin && contradicted(pin, inputs)) {
			pins.delete(entryId);
			remembered.delete(key);
			return undefined;
		}
		return pin;
	}

	return {
		look(entryId: string, key: string, inputs: LookInputs): CardLookState {
			const pin = pinFor(entryId, key, inputs);
			// The final look the details settle on, cap or no cap, is
			// what the next visit should draw.
			const truth = decideLook({ ...inputs, capReached: false }, proxy);
			const before = truths.get(entryId);
			if (truth) {
				remembered.set(key, { look: truth, text: inputs.text });
				truths.set(entryId, truth);
			} else {
				truths.delete(entryId);
			}
			if (pin && truth && before && pin.look.image?.src === before.image?.src) {
				if (stillLanded(before, truth, proxy(posterSource(inputs.match)))) {
					pin.landedOver = pin.look;
					pin.look = truth;
				}
			}
			if (pin) {
				// A card drawn from memory keeps the text it was drawn with
				// until the live details catch up, so the line does not
				// step back to "Episode N" in between.
				const text = pin.instant && !detailsIn(inputs) ? pin.text : (inputs.text ?? pin.text);
				return { look: pin.look, text, instant: pin.instant, landedOver: pin.landedOver };
			}
			const look = decideLook(inputs, proxy);
			if (look) pins.set(entryId, { look, text: null, instant: false, landedOver: null });
			return { look, text: inputs.text, instant: false, landedOver: null };
		}
	};
}

export type LookBook = ReturnType<typeof createLookBook>;
