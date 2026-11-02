// The page's record of which look each Continue card settled on, and
// the session's memory of the looks the cards settled on without the
// episode wait.
// The decision itself is ./card-look; this keeps its first answer.

import {
	decideLook,
	offeredEpisode,
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
 * The page's record of which look each card settled on. `look` is
 * called as the card renders; the first settled answer for a card is
 * kept, so the card does not change artwork while it offers the same
 * show and episode. A look decided for another show or another
 * episode — remembered from an earlier visit, or pinned before the
 * card's offered episode moved — no longer describes the card, and
 * the card resolves afresh.
 */
export function createLookBook(proxy: ProxyUrl, remembered: Map<string, RememberedCard>) {
	const pins = new Map<string, { look: SettledLook; text: CardText | null; instant: boolean }>();
	// Cards this page has drawn. Only a card's first draw may take its
	// look from an earlier visit: a look recorded while the card sat on
	// this page as a skeleton is this visit's answer, and it fades in.
	const seen = new Set<string>();

	function pinFor(entryId: string, key: string, inputs: LookInputs) {
		let pin = pins.get(entryId);
		const known = seen.has(entryId) ? undefined : remembered.get(key);
		seen.add(entryId);
		if (!pin && known) {
			pin = { ...known, instant: true };
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
			if (truth) remembered.set(key, { look: truth, text: inputs.text });
			if (pin) {
				// A card drawn from memory keeps the text it was drawn with
				// until the live details catch up, so the line does not
				// step back to "Episode N" in between.
				const text = pin.instant && !detailsIn(inputs) ? pin.text : (inputs.text ?? pin.text);
				return { look: pin.look, text, instant: pin.instant, landedOver: null };
			}
			const look = decideLook(inputs, proxy);
			if (look) pins.set(entryId, { look, text: null, instant: false });
			return { look, text: inputs.text, instant: false, landedOver: null };
		}
	};
}

export type LookBook = ReturnType<typeof createLookBook>;
