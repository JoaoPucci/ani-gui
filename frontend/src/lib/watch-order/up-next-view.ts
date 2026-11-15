/**
 * The Up next card's words, for each of its forms: an entry to watch
 * next, the ways on, one not aired yet, or, at a chain's dead end, the
 * whole franchise. Paraglide messages only, so the card stays thin.
 */
import { m } from '$lib/paraglide/messages';
import { stackPartsText } from './strip-copy';
import type { FranchiseSummary } from './strip-view';
import type { WatchEntry } from './types';
import type { ChainEndCard, PlayerUpNext } from './chain-end';

/** The franchise card's meta line: the first two parts the strip's link names. */
export function franchiseCardMeta(summary: FranchiseSummary): string {
	return stackPartsText(summary, true).slice(0, 2).join(' · ');
}

/** The card's key over its title or its list. */
export function cardKey(card: PlayerUpNext): string {
	if (card.kind === 'franchise') return m.watchorder_upnext_franchise_key();
	const { upNext } = card.card;
	if (upNext.kind === 'choices') return m.watchorder_upnext_choices();
	if (upNext.kind === 'unaired') return m.watchorder_upnext_coming();
	return m.watchorder_upnext_eyebrow();
}

/** The entries an entry card names, in its order. */
export function cardEntries(card: Extract<PlayerUpNext, { kind: 'entry' }>): readonly WatchEntry[] {
	const { upNext } = card.card;
	return upNext.kind === 'choices' ? upNext.entries : [upNext.entry];
}

/** What the card announces once, politely, when it appears. */
export function cardAnnouncement(card: PlayerUpNext): string {
	if (card.kind === 'franchise')
		return m.watchorder_upnext_franchise_announce({ name: card.card.name });
	return m.watchorder_upnext_announce({ title: cardEntries(card)[0].title });
}

/** The id the card's accent is taken from: the first title it names,
 *  or the entry the franchise's name comes from, as on its page. */
export function cardAccentId(card: PlayerUpNext): string {
	return card.kind === 'franchise' ? card.card.front : cardEntries(card)[0].id;
}

/** The franchise card's link label: its key and the franchise's name. */
export function franchiseCardLabel(card: ChainEndCard): string {
	return m.watchorder_upnext_franchise_label({ name: card.name });
}
