/**
 * The player's Up next card at the end of a chain outside the main
 * story that leads nowhere (the whole franchise), and the one reading
 * of which card a finale shows.
 */
import { chainFranchise } from './chain-view';
import type { WatchOrderData } from './loader';
import type { WatchOrder } from './order';
import { runOf } from './strip-model';
import { franchiseSummary, type FranchiseSummary } from './strip-view';
import { finaleCard, isFinale, type FinaleCard, type PlayerReading } from './up-next';

/**
 * The card at the last title of a chain outside the main story that
 * leads nowhere: the franchise page, by its name, fronted by the entry
 * the name comes from, with the poster stack and the counts the strip's
 * franchise link shows.
 */
export interface ChainEndCard {
	/** The franchise's name. */
	readonly name: string;
	/** The id the franchise page is opened for. */
	readonly id: string;
	/** The main-story entry the name comes from, which fronts the
	 *  stack and gives the card its accent. */
	readonly front: string;
	/** The franchise link's stack: its posters and its counts. */
	readonly summary: FranchiseSummary;
}

/**
 * The chain-end card `id`'s `episode` shows, or null: anywhere the
 * Up next card names something, or on anything but the finale of a
 * chain's last title that leads nowhere, read whole. `self`, the
 * player's own reading, decides the finale only when it was read for
 * `id`'s show, as for `finaleCard`.
 */
export function chainEndCard(
	data: WatchOrderData | null,
	id: string,
	order: WatchOrder | null,
	today: string,
	episode: number,
	self: PlayerReading | null
): ChainEndCard | null {
	if (finaleCard(data, id, order, today, episode, self) !== null) return null;
	if (data?.franchise == null || order === null || data.franchise.anchor !== id) return null;
	if (!data.franchise.complete) return null;
	const drawn = chainFranchise(data.franchise);
	const run = drawn ? runOf(drawn) : null;
	const entry = run?.entries.find((e) => e.id === id);
	const main = runOf(data.franchise);
	// A reading of another show (the page reused for a new one) says
	// nothing about this entry, as for the Up next card.
	const own = self?.id === id ? self : null;
	if (!entry || !main || !isFinale(own ?? entry, episode)) return null;
	const summary = franchiseSummary(main, data.franchise.name_from);
	// The franchise page's art: the title the name comes from, else the
	// main story's first, as franchiseSummary fronts the stack.
	const nameFrom = data.franchise.name_from;
	const front = main.entries.find((e) => e.id === nameFrom) ?? main.entries[0];
	return { name: data.franchise.name, id, front: front.id, summary };
}

/** The one card the player shows at a finale: an entry card, or the franchise card. */
export type PlayerUpNext =
	| { readonly kind: 'entry'; readonly card: FinaleCard }
	| { readonly kind: 'franchise'; readonly card: ChainEndCard };

/**
 * The one card `id`'s `episode` shows at its finale, or null: the
 * finale card where it names a next title (`finaleCard`), else the
 * franchise card at a chain's dead end (`chainEndCard`), else none.
 */
export function playerUpNext(
	data: WatchOrderData | null,
	id: string,
	order: WatchOrder | null,
	today: string,
	episode: number,
	self: PlayerReading | null
): PlayerUpNext | null {
	const finale = finaleCard(data, id, order, today, episode, self);
	if (finale) return { kind: 'entry', card: finale };
	const end = chainEndCard(data, id, order, today, episode, self);
	return end ? { kind: 'franchise', card: end } : null;
}
