/**
 * The Up next card's words, for each of its forms: an entry to watch
 * next, the ways on, one not aired yet, or, at a chain's dead end, the
 * whole franchise. Paraglide messages only, so the card stays thin.
 */
import { m } from '$lib/paraglide/messages';
import { stackPartsText } from './strip-copy';
import type { FranchiseSummary } from './strip-view';
import type { ChainEndCard, PlayerUpNext, UpNext } from './up-next';

export type ShownUpNext = Exclude<UpNext, { readonly kind: 'none' }>;

/** The card's key over its title or its list. */
export function upNextKey(upNext: ShownUpNext | null): string {
	if (upNext === null) return m.watchorder_upnext_franchise_key();
	if (upNext.kind === 'choices') return m.watchorder_upnext_choices();
	if (upNext.kind === 'unaired') return m.watchorder_upnext_coming();
	return m.watchorder_upnext_eyebrow();
}

/** What the card announces once, politely, when it appears. */
export function upNextAnnouncement(
	upNext: ShownUpNext | null,
	franchise: ChainEndCard | null
): string {
	if (upNext === null)
		return m.watchorder_upnext_franchise_announce({ name: franchise?.name ?? '' });
	const first = upNext.kind === 'choices' ? upNext.entries[0] : upNext.entry;
	return m.watchorder_upnext_announce({ title: first.title });
}

/** The franchise card's meta line: the first two parts the strip's link names. */
export function franchiseCardMeta(summary: FranchiseSummary): string {
	return stackPartsText(summary, true).slice(0, 2).join(' · ');
}

/** The card's key over its title or its list. */
export function cardKey(card: PlayerUpNext): string {
	void card;
	return '';
}

/** What the card announces once, politely, when it appears. */
export function cardAnnouncement(card: PlayerUpNext): string {
	void card;
	return '';
}

/** The id the card's accent is taken from. */
export function cardAccentId(card: PlayerUpNext): string {
	void card;
	return '';
}

/** The franchise card's link label. */
export function franchiseCardLabel(card: ChainEndCard): string {
	void card;
	return '';
}
