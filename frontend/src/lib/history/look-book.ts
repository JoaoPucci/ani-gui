// The page's record of which look each Continue card settled on, and
// the session's memory of the looks the cards' details settled on.
// The decision itself is ./card-look; this keeps its first answer.

import {
	decideLook,
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
	/** Live text, or the remembered text while the row is unmatched. */
	text: CardText | null;
	/** The look was known when the card first drew, so nothing fades in. */
	instant: boolean;
}

/** The cards' remembered final looks, kept across visits to Home. */
export const sessionLooks = new Map<string, RememberedCard>();

/** The session key for a history row: its id and the episode it records. */
export function lookKey(entry: { id: string; ep_no: string }): string {
	return `${entry.id}|${entry.ep_no}`;
}

/** A remembered look the live match contradicts is no longer this card's. */
function contradicted(pin: { look: SettledLook }, match: LookInputs['match']): boolean {
	return match !== undefined && (match?.id ?? null) !== pin.look.matchId;
}

/**
 * The page's record of which look each card settled on. `look` is
 * called as the card renders; the first settled answer for a card is
 * kept, so the card never changes artwork afterwards. The one
 * exception is a look remembered from an earlier visit that the live
 * match contradicts: that look belongs to a show the row no longer
 * resolves to, and the card resolves afresh.
 */
export function createLookBook(proxy: ProxyUrl, remembered: Map<string, RememberedCard>) {
	const pins = new Map<string, { look: SettledLook; text: CardText | null; instant: boolean }>();
	// Cards this page has drawn. Only a card's first draw may take its
	// look from an earlier visit: a look recorded while the card sat on
	// this page as a skeleton is this visit's answer, and it fades in.
	const seen = new Set<string>();

	function pinFor(entryId: string, key: string, match: LookInputs['match']) {
		let pin = pins.get(entryId);
		const known = seen.has(entryId) ? undefined : remembered.get(key);
		seen.add(entryId);
		if (!pin && known) {
			pin = { ...known, instant: true };
			pins.set(entryId, pin);
		}
		if (pin?.instant && contradicted(pin, match)) {
			pins.delete(entryId);
			remembered.delete(key);
			return undefined;
		}
		return pin;
	}

	return {
		look(entryId: string, key: string, inputs: LookInputs): CardLookState {
			const pin = pinFor(entryId, key, inputs.match);
			// The final look the details settle on, cap or no cap, is
			// what the next visit should draw.
			const truth = decideLook({ ...inputs, capReached: false }, proxy);
			if (truth) remembered.set(key, { look: truth, text: inputs.text });
			if (pin) return { look: pin.look, text: inputs.text ?? pin.text, instant: pin.instant };
			const look = decideLook(inputs, proxy);
			if (look) pins.set(entryId, { look, text: null, instant: false });
			return { look, text: inputs.text, instant: false };
		}
	};
}

export type LookBook = ReturnType<typeof createLookBook>;
