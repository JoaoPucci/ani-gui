// Which look a Continue Watching card is in.
//
// A card's artwork depends on two answers that land at different
// times: the row's Kitsu match (the show's poster) and then the
// episode details (the episode's thumbnail). Drawing each answer as it
// arrived made a card go placeholder → poster → thumbnail. Here a card
// stays a skeleton until its final artwork is known and then keeps it:
//
//   - match pending                → skeleton
//   - no match                     → the two-letter placeholder
//   - episode details settled      → thumbnail, else poster, else placeholder
//   - episode details still out at the cap → poster, else placeholder
//
// The first look a card settles on is pinned for the life of the page,
// so a thumbnail that lands after the cap does not swap it. What the
// details would have settled on is remembered for the session, so the
// next visit to Home draws the card in its final look at once.

import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/**
 * How long a matched card waits for its episode details before it
 * settles on the show's poster. Measured on a seeded profile of fifty
 * Continue rows, from a card's match to its episode details: 95% of
 * cards land within about 390 ms on a fresh launch and within about
 * 370 ms on a return to Home, so a card waits no longer than that
 * while a slow lookup (an episode page Kitsu has to be asked for)
 * settles on its poster instead of holding the skeleton.
 */
export const EPISODE_WAIT_CAP_MS = 400;

/** The artwork a card shows, and what to show if that one fails. */
export interface CardImage {
	src: string;
	fallback: string | null;
}

/** A card's settled artwork. `image: null` is the two-letter placeholder. */
export interface SettledLook {
	image: CardImage | null;
	/** The match the look was decided against; null when the row has none. */
	matchId: string | null;
}

/** The text a card carries beside its artwork. */
export interface CardText {
	/** The number on the artwork; null for a single-video show. */
	tag: number | null;
	/** The episode's own title, when Kitsu has one. */
	title: string | null;
	/** The "Episode N" line shown when there is no title; null when none. */
	episodeLabel: number | null;
}

export interface LookInputs {
	/** undefined while the row is being matched; null when it failed. */
	match: KitsuAnimeRef | null | undefined;
	/** undefined until the episode lookup settles. */
	episode: KitsuEpisode | null | undefined;
	/** Whether the episode wait has run out for this card. */
	capReached: boolean;
	/** The card's text from live data; null while the row is unmatched. */
	text: CardText | null;
}

/** Turns a remote image URL into the one the card loads, or null. */
export type ProxyUrl = (url: string | null) => string | null;

/** The poster size a card uses, largest-fitting first. */
export function posterSource(match: KitsuAnimeRef | null | undefined): string | null {
	const p = match?.poster_image;
	return p?.medium ?? p?.large ?? p?.small ?? p?.original ?? null;
}

/** The text a card shows for an episode number. */
export function cardText(
	episode: KitsuEpisode | null | undefined,
	singleVideo: boolean,
	episodeNumber: number
): CardText {
	const title = episode?.canonical_title ?? null;
	return {
		tag: singleVideo ? null : episodeNumber,
		title,
		episodeLabel: title === null && !singleVideo ? episodeNumber : null
	};
}

function posterLook(match: KitsuAnimeRef, proxy: ProxyUrl): SettledLook {
	const poster = proxy(posterSource(match));
	return { image: poster ? { src: poster, fallback: null } : null, matchId: match.id };
}

/**
 * The look a card settles on given what is known, or null while it is
 * still a skeleton. Pure: pinning the first answer is the book's job.
 */
export function decideLook(inputs: LookInputs, proxy: ProxyUrl): SettledLook | null {
	const { match, episode } = inputs;
	if (match === undefined) return null;
	if (match === null) return { image: null, matchId: null };
	if (episode !== undefined) {
		const thumb = proxy(episode?.thumbnail?.original ?? null);
		const poster = posterLook(match, proxy);
		if (!thumb) return poster;
		return { image: { src: thumb, fallback: poster.image?.src ?? null }, matchId: match.id };
	}
	return inputs.capReached ? posterLook(match, proxy) : null;
}

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

/**
 * The image a settled look shows once failures are counted: its own,
 * else its fallback, else none (the placeholder).
 */
export function visibleImage(
	image: CardImage | null,
	failed: Readonly<Record<string, true>>
): string | null {
	if (!image) return null;
	if (!failed[image.src]) return image.src;
	if (image.fallback && !failed[image.fallback]) return image.fallback;
	return null;
}

export interface CapTimerDeps {
	capMs: number;
	schedule: (fn: () => void, ms: number) => unknown;
	cancel: (handle: unknown) => void;
	onElapsed: (entryId: string) => void;
}

/**
 * One episode-wait timer per card, started when the card's match
 * lands. Arming a card twice keeps the first timer: the wait counts
 * from the match, not from the latest re-render.
 */
export function createCapTimers(deps: CapTimerDeps) {
	const handles = new Map<string, unknown>();
	let disposed = false;
	return {
		arm(entryId: string) {
			if (disposed || handles.has(entryId)) return;
			handles.set(
				entryId,
				deps.schedule(() => deps.onElapsed(entryId), deps.capMs)
			);
		},
		dispose() {
			disposed = true;
			for (const h of handles.values()) deps.cancel(h);
			handles.clear();
		}
	};
}
